use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Json, Router};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{Acquire, PgPool, QueryBuilder, Row};
use uuid::Uuid;

use std::collections::HashMap;

use crate::csv_import::{self, ColumnSpec, RowError};
use crate::errors::{map_database_error, ApiError};
use crate::extractors::ApiJson;
use crate::helpers::{validate_email_like, validate_text_len};
use crate::modules::audit;
use crate::modules::auth::{
    assign_roles_in_tx, generate_temp_password, hash_password, roles_for_user, AuthUser,
};
use crate::modules::rbac::Role;
use crate::pagination::{Paginated, Pagination, PaginationQuery};
use crate::state::AppState;
use crate::storage::{content_type_for_key, image_extension, MAX_AVATAR_BYTES};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/agents",
            axum::routing::get(list_agents).post(create_agent),
        )
        .route(
            "/agents/import-csv",
            axum::routing::post(import_agents_csv)
                .layer(DefaultBodyLimit::max(MAX_IMPORT_CSV_BYTES)),
        )
        .route(
            "/agents/{id}/account",
            axum::routing::post(link_agent_mobile_account),
        )
        .route(
            "/agents/{id}",
            axum::routing::get(get_agent).patch(patch_agent),
        )
        .route("/agents/{id}/suspend", axum::routing::post(suspend_agent))
        .route(
            "/agents/{id}/reactivate",
            axum::routing::post(reactivate_agent),
        )
        .route("/agents/{id}/retire", axum::routing::post(retire_agent))
        .route(
            "/agents/{id}/photo",
            axum::routing::get(get_agent_photo_content)
                .post(upload_agent_photo)
                .layer(DefaultBodyLimit::max(MAX_AVATAR_BYTES)),
        )
}

pub fn public_router() -> Router<AppState> {
    Router::new()
        .route(
            "/agents/verify/{matricule}",
            axum::routing::get(verify_agent_public),
        )
        .route(
            "/agents/verify/{matricule}/photo",
            axum::routing::get(verify_agent_photo_public),
        )
}

#[derive(Debug, Deserialize)]
struct AgentFilterQuery {
    page: Option<i64>,
    page_size: Option<i64>,
    status: Option<String>,
    commune_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
struct ImportAgentsQuery {
    /// Commune **par défaut**, appliquée aux seules lignes sans colonne commune.
    /// Optionnelle : le fichier national du client porte `Code_Commune_attache` par
    /// ligne, et un ADMIN_COMMUNE n'a pas à la préciser.
    #[serde(default)]
    commune_id: Option<Uuid>,
    /// Autorise le rattachement d'un matricule déjà enregistré dans une autre commune.
    /// Réservé au SUPER_ADMIN et audité agent par agent.
    #[serde(default)]
    allow_transfer: Option<bool>,
    /// Simulation : tout est exécuté puis annulé, les compteurs sont donc exacts.
    #[serde(default)]
    dry_run: Option<bool>,
    /// Provisionner le compte mobile des agents importés. Absent = `true`.
    #[serde(default)]
    create_accounts: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CreateAgentRequest {
    matricule: String,
    full_name: String,
    commune_id: Uuid,
    date_prise_fonction: Option<NaiveDate>,
    photo_url: Option<String>,
    telephone: Option<String>,
    email: Option<String>,
    /// Provisionner le compte mobile. Absent = `true` : un agent est un utilisateur.
    #[serde(default)]
    create_account: Option<bool>,
}

#[derive(Debug, Serialize)]
struct ImportAgentsResponse {
    created: usize,
    updated: usize,
    skipped: usize,
    /// Matricules réactivés (l'agent avait été supprimé logiquement).
    restored: usize,
    /// Agents rattachés à une autre commune (uniquement avec `allow_transfer`).
    transferred: usize,
    total_rows: usize,
    error_count: usize,
    errors: Vec<RowError>,
    errors_truncated: bool,
    dry_run: bool,
    /// Ventilation par commune — confirme le dispatch d'un fichier national.
    communes: Vec<ImportCommuneSummary>,
    /// Comptes mobiles provisionnés pendant l'import, avec leur mot de passe temporaire.
    /// Restitués une seule fois : à exporter immédiatement pour distribution aux agents.
    /// Vide en simulation (`dry_run`), où rien n'a été conservé.
    accounts: Vec<ProvisionedAccount>,
}

#[derive(Debug, Serialize, Clone)]
struct ImportCommuneSummary {
    commune_id: Uuid,
    code: String,
    nom: String,
    created: usize,
    updated: usize,
    restored: usize,
}

/// Commune résolue depuis le fichier (par code, par nom ou par identifiant).
#[derive(Debug, Clone)]
struct CommuneRef {
    id: Uuid,
    code: String,
    nom: String,
}

/// Une ligne du fichier, déjà validée.
struct ParsedAgentRow {
    line: usize,
    matricule: String,
    full_name: String,
    commune_token: Option<String>,
    date_prise_fonction: Option<NaiveDate>,
    telephone: Option<String>,
    email: Option<String>,
}

/// Colonnes acceptées. Les alias couvrent les en-têtes du fichier client
/// (`Matricule`, `Nom_Complet`, `Code_Commune_attache`) et ceux de l'export agents,
/// pour que le cycle « exporter → corriger dans Excel → réimporter » fonctionne.
const AGENT_COLUMNS: &[ColumnSpec] = &[
    ColumnSpec {
        canonical: "matricule",
        aliases: &["n_matricule", "numero_matricule", "matricule_agent"],
        loose_aliases: &[],
        required: true,
    },
    ColumnSpec {
        canonical: "full_name",
        aliases: &[
            "nom_complet",
            "noms_et_prenoms",
            "nom_et_prenom",
            "nom_prenom",
            "nom_complet_agent",
        ],
        loose_aliases: &["nom", "agent", "nom_agent"],
        required: true,
    },
    ColumnSpec {
        canonical: "code_commune",
        aliases: &[
            "code_commune_attache",
            "code_commune_d_attache",
            "commune_code",
            "code_com",
        ],
        loose_aliases: &["commune", "commune_attache", "commune_d_attache"],
        required: false,
    },
    ColumnSpec {
        canonical: "date_prise_fonction",
        aliases: &[
            "date_de_prise_de_fonction",
            "prise_de_fonction",
            "date_prise_service",
        ],
        loose_aliases: &[],
        required: false,
    },
    ColumnSpec {
        canonical: "telephone",
        aliases: &["numero_telephone", "tel", "phone", "contact"],
        loose_aliases: &[],
        required: false,
    },
    ColumnSpec {
        canonical: "email",
        aliases: &["e_mail", "mail", "courriel", "adresse_email"],
        loose_aliases: &[],
        required: false,
    },
];

/// 8 Mio ≈ bien au-delà du fichier national attendu, et le plafond de lignes prend le
/// relais avant que la mémoire ne soit un sujet.
const MAX_IMPORT_CSV_BYTES: usize = 8 * 1024 * 1024;
const MAX_IMPORT_ROWS: usize = 20_000;

#[derive(Debug, Deserialize)]
struct PatchAgentRequest {
    matricule: Option<String>,
    full_name: Option<String>,
    commune_id: Option<Uuid>,
    status: Option<String>,
    date_prise_fonction: Option<NaiveDate>,
    photo_url: Option<String>,
    telephone: Option<String>,
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LinkAgentAccountRequest {
    email: String,
    password: String,
    active: Option<bool>,
}

#[derive(Debug, Serialize)]
struct LinkAgentAccountResponse {
    agent_id: Uuid,
    user_id: Uuid,
    email: String,
    full_name: String,
    commune_id: Uuid,
    linked: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct AgentResponse {
    id: Uuid,
    matricule: String,
    full_name: String,
    commune_id: Uuid,
    status: String,
    date_prise_fonction: Option<NaiveDate>,
    photo_url: Option<String>,
    has_photo: bool,
    telephone: Option<String>,
    email: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    /// Renseigne uniquement dans la reponse de creation, jamais en lecture : le mot de
    /// passe temporaire n'est restituable qu'une fois.
    #[serde(skip_serializing_if = "Option::is_none")]
    account: Option<ProvisionedAccount>,
}

/// Compte mobile cree automatiquement pour un agent.
///
/// `temp_password` n'est jamais stocke en clair ni relu : il n'existe que dans la reponse
/// HTTP de creation/import, a charge de l'administrateur de le transmettre a l'agent.
#[derive(Debug, Serialize, Clone)]
pub struct ProvisionedAccount {
    pub user_id: Uuid,
    pub matricule: String,
    pub full_name: String,
    /// Adresse technique de connexion. L'agent se connecte normalement par matricule.
    pub email: String,
    pub temp_password: String,
}

/// Domaine des adresses techniques : elles n'ont pas vocation a recevoir du courrier,
/// elles ne servent qu'a satisfaire l'unicite de `users.email`.
const AGENT_LOGIN_DOMAIN: &str = "agents.apmtrack.cm";

#[derive(Debug, Serialize)]
struct PublicAgentVerification {
    matricule: String,
    full_name: String,
    commune_code: String,
    commune_nom: String,
    status: String,
    active: bool,
    has_photo: bool,
}

async fn get_agent(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(agent_id): Path<Uuid>,
) -> Result<Json<AgentResponse>, ApiError> {
    auth_user.require_any_role(&[
        Role::SuperAdmin,
        Role::AdminCommune,
        Role::ApmAgent,
        Role::Superviseur,
        Role::Receveur,
    ])?;
    let agent = load_agent(&state.db, agent_id).await?;
    auth_user.require_commune_access(agent.commune_id)?;
    Ok(Json(agent))
}

async fn list_agents(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<AgentFilterQuery>,
) -> Result<Json<Paginated<AgentResponse>>, ApiError> {
    auth_user.require_any_role(&[
        Role::SuperAdmin,
        Role::AdminCommune,
        Role::ApmAgent,
        Role::Superviseur,
        Role::Receveur,
    ])?;
    let pagination = Pagination::from_query(PaginationQuery {
        page: query.page,
        page_size: query.page_size,
    })?;

    let status_filter = match query.status {
        Some(ref s) => Some(validate_agent_status(s)?),
        None => None,
    };

    let commune_filter: Option<Uuid> = if auth_user.has_role(Role::SuperAdmin)
        || (auth_user.has_role(Role::Superviseur) && auth_user.commune_id.is_none())
    {
        query.commune_id
    } else {
        let user_commune = auth_user
            .commune_id
            .ok_or_else(|| ApiError::forbidden("Utilisateur non rattache a une commune"))?;
        if let Some(req) = query.commune_id {
            if req != user_commune {
                return Err(ApiError::forbidden("Acces refuse a cette commune"));
            }
        }
        Some(user_commune)
    };

    let mut count_qb: QueryBuilder<sqlx::Postgres> =
        QueryBuilder::new("SELECT COUNT(*) AS total FROM agents WHERE deleted_at IS NULL");
    if let Some(id) = commune_filter {
        count_qb.push(" AND commune_id = ").push_bind(id);
    }
    if let Some(ref s) = status_filter {
        count_qb.push(" AND status = ").push_bind(s.clone());
    }
    let total: i64 = count_qb.build().fetch_one(&state.db).await?.get("total");

    let mut qb: QueryBuilder<sqlx::Postgres> =
        QueryBuilder::new("SELECT * FROM agents WHERE deleted_at IS NULL");
    if let Some(id) = commune_filter {
        qb.push(" AND commune_id = ").push_bind(id);
    }
    if let Some(ref s) = status_filter {
        qb.push(" AND status = ").push_bind(s.clone());
    }
    qb.push(" ORDER BY created_at DESC LIMIT ")
        .push_bind(pagination.limit)
        .push(" OFFSET ")
        .push_bind(pagination.offset);

    let rows = qb.build().fetch_all(&state.db).await?;
    let agents = rows.into_iter().map(row_to_agent).collect::<Vec<_>>();
    Ok(Json(Paginated::new(agents, &pagination, total)))
}

async fn create_agent(
    State(state): State<AppState>,
    auth_user: AuthUser,
    ApiJson(payload): ApiJson<CreateAgentRequest>,
) -> Result<Json<AgentResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    auth_user.require_commune_access(payload.commune_id)?;

    let agent_id = Uuid::new_v4();
    let matricule = required_text(payload.matricule, "matricule")?;
    let full_name = required_text(payload.full_name, "full_name")?;
    // Un agent est d'office un utilisateur de l'application mobile : son compte est cree
    // avec sa fiche. `create_account: false` reste possible pour un agent qui ne doit pas
    // se connecter (detachement, agent purement administratif).
    let wants_account = payload.create_account.unwrap_or(true);

    let mut tx = state.db.begin().await?;
    sqlx::query(
        r#"
        INSERT INTO agents (
            id, matricule, full_name, commune_id, status,
            date_prise_fonction, photo_url, telephone, email
        )
        VALUES ($1, $2, $3, $4, 'ACTIF', $5, $6, $7, $8)
        "#,
    )
    .bind(agent_id)
    .bind(&matricule)
    .bind(&full_name)
    .bind(payload.commune_id)
    .bind(payload.date_prise_fonction)
    .bind(clean_optional(payload.photo_url))
    .bind(clean_optional(payload.telephone))
    .bind(clean_optional(payload.email))
    .execute(&mut *tx)
    .await
    .map_err(map_database_error)?;

    let account = if wants_account {
        provision_agent_account(&mut tx, agent_id, &matricule, &full_name, payload.commune_id)
            .await?
    } else {
        None
    };

    audit::record_for_commune_tx(
        &mut tx,
        Some(payload.commune_id),
        Some(auth_user.id),
        "AGENT_CREATED",
        "agents",
        Some(agent_id),
        None,
        Some(json!({
            "id": agent_id,
            "account_provisioned": account.is_some(),
        })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;
    tx.commit().await?;

    let mut response = load_agent(&state.db, agent_id).await?;
    response.account = account;
    Ok(Json(response))
}

/// Cree le compte mobile d'un agent qui n'en a pas encore.
///
/// Renvoie `None` si l'agent est deja rattache a un utilisateur — l'appel est donc
/// idempotent et ne reinitialise jamais le mot de passe d'un compte existant.
///
/// L'adresse est technique et derivee du matricule : `users.email` est NOT NULL et unique,
/// alors qu'un agent de terrain n'a pas toujours d'adresse. La connexion se fait de toute
/// facon par matricule (voir `auth::load_login_user`).
async fn provision_agent_account(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    agent_id: Uuid,
    matricule: &str,
    full_name: &str,
    commune_id: Uuid,
) -> Result<Option<ProvisionedAccount>, ApiError> {
    let already_linked: Option<Uuid> = sqlx::query_scalar(
        "SELECT user_id FROM agents WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(agent_id)
    .fetch_optional(&mut **tx)
    .await?
    .flatten();
    if already_linked.is_some() {
        return Ok(None);
    }

    let email = allocate_login_email(tx, matricule, agent_id).await?;
    let temp_password = generate_temp_password();
    let password_hash = hash_password(&temp_password)?;
    let user_id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO users (
            id, email, password_hash, full_name, commune_id, active, must_change_password
        )
        VALUES ($1, $2, $3, $4, $5, TRUE, TRUE)
        "#,
    )
    .bind(user_id)
    .bind(&email)
    .bind(&password_hash)
    .bind(full_name)
    .bind(commune_id)
    .execute(&mut **tx)
    .await
    .map_err(map_database_error)?;

    assign_roles_in_tx(tx, user_id, &[Role::ApmAgent]).await?;

    sqlx::query("UPDATE agents SET user_id = $2, updated_at = now() WHERE id = $1")
        .bind(agent_id)
        .bind(user_id)
        .execute(&mut **tx)
        .await
        .map_err(map_database_error)?;

    Ok(Some(ProvisionedAccount {
        user_id,
        matricule: matricule.to_string(),
        full_name: full_name.to_string(),
        email,
        temp_password,
    }))
}

/// Adresse technique libre pour un matricule.
///
/// Le matricule est unique parmi les agents vivants, mais l'utilisateur d'un agent
/// supprime logiquement, lui, subsiste : en cas de collision on suffixe avec le debut de
/// l'identifiant de l'agent plutot que d'echouer la creation.
async fn allocate_login_email(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    matricule: &str,
    agent_id: Uuid,
) -> Result<String, ApiError> {
    let local: String = matricule
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '-'
            }
        })
        .collect();
    let local = local.trim_matches(['.', '-', '_']).to_string();
    let local = if local.is_empty() {
        "agent".to_string()
    } else {
        local
    };

    let candidate = format!("{local}@{AGENT_LOGIN_DOMAIN}");
    let taken: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM users WHERE lower(email) = lower($1) AND deleted_at IS NULL",
    )
    .bind(&candidate)
    .fetch_optional(&mut **tx)
    .await?;
    if taken.is_none() {
        return Ok(candidate);
    }

    let suffix = &agent_id.simple().to_string()[..8];
    Ok(format!("{local}-{suffix}@{AGENT_LOGIN_DOMAIN}"))
}

async fn import_agents_csv(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<ImportAgentsQuery>,
    body: Bytes,
) -> Result<Json<ImportAgentsResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;

    let allow_transfer = query.allow_transfer.unwrap_or(false);
    if allow_transfer && !auth_user.has_role(Role::SuperAdmin) {
        return Err(ApiError::forbidden(
            "allow_transfer est reserve au SUPER_ADMIN",
        ));
    }
    // La commune passée en paramètre reste soumise au contrôle de périmètre ; le
    // contrôle décisif reste toutefois celui fait ligne par ligne plus bas.
    if let Some(id) = query.commune_id {
        auth_user.require_commune_access(id)?;
    }
    let dry_run = query.dry_run.unwrap_or(false);
    let provision_accounts = query.create_accounts.unwrap_or(true);

    let content = csv_import::decode(&body);
    if content.trim().is_empty() {
        return Err(ApiError::bad_request("Le fichier CSV est vide"));
    }

    let delimiter = csv_import::detect_delimiter(&content);
    let mut reader = csv_import::reader(&content, delimiter);
    let headers = reader
        .headers()
        .map_err(|error| ApiError::bad_request(format!("En-tete CSV illisible: {error}")))?
        .clone();
    let columns = csv_import::resolve_columns(&headers, AGENT_COLUMNS)?;

    // ── Lecture et validation ligne à ligne ──────────────────────────────────────
    let mut rows: Vec<ParsedAgentRow> = Vec::new();
    let mut errors: Vec<RowError> = Vec::new();
    let mut error_count = 0usize;
    let mut skipped = 0usize;
    let mut seen_matricules: HashMap<String, usize> = HashMap::new();

    for (index, record) in reader.records().enumerate() {
        let record = match record {
            Ok(record) => record,
            Err(error) => {
                skipped += 1;
                csv_import::push_error(&mut errors, &mut error_count, index + 2, error.to_string());
                continue;
            }
        };
        let line = record
            .position()
            .map(|position| position.line() as usize)
            .unwrap_or(index + 2);

        // Les lignes entièrement vides (« ;;; » de fin de fichier Excel) sont ignorées
        // sans être comptées comme rejets.
        if csv_import::is_blank(&record) {
            continue;
        }

        if rows.len() + skipped >= MAX_IMPORT_ROWS {
            return Err(ApiError::bad_request(format!(
                "Le fichier depasse la limite de {MAX_IMPORT_ROWS} lignes; scindez-le"
            )));
        }

        let matricule = match columns.get(&record, "matricule") {
            Some(value) => value.to_string(),
            None => {
                skipped += 1;
                csv_import::push_error(&mut errors, &mut error_count, line, "matricule est requis");
                continue;
            }
        };
        let full_name = match columns.get(&record, "full_name") {
            Some(value) => value.to_string(),
            None => {
                skipped += 1;
                csv_import::push_error(&mut errors, &mut error_count, line, "full_name est requis");
                continue;
            }
        };
        if let Err(error) = validate_text_len(&matricule, "matricule", 50) {
            skipped += 1;
            csv_import::push_error(&mut errors, &mut error_count, line, error.message());
            continue;
        }
        if let Err(error) = validate_text_len(&full_name, "full_name", 150) {
            skipped += 1;
            csv_import::push_error(&mut errors, &mut error_count, line, error.message());
            continue;
        }

        // Doublon interne : sans ce contrôle, deux lignes du même matricule visant des
        // communes différentes se battaient en silence, la dernière l'emportant.
        let key = matricule.to_lowercase();
        if let Some(previous) = seen_matricules.get(&key) {
            skipped += 1;
            csv_import::push_error(
                &mut errors,
                &mut error_count,
                line,
                format!("matricule '{matricule}' en doublon dans le fichier (deja ligne {previous})"),
            );
            continue;
        }
        seen_matricules.insert(key, line);

        let date_prise_fonction = match columns.get(&record, "date_prise_fonction") {
            Some(raw) => match csv_import::parse_date(raw) {
                Some(date) => Some(date),
                None => {
                    skipped += 1;
                    csv_import::push_error(
                        &mut errors,
                        &mut error_count,
                        line,
                        format!("date_prise_fonction illisible: '{raw}' (attendu AAAA-MM-JJ ou JJ/MM/AAAA)"),
                    );
                    continue;
                }
            },
            None => None,
        };

        let email = columns.get(&record, "email").map(str::to_string);
        if let Err(error) = validate_email_like(email.as_deref(), "email") {
            skipped += 1;
            csv_import::push_error(&mut errors, &mut error_count, line, error.message());
            continue;
        }

        rows.push(ParsedAgentRow {
            line,
            matricule,
            full_name,
            commune_token: columns.get(&record, "code_commune").map(str::to_string),
            date_prise_fonction,
            telephone: columns.get(&record, "telephone").map(str::to_string),
            email,
        });
    }

    let total_rows = rows.len() + skipped;

    // ── Préchargement (jamais une requête par ligne) ─────────────────────────────
    let tokens: Vec<String> = rows
        .iter()
        .filter_map(|row| row.commune_token.as_ref())
        .map(|token| token.to_lowercase())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let mut commune_by_token: HashMap<String, Option<CommuneRef>> = HashMap::new();
    if !tokens.is_empty() {
        let commune_rows = sqlx::query(
            r#"
            SELECT id, code, nom
            FROM communes
            WHERE deleted_at IS NULL
              AND (lower(code) = ANY($1) OR lower(nom) = ANY($1) OR id::text = ANY($1))
            "#,
        )
        .bind(&tokens)
        .fetch_all(&state.db)
        .await?;

        for row in commune_rows {
            let reference = CommuneRef {
                id: row.get("id"),
                code: row.get("code"),
                nom: row.get("nom"),
            };
            for key in [
                reference.code.to_lowercase(),
                reference.nom.to_lowercase(),
                reference.id.to_string(),
            ] {
                if !tokens.contains(&key) {
                    continue;
                }
                // Un libellé de commune peut être ambigu (homonymes) ; on marque alors
                // le jeton comme non résoluble plutôt que d'en choisir un au hasard.
                // Les codes, eux, sont uniques (index communes_code_unique_ci).
                match commune_by_token.get(&key) {
                    Some(Some(existing)) if existing.id != reference.id => {
                        commune_by_token.insert(key, None);
                    }
                    Some(None) => {}
                    _ => {
                        commune_by_token.insert(key, Some(reference.clone()));
                    }
                }
            }
        }
    }

    let default_commune = match query.commune_id.or(auth_user.commune_id) {
        Some(id) => {
            let row = sqlx::query("SELECT id, code, nom FROM communes WHERE id = $1 AND deleted_at IS NULL")
                .bind(id)
                .fetch_optional(&state.db)
                .await?
                .ok_or_else(|| ApiError::bad_request("Commune par defaut introuvable"))?;
            Some(CommuneRef {
                id: row.get("id"),
                code: row.get("code"),
                nom: row.get("nom"),
            })
        }
        None => None,
    };

    let matricule_keys: Vec<String> = rows
        .iter()
        .map(|row| row.matricule.to_lowercase())
        .collect();
    let mut existing_agents: HashMap<String, (Uuid, Uuid, bool)> = HashMap::new();
    if !matricule_keys.is_empty() {
        // `deleted_at` inclus : un matricule supprimé logiquement doit être restauré et
        // non dupliqué (l'index unique étant partiel, l'INSERT passait).
        let agent_rows = sqlx::query(
            r#"
            SELECT id, lower(matricule) AS matricule_key, commune_id,
                   (deleted_at IS NOT NULL) AS is_deleted
            FROM agents
            WHERE lower(matricule) = ANY($1)
            ORDER BY (deleted_at IS NOT NULL), created_at DESC
            "#,
        )
        .bind(&matricule_keys)
        .fetch_all(&state.db)
        .await?;
        for row in agent_rows {
            let key: String = row.get("matricule_key");
            existing_agents.entry(key).or_insert((
                row.get("id"),
                row.get("commune_id"),
                row.get("is_deleted"),
            ));
        }
    }

    // Colonnes réellement présentes : une colonne absente ne doit JAMAIS écraser la
    // valeur en base. Sans ce garde-fou, importer le fichier national à 3 colonnes
    // effaçait le téléphone et l'e-mail de tous les agents déjà saisis.
    let has_date = columns.has("date_prise_fonction");
    let has_phone = columns.has("telephone");
    let has_email = columns.has("email");

    // ── Écriture, un SAVEPOINT par ligne ────────────────────────────────────────
    let mut created = 0usize;
    let mut updated = 0usize;
    let mut restored = 0usize;
    let mut transferred = 0usize;
    let mut per_commune: HashMap<Uuid, ImportCommuneSummary> = HashMap::new();
    let mut transfers: Vec<(Uuid, Uuid, Uuid)> = Vec::new();
    let mut accounts: Vec<ProvisionedAccount> = Vec::new();
    let mut tx = state.db.begin().await?;

    for row in &rows {
        let target = match row.commune_token.as_ref() {
            Some(token) => match commune_by_token.get(&token.to_lowercase()) {
                Some(Some(reference)) => reference.clone(),
                Some(None) => {
                    skipped += 1;
                    csv_import::push_error(
                        &mut errors,
                        &mut error_count,
                        row.line,
                        format!("commune '{token}' ambigue, utilisez le code commune"),
                    );
                    continue;
                }
                None => {
                    skipped += 1;
                    csv_import::push_error(
                        &mut errors,
                        &mut error_count,
                        row.line,
                        format!("commune inconnue '{token}'"),
                    );
                    continue;
                }
            },
            None => match default_commune.clone() {
                Some(reference) => reference,
                None => {
                    skipped += 1;
                    csv_import::push_error(
                        &mut errors,
                        &mut error_count,
                        row.line,
                        "commune absente — ajoutez une colonne code_commune ou choisissez une commune par defaut",
                    );
                    continue;
                }
            },
        };

        // FRONTIÈRE DE CLOISONNEMENT : le contrôle porte sur la commune *résolue*, pas
        // sur un paramètre fourni par l'appelant. Un ADMIN_COMMUNE à qui l'on confie le
        // fichier national importe sa part et voit les autres lignes rejetées.
        if auth_user.require_commune_access(target.id).is_err() {
            skipped += 1;
            csv_import::push_error(
                &mut errors,
                &mut error_count,
                row.line,
                format!("commune '{}' hors de votre perimetre", target.code),
            );
            continue;
        }

        let key = row.matricule.to_lowercase();
        let existing = existing_agents.get(&key).copied();

        if let Some((_, current_commune, _)) = existing {
            if current_commune != target.id && !allow_transfer {
                skipped += 1;
                csv_import::push_error(
                    &mut errors,
                    &mut error_count,
                    row.line,
                    format!(
                        "matricule '{}' deja rattache a une autre commune — relancez avec allow_transfer=true pour le transferer",
                        row.matricule
                    ),
                );
                continue;
            }
        }

        let mut savepoint = tx.begin().await?;
        let outcome = write_agent_row(
            &mut savepoint,
            row,
            &target,
            existing,
            (has_date, has_phone, has_email),
            provision_accounts,
        )
        .await;

        match outcome {
            Ok((outcome, account)) => {
                savepoint.commit().await?;
                if let Some(account) = account {
                    accounts.push(account);
                }
                let entry = per_commune
                    .entry(target.id)
                    .or_insert_with(|| ImportCommuneSummary {
                        commune_id: target.id,
                        code: target.code.clone(),
                        nom: target.nom.clone(),
                        created: 0,
                        updated: 0,
                        restored: 0,
                    });
                match outcome {
                    RowOutcome::Created => {
                        created += 1;
                        entry.created += 1;
                    }
                    RowOutcome::Updated => {
                        updated += 1;
                        entry.updated += 1;
                    }
                    RowOutcome::Restored => {
                        restored += 1;
                        entry.restored += 1;
                    }
                }
                if let Some((agent_id, current_commune, _)) = existing {
                    if current_commune != target.id {
                        transferred += 1;
                        transfers.push((agent_id, current_commune, target.id));
                    }
                }
            }
            Err(error) => {
                savepoint.rollback().await?;
                skipped += 1;
                csv_import::push_error(&mut errors, &mut error_count, row.line, error.message());
            }
        }
    }

    if !dry_run {
        // Un transfert inter-communes est un acte explicite : il est tracé agent par agent.
        for (agent_id, from_commune, to_commune) in &transfers {
            audit::record_for_commune_tx(
                &mut tx,
                Some(*to_commune),
                Some(auth_user.id),
                "AGENTS_IMPORT_TRANSFER",
                "agents",
                Some(*agent_id),
                Some(json!({ "commune_id": from_commune })),
                Some(json!({ "commune_id": to_commune })),
                auth_user.ip_address.clone(),
                auth_user.user_agent.clone(),
            )
            .await;
        }
        for summary in per_commune.values() {
            audit::record_for_commune_tx(
                &mut tx,
                Some(summary.commune_id),
                Some(auth_user.id),
                "AGENTS_IMPORTED_CSV",
                "agents",
                None,
                None,
                Some(json!({
                    "created": summary.created,
                    "updated": summary.updated,
                    "restored": summary.restored,
                })),
                auth_user.ip_address.clone(),
                auth_user.user_agent.clone(),
            )
            .await;
        }
        tx.commit().await?;
    } else {
        // Simulation : le travail a réellement eu lieu, donc les compteurs sont exacts,
        // puis tout est annulé.
        tx.rollback().await?;
    }

    let mut communes: Vec<ImportCommuneSummary> = per_commune.into_values().collect();
    communes.sort_by(|a, b| a.nom.cmp(&b.nom));

    // En simulation, les comptes ont ete annules avec la transaction : restituer leurs mots
    // de passe laisserait croire a des identifiants utilisables.
    if dry_run {
        accounts.clear();
    }

    Ok(Json(ImportAgentsResponse {
        created,
        updated,
        skipped,
        restored,
        transferred,
        total_rows,
        error_count,
        errors_truncated: error_count > errors.len(),
        errors,
        dry_run,
        communes,
        accounts,
    }))
}

enum RowOutcome {
    Created,
    Updated,
    Restored,
}

/// Écrit une ligne. Les colonnes absentes du fichier sont préservées côté base
/// (`CASE WHEN $n THEN ... ELSE colonne END`).
async fn write_agent_row(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    row: &ParsedAgentRow,
    target: &CommuneRef,
    existing: Option<(Uuid, Uuid, bool)>,
    present: (bool, bool, bool),
    provision_accounts: bool,
) -> Result<(RowOutcome, Option<ProvisionedAccount>), ApiError> {
    let (has_date, has_phone, has_email) = present;
    let telephone = clean_optional(row.telephone.clone());
    let email = clean_optional(row.email.clone());

    match existing {
        Some((agent_id, _, is_deleted)) => {
            let sql = if is_deleted {
                // Le matricule est l'identité nationale de l'agent et l'index unique est
                // global : réimporter un matricule retiré signifie que l'agent est de
                // retour, pas qu'un second agent porte le même numéro.
                r#"
                UPDATE agents
                SET full_name = $2,
                    commune_id = $3,
                    status = 'ACTIF',
                    deleted_at = NULL,
                    date_prise_fonction = CASE WHEN $4 THEN $5 ELSE date_prise_fonction END,
                    telephone = CASE WHEN $6 THEN $7 ELSE telephone END,
                    email = CASE WHEN $8 THEN $9 ELSE email END,
                    updated_at = now()
                WHERE id = $1
                "#
            } else {
                r#"
                UPDATE agents
                SET full_name = $2,
                    commune_id = $3,
                    date_prise_fonction = CASE WHEN $4 THEN $5 ELSE date_prise_fonction END,
                    telephone = CASE WHEN $6 THEN $7 ELSE telephone END,
                    email = CASE WHEN $8 THEN $9 ELSE email END,
                    updated_at = now()
                WHERE id = $1 AND deleted_at IS NULL
                "#
            };
            sqlx::query(sql)
                .bind(agent_id)
                .bind(&row.full_name)
                .bind(target.id)
                .bind(has_date)
                .bind(row.date_prise_fonction)
                .bind(has_phone)
                .bind(telephone)
                .bind(has_email)
                .bind(email)
                .execute(&mut **tx)
                .await
                .map_err(map_database_error)?;
            // Sans effet si l'agent est deja rattache a un compte : une reimportation ne
            // reinitialise donc jamais un mot de passe en service.
            let account = if provision_accounts {
                provision_agent_account(tx, agent_id, &row.matricule, &row.full_name, target.id)
                    .await?
            } else {
                None
            };
            Ok((
                if is_deleted {
                    RowOutcome::Restored
                } else {
                    RowOutcome::Updated
                },
                account,
            ))
        }
        None => {
            let agent_id = Uuid::new_v4();
            sqlx::query(
                r#"
                INSERT INTO agents (
                    id, matricule, full_name, commune_id, status,
                    date_prise_fonction, telephone, email
                )
                VALUES ($1, $2, $3, $4, 'ACTIF', $5, $6, $7)
                "#,
            )
            .bind(agent_id)
            .bind(&row.matricule)
            .bind(&row.full_name)
            .bind(target.id)
            .bind(row.date_prise_fonction)
            .bind(telephone)
            .bind(email)
            .execute(&mut **tx)
            .await
            .map_err(map_database_error)?;
            let account = if provision_accounts {
                provision_agent_account(tx, agent_id, &row.matricule, &row.full_name, target.id)
                    .await?
            } else {
                None
            };
            Ok((RowOutcome::Created, account))
        }
    }
}

async fn patch_agent(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(agent_id): Path<Uuid>,
    ApiJson(payload): ApiJson<PatchAgentRequest>,
) -> Result<Json<AgentResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    let existing = load_agent(&state.db, agent_id).await?;
    auth_user.require_commune_access(existing.commune_id)?;
    let commune_id = payload.commune_id.unwrap_or(existing.commune_id);
    auth_user.require_commune_access(commune_id)?;
    let status = match payload.status {
        Some(status) => validate_agent_status(&status)?,
        None => existing.status.clone(),
    };

    sqlx::query(
        r#"
        UPDATE agents
        SET matricule = $2,
            full_name = $3,
            commune_id = $4,
            status = $5,
            date_prise_fonction = $6,
            photo_url = $7,
            telephone = $8,
            email = $9,
            updated_at = now()
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(agent_id)
    .bind(match payload.matricule {
        Some(value) => required_text(value, "matricule")?,
        None => existing.matricule.clone(),
    })
    .bind(match payload.full_name {
        Some(value) => required_text(value, "full_name")?,
        None => existing.full_name.clone(),
    })
    .bind(commune_id)
    .bind(&status)
    .bind(payload.date_prise_fonction.or(existing.date_prise_fonction))
    .bind(payload.photo_url.or(existing.photo_url.clone()))
    .bind(payload.telephone.or(existing.telephone.clone()))
    .bind(payload.email.or(existing.email.clone()))
    .execute(&state.db)
    .await
    .map_err(map_database_error)?;

    audit::record_for_commune(
        &state.db,
        Some(commune_id),
        Some(auth_user.id),
        "AGENT_UPDATED",
        "agents",
        Some(agent_id),
        Some(json!({ "status": existing.status, "commune_id": existing.commune_id })),
        Some(json!({ "status": status, "commune_id": commune_id })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok(Json(load_agent(&state.db, agent_id).await?))
}

async fn link_agent_mobile_account(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(agent_id): Path<Uuid>,
    ApiJson(payload): ApiJson<LinkAgentAccountRequest>,
) -> Result<Json<LinkAgentAccountResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    let agent = load_agent(&state.db, agent_id).await?;
    auth_user.require_commune_access(agent.commune_id)?;

    let email = normalize_email(&payload.email)?;
    let password_hash = hash_password(payload.password.trim())?;
    let active = payload.active.unwrap_or(true);

    let existing_user_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM users WHERE lower(email) = lower($1) AND deleted_at IS NULL",
    )
    .bind(&email)
    .fetch_optional(&state.db)
    .await?;
    let user_id = existing_user_id.unwrap_or_else(Uuid::new_v4);

    let conflicting_agent: Option<Uuid> = sqlx::query_scalar(
        r#"
        SELECT id
        FROM agents
        WHERE user_id = $1
          AND id <> $2
          AND deleted_at IS NULL
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .bind(agent_id)
    .fetch_optional(&state.db)
    .await?;
    if conflicting_agent.is_some() {
        return Err(ApiError::bad_request(
            "Ce compte utilisateur est deja lie a un autre agent",
        ));
    }

    let mut roles = match existing_user_id {
        Some(id) => roles_for_user(&state.db, id).await?,
        None => Vec::new(),
    };
    if !roles.contains(&Role::ApmAgent) {
        roles.push(Role::ApmAgent);
    }

    let mut tx = state.db.begin().await?;
    if existing_user_id.is_some() {
        sqlx::query(
            r#"
            UPDATE users
            SET email = $2,
                password_hash = $3,
                full_name = $4,
                commune_id = $5,
                active = $6,
                -- Mot de passe choisi par l'administrateur, donc connu d'un tiers :
                -- l'agent doit le remplacer a sa premiere connexion.
                must_change_password = TRUE,
                updated_at = now()
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(user_id)
        .bind(&email)
        .bind(&password_hash)
        .bind(&agent.full_name)
        .bind(agent.commune_id)
        .bind(active)
        .execute(&mut *tx)
        .await
        .map_err(map_database_error)?;
    } else {
        sqlx::query(
            r#"
            INSERT INTO users (
                id, email, password_hash, full_name, commune_id, active, must_change_password
            )
            VALUES ($1, $2, $3, $4, $5, $6, TRUE)
            "#,
        )
        .bind(user_id)
        .bind(&email)
        .bind(&password_hash)
        .bind(&agent.full_name)
        .bind(agent.commune_id)
        .bind(active)
        .execute(&mut *tx)
        .await
        .map_err(map_database_error)?;
    }

    assign_roles_in_tx(&mut tx, user_id, &roles).await?;
    sqlx::query(
        r#"
        UPDATE agents
        SET user_id = $2, updated_at = now()
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(agent_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(map_database_error)?;

    audit::record_for_commune_tx(
        &mut tx,
        Some(agent.commune_id),
        Some(auth_user.id),
        "AGENT_ACCOUNT_LINKED",
        "agents",
        Some(agent_id),
        None,
        Some(json!({ "email": email.clone(), "user_id": user_id })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;
    tx.commit().await?;

    Ok(Json(LinkAgentAccountResponse {
        agent_id,
        user_id,
        email,
        full_name: agent.full_name,
        commune_id: agent.commune_id,
        linked: true,
    }))
}

async fn suspend_agent(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(agent_id): Path<Uuid>,
) -> Result<Json<AgentResponse>, ApiError> {
    change_agent_status(&state, &auth_user, agent_id, "SUSPENDU", "AGENT_SUSPENDED").await
}

async fn reactivate_agent(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(agent_id): Path<Uuid>,
) -> Result<Json<AgentResponse>, ApiError> {
    change_agent_status(&state, &auth_user, agent_id, "ACTIF", "AGENT_REACTIVATED").await
}

async fn retire_agent(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(agent_id): Path<Uuid>,
) -> Result<Json<AgentResponse>, ApiError> {
    change_agent_status(&state, &auth_user, agent_id, "RETRAITE", "AGENT_RETIRED").await
}

async fn change_agent_status(
    state: &AppState,
    auth_user: &AuthUser,
    agent_id: Uuid,
    status: &'static str,
    action: &'static str,
) -> Result<Json<AgentResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    let existing = load_agent(&state.db, agent_id).await?;
    auth_user.require_commune_access(existing.commune_id)?;

    sqlx::query(
        r#"
        UPDATE agents
        SET status = $2, updated_at = now()
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(agent_id)
    .bind(status)
    .execute(&state.db)
    .await?;

    audit::record_for_commune(
        &state.db,
        Some(existing.commune_id),
        Some(auth_user.id),
        action,
        "agents",
        Some(agent_id),
        Some(json!({ "status": existing.status })),
        Some(json!({ "status": status })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok(Json(load_agent(&state.db, agent_id).await?))
}

async fn verify_agent_public(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(matricule): Path<String>,
) -> Result<Json<PublicAgentVerification>, ApiError> {
    state.rate_limiter.check(
        "public:agents:verify",
        &headers,
        state.config.rate_limit_public_max,
        state.config.rate_limit_window_seconds,
    )?;

    let matricule = required_text(matricule, "matricule")?;
    let row = sqlx::query(
        r#"
        SELECT
            a.matricule,
            a.full_name,
            a.status,
            a.photo_url,
            c.code AS commune_code,
            c.nom AS commune_nom
        FROM agents a
        INNER JOIN communes c ON c.id = a.commune_id
        WHERE lower(a.matricule) = lower($1)
          AND a.deleted_at IS NULL
          AND c.deleted_at IS NULL
        "#,
    )
    .bind(&matricule)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| ApiError::not_found("Agent introuvable"))?;

    let status: String = row.get("status");
    Ok(Json(PublicAgentVerification {
        matricule: row.get("matricule"),
        full_name: row.get("full_name"),
        commune_code: row.get("commune_code"),
        commune_nom: row.get("commune_nom"),
        active: status == "ACTIF",
        status,
        has_photo: row.get::<Option<String>, _>("photo_url").is_some(),
    }))
}

/// Streame l'avatar public d'un agent (sans authentification).
async fn verify_agent_photo_public(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(matricule): Path<String>,
) -> Result<Response, ApiError> {
    state.rate_limiter.check(
        "public:agents:photo",
        &headers,
        state.config.rate_limit_public_max,
        state.config.rate_limit_window_seconds,
    )?;

    let matricule = required_text(matricule, "matricule")?;
    let object_key: Option<String> = sqlx::query_scalar(
        r#"
        SELECT a.photo_url
        FROM agents a
        INNER JOIN communes c ON c.id = a.commune_id
        WHERE lower(a.matricule) = lower($1)
          AND a.deleted_at IS NULL
          AND c.deleted_at IS NULL
        "#,
    )
    .bind(&matricule)
    .fetch_optional(&state.db)
    .await?
    .flatten();

    serve_avatar(&state, object_key).await
}

// ─────────────────────────────────────────────────────────────────────────────
// Photo de profil (avatar, object storage MinIO/S3)
// ─────────────────────────────────────────────────────────────────────────────

/// Lit un champ image unique d'un corps multipart et valide type/taille.
pub(crate) async fn read_image_field(
    multipart: &mut Multipart,
) -> Result<(Bytes, String), ApiError> {
    let field = multipart
        .next_field()
        .await
        .map_err(|err| ApiError::bad_request(err.to_string()))?
        .ok_or_else(|| ApiError::bad_request("Fichier manquant"))?;

    let content_type = field
        .content_type()
        .map(|value| value.to_string())
        .unwrap_or_else(|| "application/octet-stream".to_string());
    if !content_type.starts_with("image/") {
        return Err(ApiError::bad_request("Le fichier doit etre une image"));
    }

    let data = field
        .bytes()
        .await
        .map_err(|err| ApiError::bad_request(err.to_string()))?;
    if data.is_empty() {
        return Err(ApiError::bad_request("Fichier vide"));
    }
    if data.len() > MAX_AVATAR_BYTES {
        return Err(ApiError::bad_request("Image trop volumineuse (max 5 Mo)"));
    }
    Ok((data, content_type))
}

/// Charge un objet avatar depuis le stockage et le renvoie en reponse HTTP.
pub(crate) async fn serve_avatar(
    state: &AppState,
    object_key: Option<String>,
) -> Result<Response, ApiError> {
    let object_key = object_key.ok_or_else(|| ApiError::not_found("Photo introuvable"))?;
    let storage = state
        .storage
        .as_ref()
        .ok_or_else(|| ApiError::internal("Stockage des photos non configure"))?;
    let bytes = storage.get(&object_key).await.map_err(|error| {
        tracing::error!(%error, "agent photo download failed");
        ApiError::internal("Echec du telechargement de la photo")
    })?;
    let content_type = content_type_for_key(&object_key);
    Ok(([(header::CONTENT_TYPE, content_type)], bytes).into_response())
}

async fn upload_agent_photo(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(agent_id): Path<Uuid>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<AgentResponse>), ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    let agent = load_agent(&state.db, agent_id).await?;
    auth_user.require_commune_access(agent.commune_id)?;

    let storage = state
        .storage
        .as_ref()
        .ok_or_else(|| ApiError::internal("Stockage des photos non configure"))?;

    let (data, content_type) = read_image_field(&mut multipart).await?;
    let object_key = format!("avatars/agents/{}.{}", agent_id, image_extension(&content_type));
    storage
        .put(&object_key, data.as_ref(), &content_type)
        .await
        .map_err(|error| {
            tracing::error!(%error, "agent photo upload failed");
            ApiError::internal("Echec de l'enregistrement de la photo")
        })?;

    sqlx::query("UPDATE agents SET photo_url = $2, updated_at = now() WHERE id = $1 AND deleted_at IS NULL")
        .bind(agent_id)
        .bind(&object_key)
        .execute(&state.db)
        .await
        .map_err(map_database_error)?;

    audit::record_for_commune(
        &state.db,
        Some(agent.commune_id),
        Some(auth_user.id),
        "AGENT_PHOTO_UPLOADED",
        "agents",
        Some(agent_id),
        None,
        Some(json!({ "content_type": content_type })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok((
        StatusCode::CREATED,
        Json(load_agent(&state.db, agent_id).await?),
    ))
}

async fn get_agent_photo_content(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(agent_id): Path<Uuid>,
) -> Result<Response, ApiError> {
    auth_user.require_any_role(&[
        Role::SuperAdmin,
        Role::AdminCommune,
        Role::ApmAgent,
        Role::Superviseur,
        Role::Receveur,
    ])?;
    let agent = load_agent(&state.db, agent_id).await?;
    auth_user.require_commune_access(agent.commune_id)?;
    serve_avatar(&state, agent.photo_url).await
}

pub async fn load_agent(pool: &PgPool, agent_id: Uuid) -> Result<AgentResponse, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT *
        FROM agents
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(agent_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::not_found("Agent introuvable"))?;

    Ok(row_to_agent(row))
}

fn row_to_agent(row: sqlx::postgres::PgRow) -> AgentResponse {
    AgentResponse {
        id: row.get("id"),
        matricule: row.get("matricule"),
        full_name: row.get("full_name"),
        commune_id: row.get("commune_id"),
        status: row.get("status"),
        date_prise_fonction: row.get("date_prise_fonction"),
        has_photo: row.get::<Option<String>, _>("photo_url").is_some(),
        photo_url: row.get("photo_url"),
        telephone: row.get("telephone"),
        email: row.get("email"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        account: None,
    }
}

fn validate_agent_status(value: &str) -> Result<String, ApiError> {
    let status = value.trim().to_ascii_uppercase();
    if matches!(
        status.as_str(),
        "ACTIF" | "SUSPENDU" | "RETRAITE" | "INACTIF"
    ) {
        Ok(status)
    } else {
        Err(ApiError::bad_request(
            "Statut agent invalide. Valeurs acceptees: ACTIF, SUSPENDU, RETRAITE, INACTIF",
        ))
    }
}

fn required_text(value: String, field: &'static str) -> Result<String, ApiError> {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request(format!("{field} est requis")));
    }
    Ok(trimmed)
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|candidate| candidate.trim().to_string())
        .filter(|candidate| !candidate.is_empty())
}

fn normalize_email(value: &str) -> Result<String, ApiError> {
    let email = value.trim().to_ascii_lowercase();
    if email.is_empty() || !email.contains('@') {
        return Err(ApiError::bad_request("Email invalide"));
    }
    Ok(email)
}
