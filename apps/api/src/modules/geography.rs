//! Découpage administratif national : Région → Département → Arrondissement → Quartier.
//!
//! **Données globales, hors tenant.** Ces tables ne portent pas de `commune_id`, donc
//! `resolve_commune_filter()` / `require_commune_access()` — les deux primitives qui
//! rendent toute autre mutation sûre — sont inapplicables. Les **écritures sont donc
//! réservées au SUPER_ADMIN** : un ADMIN_COMMUNE renommant « Mfoundi » ou supprimant
//! « Wouri » modifierait une donnée que tous les autres tenants lisent. Les lectures
//! restent ouvertes aux cinq rôles (formulaire commune, mobile, cascade citoyenne).
//!
//! Pour la même raison, l'audit passe par `audit::record` (`commune_id` NULL) et non
//! `record_for_commune` : rattacher une donnée nationale à une commune arbitraire
//! fausserait le filtrage par tenant du journal d'audit.
//!
//! Les suppressions sont logiques et gardées par deux compteurs — les enfants **et** les
//! communes qui référencent l'entité — pour ne jamais orphaliner une commune.

use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::HeaderMap;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgPool, QueryBuilder, Row};
use std::collections::HashMap;
use uuid::Uuid;

use crate::csv_import::{self, ColumnSpec, RowError};
use crate::errors::{map_database_error, ApiError};
use crate::extractors::ApiJson;
use crate::helpers::{clean_optional, required_text, validate_optional_text_len, validate_text_len};
use crate::modules::audit;
use crate::modules::auth::AuthUser;
use crate::modules::rbac::Role;
use crate::pagination::{Paginated, Pagination, PaginationQuery};
use crate::state::AppState;

/// Prédicat SQL d'une commune visible publiquement (active + abonnement valide).
const VISIBLE_COMMUNE_PREDICATE: &str = "active = true \
    AND subscription_status IN ('ACTIVE', 'TRIAL') \
    AND (subscription_expires_at IS NULL OR subscription_expires_at >= now()) \
    AND deleted_at IS NULL";

/// Les référentiels nationaux doivent pouvoir être chargés d'un seul coup par les
/// sélecteurs du back-office (~360 arrondissements) ; les lignes sont minuscules.
const GEO_MAX_PAGE_SIZE: i64 = 1000;
const MAX_IMPORT_BYTES: usize = 8 * 1024 * 1024;
const MAX_IMPORT_ROWS: usize = 100_000;

/// Description d'un niveau de la hiérarchie. Les identifiants de tables/colonnes sont
/// des constantes du code (jamais des entrées utilisateur) : leur interpolation dans le
/// SQL est sûre, les valeurs restant systématiquement liées par paramètre.
struct Level {
    table: &'static str,
    entity_label: &'static str,
    parent_column: Option<&'static str>,
    parent_table: Option<&'static str>,
    parent_label: &'static str,
    /// Table enfant à compter avant une suppression logique.
    child_table: Option<&'static str>,
    child_column: Option<&'static str>,
    child_label: &'static str,
    /// Colonne de `communes` (ou `zones`) référençant ce niveau.
    referencing_table: &'static str,
    referencing_column: Option<&'static str>,
    referencing_label: &'static str,
    /// `code` obligatoire à la création (vrai uniquement pour les régions).
    code_required: bool,
}

const REGION: Level = Level {
    table: "regions",
    entity_label: "Region",
    parent_column: None,
    parent_table: None,
    parent_label: "",
    child_table: Some("departements"),
    child_column: Some("region_id"),
    child_label: "departements",
    referencing_table: "communes",
    referencing_column: Some("region_id"),
    referencing_label: "communes",
    code_required: true,
};

const DEPARTEMENT: Level = Level {
    table: "departements",
    entity_label: "Departement",
    parent_column: Some("region_id"),
    parent_table: Some("regions"),
    parent_label: "region",
    child_table: Some("arrondissements"),
    child_column: Some("departement_id"),
    child_label: "arrondissements",
    referencing_table: "communes",
    referencing_column: Some("departement_id"),
    referencing_label: "communes",
    code_required: false,
};

const ARRONDISSEMENT: Level = Level {
    table: "arrondissements",
    entity_label: "Arrondissement",
    parent_column: Some("departement_id"),
    parent_table: Some("departements"),
    parent_label: "departement",
    child_table: Some("quartiers"),
    child_column: Some("arrondissement_id"),
    child_label: "quartiers",
    referencing_table: "communes",
    referencing_column: Some("arrondissement_id"),
    referencing_label: "communes",
    code_required: false,
};

const QUARTIER: Level = Level {
    table: "quartiers",
    entity_label: "Quartier",
    parent_column: Some("arrondissement_id"),
    parent_table: Some("arrondissements"),
    parent_label: "arrondissement",
    child_table: None,
    child_column: None,
    child_label: "",
    referencing_table: "zones",
    referencing_column: Some("quartier_id"),
    referencing_label: "zones communales",
    code_required: false,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/geography/regions",
            axum::routing::get(list_regions).post(create_region),
        )
        .route(
            "/geography/regions/{id}",
            axum::routing::get(get_region)
                .patch(patch_region)
                .delete(delete_region),
        )
        .route(
            "/geography/departements",
            axum::routing::get(list_departements).post(create_departement),
        )
        .route(
            "/geography/departements/{id}",
            axum::routing::get(get_departement)
                .patch(patch_departement)
                .delete(delete_departement),
        )
        .route(
            "/geography/arrondissements",
            axum::routing::get(list_arrondissements).post(create_arrondissement),
        )
        .route(
            "/geography/arrondissements/{id}",
            axum::routing::get(get_arrondissement)
                .patch(patch_arrondissement)
                .delete(delete_arrondissement),
        )
        .route(
            "/geography/quartiers",
            axum::routing::get(list_quartiers).post(create_quartier),
        )
        .route(
            "/geography/quartiers/{id}",
            axum::routing::get(get_quartier)
                .patch(patch_quartier)
                .delete(delete_quartier),
        )
        .route(
            "/geography/import-template.csv",
            axum::routing::get(import_template),
        )
        .route(
            "/geography/import-csv",
            axum::routing::post(import_geography_csv)
                .layer(DefaultBodyLimit::max(MAX_IMPORT_BYTES)),
        )
}

/// Routes publiques (sans authentification) — cascade Région → Département →
/// Arrondissement → Commune pour le formulaire de signalement citoyen. Filtrées aux
/// entités contenant au moins une commune visible afin d'éviter les impasses.
pub fn public_router() -> Router<AppState> {
    Router::new()
        .route("/geography/regions", axum::routing::get(public_regions))
        .route(
            "/geography/regions/{id}/departements",
            axum::routing::get(public_departements),
        )
        .route(
            "/geography/departements/{id}/arrondissements",
            axum::routing::get(public_arrondissements),
        )
        .route(
            "/geography/departements/{id}/communes",
            axum::routing::get(public_communes),
        )
        .route(
            "/geography/arrondissements/{id}/communes",
            axum::routing::get(public_communes_by_arrondissement),
        )
}

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct GeoNode {
    id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    region_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    departement_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    arrondissement_id: Option<Uuid>,
    code: Option<String>,
    nom: String,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct GeoFilterQuery {
    page: Option<i64>,
    page_size: Option<i64>,
    search: Option<String>,
    region_id: Option<Uuid>,
    departement_id: Option<Uuid>,
    arrondissement_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct CreateGeoRequest {
    nom: String,
    code: Option<String>,
    region_id: Option<Uuid>,
    departement_id: Option<Uuid>,
    arrondissement_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct PatchGeoRequest {
    nom: Option<String>,
    code: Option<String>,
    region_id: Option<Uuid>,
    departement_id: Option<Uuid>,
    arrondissement_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
struct RegionOption {
    id: Uuid,
    code: String,
    nom: String,
}

#[derive(Debug, Serialize)]
struct DepartementOption {
    id: Uuid,
    region_id: Uuid,
    nom: String,
}

#[derive(Debug, Serialize)]
struct ArrondissementOption {
    id: Uuid,
    departement_id: Uuid,
    nom: String,
}

#[derive(Debug, Serialize)]
struct PublicCommuneOption {
    id: Uuid,
    code: String,
    nom: String,
    region: String,
    departement: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// CRUD générique
// ─────────────────────────────────────────────────────────────────────────────

const READ_ROLES: [Role; 5] = [
    Role::SuperAdmin,
    Role::AdminCommune,
    Role::ApmAgent,
    Role::Superviseur,
    Role::Receveur,
];

fn select_columns(level: &Level) -> String {
    match level.parent_column {
        Some(parent) => format!("id, {parent}, code, nom, created_at, updated_at"),
        None => "id, code, nom, created_at, updated_at".to_string(),
    }
}

fn row_to_node(level: &Level, row: &sqlx::postgres::PgRow) -> GeoNode {
    let parent: Option<Uuid> = level.parent_column.map(|column| row.get(column));
    GeoNode {
        id: row.get("id"),
        region_id: if level.parent_column == Some("region_id") {
            parent
        } else {
            None
        },
        departement_id: if level.parent_column == Some("departement_id") {
            parent
        } else {
            None
        },
        arrondissement_id: if level.parent_column == Some("arrondissement_id") {
            parent
        } else {
            None
        },
        code: row.get("code"),
        nom: row.get("nom"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

async fn list_level(
    state: &AppState,
    auth_user: &AuthUser,
    level: &Level,
    query: GeoFilterQuery,
) -> Result<Json<Paginated<GeoNode>>, ApiError> {
    auth_user.require_any_role(&READ_ROLES)?;

    let pagination = Pagination::from_query_with_max(
        PaginationQuery {
            page: query.page,
            page_size: query.page_size,
        },
        GEO_MAX_PAGE_SIZE,
    )?;

    let parent_filter = match level.parent_column {
        Some("region_id") => query.region_id,
        Some("departement_id") => query.departement_id,
        Some("arrondissement_id") => query.arrondissement_id,
        _ => None,
    };

    // Un référentiel de quartiers se compte en milliers : on refuse une page géante non
    // filtrée, qui n'aurait de toute façon aucun sens dans un sélecteur.
    if level.table == "quartiers"
        && pagination.page_size > 100
        && parent_filter.is_none()
        && query.departement_id.is_none()
        && query.search.as_deref().map(str::len).unwrap_or(0) < 2
    {
        return Err(ApiError::bad_request(
            "Filtrez les quartiers (arrondissement_id, departement_id ou search) au-dela de 100 elements",
        ));
    }

    let mut count_qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(format!(
        "SELECT COUNT(*) AS total FROM {} WHERE deleted_at IS NULL",
        level.table
    ));
    let mut qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(format!(
        "SELECT {} FROM {} WHERE deleted_at IS NULL",
        select_columns(level),
        level.table
    ));

    for builder in [&mut count_qb, &mut qb] {
        if let (Some(column), Some(value)) = (level.parent_column, parent_filter) {
            builder.push(format!(" AND {column} = ")).push_bind(value);
        }
        // Remontée d'un niveau : filtrer les arrondissements par région, ou les
        // quartiers par département, sans imposer le parent direct.
        if level.table == "arrondissements" {
            if let Some(region_id) = query.region_id {
                builder
                    .push(" AND departement_id IN (SELECT id FROM departements WHERE region_id = ")
                    .push_bind(region_id)
                    .push(")");
            }
        }
        if level.table == "quartiers" {
            if let Some(departement_id) = query.departement_id {
                builder
                    .push(" AND arrondissement_id IN (SELECT id FROM arrondissements WHERE departement_id = ")
                    .push_bind(departement_id)
                    .push(")");
            }
        }
        if let Some(search) = query.search.as_ref().map(|value| value.trim()) {
            if !search.is_empty() {
                let pattern = format!("%{search}%");
                builder
                    .push(" AND (nom ILIKE ")
                    .push_bind(pattern.clone())
                    .push(" OR code ILIKE ")
                    .push_bind(pattern)
                    .push(")");
            }
        }
    }

    let total: i64 = count_qb.build().fetch_one(&state.db).await?.get("total");
    qb.push(" ORDER BY nom LIMIT ")
        .push_bind(pagination.limit)
        .push(" OFFSET ")
        .push_bind(pagination.offset);
    let rows = qb.build().fetch_all(&state.db).await?;
    let items = rows.iter().map(|row| row_to_node(level, row)).collect();
    Ok(Json(Paginated::new(items, &pagination, total)))
}

async fn load_node(pool: &PgPool, level: &Level, id: Uuid) -> Result<GeoNode, ApiError> {
    let row = sqlx::query(&format!(
        "SELECT {} FROM {} WHERE id = $1 AND deleted_at IS NULL",
        select_columns(level),
        level.table
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::not_found(format!("{} introuvable", level.entity_label)))?;
    Ok(row_to_node(level, &row))
}

async fn ensure_parent_exists(
    pool: &PgPool,
    level: &Level,
    parent_id: Uuid,
) -> Result<(), ApiError> {
    let Some(parent_table) = level.parent_table else {
        return Ok(());
    };
    let exists: Option<Uuid> = sqlx::query_scalar(&format!(
        "SELECT id FROM {parent_table} WHERE id = $1 AND deleted_at IS NULL"
    ))
    .bind(parent_id)
    .fetch_optional(pool)
    .await?;
    exists
        .map(|_| ())
        .ok_or_else(|| ApiError::bad_request(format!("{} introuvable", level.parent_label)))
}

fn parent_from_payload(
    level: &Level,
    region_id: Option<Uuid>,
    departement_id: Option<Uuid>,
    arrondissement_id: Option<Uuid>,
) -> Option<Uuid> {
    match level.parent_column {
        Some("region_id") => region_id,
        Some("departement_id") => departement_id,
        Some("arrondissement_id") => arrondissement_id,
        _ => None,
    }
}

async fn create_level(
    state: &AppState,
    auth_user: &AuthUser,
    level: &Level,
    payload: CreateGeoRequest,
) -> Result<Json<GeoNode>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin])?;

    let nom = required_text(payload.nom, "nom")?;
    validate_text_len(&nom, "nom", 120)?;
    let code = clean_optional(payload.code);
    validate_optional_text_len(code.as_deref(), "code", 32)?;
    if level.code_required && code.is_none() {
        return Err(ApiError::bad_request("code est requis"));
    }

    let parent = parent_from_payload(
        level,
        payload.region_id,
        payload.departement_id,
        payload.arrondissement_id,
    );
    if level.parent_column.is_some() {
        let parent_id = parent.ok_or_else(|| {
            ApiError::bad_request(format!("{} est requis", level.parent_label))
        })?;
        ensure_parent_exists(&state.db, level, parent_id).await?;
    }

    let id = Uuid::new_v4();
    match (level.parent_column, parent) {
        (Some(column), Some(parent_id)) => {
            sqlx::query(&format!(
                "INSERT INTO {} (id, {column}, code, nom) VALUES ($1, $2, $3, $4)",
                level.table
            ))
            .bind(id)
            .bind(parent_id)
            .bind(&code)
            .bind(&nom)
            .execute(&state.db)
            .await
            .map_err(map_database_error)?;
        }
        _ => {
            sqlx::query(&format!(
                "INSERT INTO {} (id, code, nom) VALUES ($1, $2, $3)",
                level.table
            ))
            .bind(id)
            .bind(&code)
            .bind(&nom)
            .execute(&state.db)
            .await
            .map_err(map_database_error)?;
        }
    }

    let node = load_node(&state.db, level, id).await?;
    audit::record(
        &state.db,
        Some(auth_user.id),
        &format!("{}_CREATED", level.entity_label.to_uppercase()),
        level.table,
        Some(id),
        None,
        Some(json!({ "nom": nom, "code": code })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;
    Ok(Json(node))
}

async fn patch_level(
    state: &AppState,
    auth_user: &AuthUser,
    level: &Level,
    id: Uuid,
    payload: PatchGeoRequest,
) -> Result<Json<GeoNode>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin])?;
    let existing = load_node(&state.db, level, id).await?;

    let nom = match payload.nom {
        Some(value) => {
            let value = required_text(value, "nom")?;
            validate_text_len(&value, "nom", 120)?;
            value
        }
        None => existing.nom.clone(),
    };
    let code = match payload.code {
        Some(value) => {
            let cleaned = clean_optional(Some(value));
            validate_optional_text_len(cleaned.as_deref(), "code", 32)?;
            cleaned
        }
        None => existing.code.clone(),
    };
    let parent = parent_from_payload(
        level,
        payload.region_id,
        payload.departement_id,
        payload.arrondissement_id,
    );
    if let Some(parent_id) = parent {
        ensure_parent_exists(&state.db, level, parent_id).await?;
    }

    match (level.parent_column, parent) {
        (Some(column), Some(parent_id)) => {
            sqlx::query(&format!(
                "UPDATE {} SET nom = $2, code = $3, {column} = $4, updated_at = now() \
                 WHERE id = $1 AND deleted_at IS NULL",
                level.table
            ))
            .bind(id)
            .bind(&nom)
            .bind(&code)
            .bind(parent_id)
            .execute(&state.db)
            .await
            .map_err(map_database_error)?;
        }
        _ => {
            sqlx::query(&format!(
                "UPDATE {} SET nom = $2, code = $3, updated_at = now() \
                 WHERE id = $1 AND deleted_at IS NULL",
                level.table
            ))
            .bind(id)
            .bind(&nom)
            .bind(&code)
            .execute(&state.db)
            .await
            .map_err(map_database_error)?;
        }
    }

    let node = load_node(&state.db, level, id).await?;
    audit::record(
        &state.db,
        Some(auth_user.id),
        &format!("{}_UPDATED", level.entity_label.to_uppercase()),
        level.table,
        Some(id),
        Some(json!({ "nom": existing.nom, "code": existing.code })),
        Some(json!({ "nom": nom, "code": code })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;
    Ok(Json(node))
}

async fn delete_level(
    state: &AppState,
    auth_user: &AuthUser,
    level: &Level,
    id: Uuid,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin])?;
    let existing = load_node(&state.db, level, id).await?;

    // Deux gardes distinctes, avec deux messages distincts : l'opérateur doit savoir
    // *pourquoi* la suppression est refusée.
    if let (Some(child_table), Some(child_column)) = (level.child_table, level.child_column) {
        let children: i64 = sqlx::query_scalar(&format!(
            "SELECT COUNT(*) FROM {child_table} WHERE {child_column} = $1 AND deleted_at IS NULL"
        ))
        .bind(id)
        .fetch_one(&state.db)
        .await?;
        if children > 0 {
            return Err(ApiError::conflict(format!(
                "Impossible de supprimer: {} {} rattache(s)",
                children, level.child_label
            )));
        }
    }
    if let Some(column) = level.referencing_column {
        let referencing: i64 = sqlx::query_scalar(&format!(
            "SELECT COUNT(*) FROM {} WHERE {column} = $1 AND deleted_at IS NULL",
            level.referencing_table
        ))
        .bind(id)
        .fetch_one(&state.db)
        .await?;
        if referencing > 0 {
            return Err(ApiError::conflict(format!(
                "Impossible de supprimer: {} {} rattachee(s)",
                referencing, level.referencing_label
            )));
        }
    }

    sqlx::query(&format!(
        "UPDATE {} SET deleted_at = now() WHERE id = $1 AND deleted_at IS NULL",
        level.table
    ))
    .bind(id)
    .execute(&state.db)
    .await
    .map_err(map_database_error)?;

    audit::record(
        &state.db,
        Some(auth_user.id),
        &format!("{}_DELETED", level.entity_label.to_uppercase()),
        level.table,
        Some(id),
        Some(json!({ "nom": existing.nom, "code": existing.code })),
        None,
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;
    Ok(Json(json!({ "deleted": true, "id": id })))
}

macro_rules! geo_handlers {
    ($level:ident, $list:ident, $get:ident, $create:ident, $patch:ident, $delete:ident) => {
        async fn $list(
            State(state): State<AppState>,
            auth_user: AuthUser,
            Query(query): Query<GeoFilterQuery>,
        ) -> Result<Json<Paginated<GeoNode>>, ApiError> {
            list_level(&state, &auth_user, &$level, query).await
        }

        async fn $get(
            State(state): State<AppState>,
            auth_user: AuthUser,
            Path(id): Path<Uuid>,
        ) -> Result<Json<GeoNode>, ApiError> {
            auth_user.require_any_role(&READ_ROLES)?;
            Ok(Json(load_node(&state.db, &$level, id).await?))
        }

        async fn $create(
            State(state): State<AppState>,
            auth_user: AuthUser,
            ApiJson(payload): ApiJson<CreateGeoRequest>,
        ) -> Result<Json<GeoNode>, ApiError> {
            create_level(&state, &auth_user, &$level, payload).await
        }

        async fn $patch(
            State(state): State<AppState>,
            auth_user: AuthUser,
            Path(id): Path<Uuid>,
            ApiJson(payload): ApiJson<PatchGeoRequest>,
        ) -> Result<Json<GeoNode>, ApiError> {
            patch_level(&state, &auth_user, &$level, id, payload).await
        }

        async fn $delete(
            State(state): State<AppState>,
            auth_user: AuthUser,
            Path(id): Path<Uuid>,
        ) -> Result<Json<serde_json::Value>, ApiError> {
            delete_level(&state, &auth_user, &$level, id).await
        }
    };
}

geo_handlers!(
    REGION,
    list_regions,
    get_region,
    create_region,
    patch_region,
    delete_region
);
geo_handlers!(
    DEPARTEMENT,
    list_departements,
    get_departement,
    create_departement,
    patch_departement,
    delete_departement
);
geo_handlers!(
    ARRONDISSEMENT,
    list_arrondissements,
    get_arrondissement,
    create_arrondissement,
    patch_arrondissement,
    delete_arrondissement
);
geo_handlers!(
    QUARTIER,
    list_quartiers,
    get_quartier,
    create_quartier,
    patch_quartier,
    delete_quartier
);

// ─────────────────────────────────────────────────────────────────────────────
// Import du répertoire national
// ─────────────────────────────────────────────────────────────────────────────

const GEOGRAPHY_COLUMNS: &[ColumnSpec] = &[
    ColumnSpec {
        canonical: "region",
        aliases: &["nom_region", "libelle_region"],
        loose_aliases: &[],
        required: true,
    },
    ColumnSpec {
        canonical: "region_code",
        aliases: &["code_region"],
        loose_aliases: &[],
        required: false,
    },
    ColumnSpec {
        canonical: "departement",
        aliases: &["nom_departement", "libelle_departement"],
        loose_aliases: &[],
        required: true,
    },
    ColumnSpec {
        canonical: "departement_code",
        aliases: &["code_departement"],
        loose_aliases: &[],
        required: false,
    },
    ColumnSpec {
        canonical: "arrondissement",
        aliases: &["nom_arrondissement", "libelle_arrondissement"],
        loose_aliases: &[],
        required: true,
    },
    ColumnSpec {
        canonical: "arrondissement_code",
        aliases: &["code_arrondissement"],
        loose_aliases: &[],
        required: false,
    },
    ColumnSpec {
        canonical: "quartier",
        aliases: &["nom_quartier", "libelle_quartier"],
        loose_aliases: &[],
        required: false,
    },
    ColumnSpec {
        canonical: "quartier_code",
        aliases: &["code_quartier"],
        loose_aliases: &[],
        required: false,
    },
    ColumnSpec {
        canonical: "commune_code",
        aliases: &["code_commune", "code_commune_attache"],
        loose_aliases: &[],
        required: false,
    },
];

/// Gabarit remis au client : son fichier d'origine n'étant plus disponible, c'est ce
/// document qui définit le format attendu pour le rechargement des données réelles.
const IMPORT_TEMPLATE: &str = "region;region_code;departement;departement_code;arrondissement;arrondissement_code;quartier;quartier_code;commune_code\n\
Centre;CM-CE;Mfoundi;CE-MF;Yaounde Ier;CE-MF-01;Bastos;CE-MF-01-001;YDE1\n\
Centre;CM-CE;Mfoundi;CE-MF;Yaounde Ier;CE-MF-01;Nlongkak;CE-MF-01-002;YDE1\n\
Littoral;CM-LT;Wouri;LT-WO;Douala Ier;LT-WO-01;Bonanjo;LT-WO-01-001;DLA1\n\
Adamaoua;CM-AD;Vina;AD-VI;Ngaoundere Ier;AD-VI-01;;;\n";

#[derive(Debug, Serialize, Default)]
struct LevelCounts {
    created: usize,
    updated: usize,
    matched: usize,
}

#[derive(Debug, Serialize)]
pub struct GeographyImportReport {
    dry_run: bool,
    rows_total: usize,
    regions: LevelCounts,
    departements: LevelCounts,
    arrondissements: LevelCounts,
    quartiers: LevelCounts,
    communes_linked: usize,
    skipped: usize,
    errors_total: usize,
    errors: Vec<RowError>,
    errors_truncated: bool,
}

impl GeographyImportReport {
    /// Vrai si au moins une ligne a été rejetée (code de sortie non nul en CLI).
    pub fn has_errors(&self) -> bool {
        self.errors_total > 0
    }
}

#[derive(Debug, Deserialize)]
pub struct GeographyImportQuery {
    #[serde(default)]
    dry_run: Option<bool>,
}

async fn import_template() -> ([(axum::http::HeaderName, &'static str); 2], &'static str) {
    (
        [
            (axum::http::header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                axum::http::header::CONTENT_DISPOSITION,
                "attachment; filename=\"modele-decoupage-administratif.csv\"",
            ),
        ],
        IMPORT_TEMPLATE,
    )
}

async fn import_geography_csv(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<GeographyImportQuery>,
    body: axum::body::Bytes,
) -> Result<Json<GeographyImportReport>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin])?;
    let dry_run = query.dry_run.unwrap_or(false);
    let report = import_geography_hierarchy(&state.db, &body, dry_run, Some(auth_user.id)).await?;
    Ok(Json(report))
}

/// Cœur de l'import, appelé aussi bien par la route HTTP que par la sous-commande
/// `seed-geography` (déploiement initial, quand aucun SUPER_ADMIN n'existe encore).
///
/// **Création et mise à jour uniquement, jamais de suppression** : un fichier partiel ou
/// tronqué ne doit pas pouvoir effacer la carte nationale. Retirer une entité reste un
/// `DELETE` explicite via le CRUD.
pub async fn import_geography_hierarchy(
    pool: &PgPool,
    body: &[u8],
    dry_run: bool,
    actor: Option<Uuid>,
) -> Result<GeographyImportReport, ApiError> {
    let content = csv_import::decode(body);
    if content.trim().is_empty() {
        return Err(ApiError::bad_request("Le fichier CSV est vide"));
    }
    let delimiter = csv_import::detect_delimiter(&content);
    let mut reader = csv_import::reader(&content, delimiter);
    let headers = reader
        .headers()
        .map_err(|error| ApiError::bad_request(format!("En-tete CSV illisible: {error}")))?
        .clone();
    let columns = csv_import::resolve_columns(&headers, GEOGRAPHY_COLUMNS)?;

    let mut report = GeographyImportReport {
        dry_run,
        rows_total: 0,
        regions: LevelCounts::default(),
        departements: LevelCounts::default(),
        arrondissements: LevelCounts::default(),
        quartiers: LevelCounts::default(),
        communes_linked: 0,
        skipped: 0,
        errors_total: 0,
        errors: Vec::new(),
        errors_truncated: false,
    };

    // Caches (niveau, parent, nom) → id : un fichier de 15 000 lignes répète les mêmes
    // régions/départements des milliers de fois.
    let mut region_cache: HashMap<String, Uuid> = HashMap::new();
    let mut departement_cache: HashMap<(Uuid, String), Uuid> = HashMap::new();
    let mut arrondissement_cache: HashMap<(Uuid, String), Uuid> = HashMap::new();

    let mut tx = pool.begin().await?;

    for (index, record) in reader.records().enumerate() {
        let record = match record {
            Ok(record) => record,
            Err(error) => {
                report.skipped += 1;
                csv_import::push_error(
                    &mut report.errors,
                    &mut report.errors_total,
                    index + 2,
                    error.to_string(),
                );
                continue;
            }
        };
        if csv_import::is_blank(&record) {
            continue;
        }
        let line = record
            .position()
            .map(|position| position.line() as usize)
            .unwrap_or(index + 2);
        report.rows_total += 1;
        if report.rows_total > MAX_IMPORT_ROWS {
            return Err(ApiError::bad_request(format!(
                "Le fichier depasse la limite de {MAX_IMPORT_ROWS} lignes"
            )));
        }

        let Some(region_nom) = columns.get(&record, "region") else {
            report.skipped += 1;
            csv_import::push_error(
                &mut report.errors,
                &mut report.errors_total,
                line,
                "region est requise",
            );
            continue;
        };
        let Some(departement_nom) = columns.get(&record, "departement") else {
            report.skipped += 1;
            csv_import::push_error(
                &mut report.errors,
                &mut report.errors_total,
                line,
                "departement est requis",
            );
            continue;
        };
        let Some(arrondissement_nom) = columns.get(&record, "arrondissement") else {
            report.skipped += 1;
            csv_import::push_error(
                &mut report.errors,
                &mut report.errors_total,
                line,
                "arrondissement est requis",
            );
            continue;
        };

        // La région doit exister : une faute de frappe ne doit jamais créer une 11e région.
        let region_key = region_nom.to_lowercase();
        let region_id = match region_cache.get(&region_key) {
            Some(id) => *id,
            None => {
                let region_code = columns.get(&record, "region_code");
                let found: Option<Uuid> = sqlx::query_scalar(
                    "SELECT id FROM regions \
                     WHERE deleted_at IS NULL AND (lower(nom) = $1 OR lower(code) = COALESCE($2, '')) \
                     LIMIT 1",
                )
                .bind(&region_key)
                .bind(region_code.map(str::to_lowercase))
                .fetch_optional(&mut *tx)
                .await?;
                match found {
                    Some(id) => {
                        report.regions.matched += 1;
                        region_cache.insert(region_key.clone(), id);
                        id
                    }
                    None => {
                        report.skipped += 1;
                        csv_import::push_error(
                            &mut report.errors,
                            &mut report.errors_total,
                            line,
                            format!("region inconnue: '{region_nom}'"),
                        );
                        continue;
                    }
                }
            }
        };

        let departement_id = match upsert_child(
            &mut tx,
            "departements",
            "region_id",
            region_id,
            departement_nom,
            columns.get(&record, "departement_code"),
            &mut departement_cache,
            &mut report.departements,
        )
        .await
        {
            Ok(id) => id,
            Err(error) => {
                report.skipped += 1;
                csv_import::push_error(
                    &mut report.errors,
                    &mut report.errors_total,
                    line,
                    error.message(),
                );
                continue;
            }
        };

        let arrondissement_id = match upsert_child(
            &mut tx,
            "arrondissements",
            "departement_id",
            departement_id,
            arrondissement_nom,
            columns.get(&record, "arrondissement_code"),
            &mut arrondissement_cache,
            &mut report.arrondissements,
        )
        .await
        {
            Ok(id) => id,
            Err(error) => {
                report.skipped += 1;
                csv_import::push_error(
                    &mut report.errors,
                    &mut report.errors_total,
                    line,
                    error.message(),
                );
                continue;
            }
        };

        // Quartier facultatif : une ligne sans quartier ne crée que l'arrondissement.
        if let Some(quartier_nom) = columns.get(&record, "quartier") {
            let mut quartier_cache: HashMap<(Uuid, String), Uuid> = HashMap::new();
            if let Err(error) = upsert_child(
                &mut tx,
                "quartiers",
                "arrondissement_id",
                arrondissement_id,
                quartier_nom,
                columns.get(&record, "quartier_code"),
                &mut quartier_cache,
                &mut report.quartiers,
            )
            .await
            {
                report.skipped += 1;
                csv_import::push_error(
                    &mut report.errors,
                    &mut report.errors_total,
                    line,
                    error.message(),
                );
                continue;
            }
        }

        if let Some(commune_code) = columns.get(&record, "commune_code") {
            let updated = sqlx::query(
                "UPDATE communes SET arrondissement_id = $2, updated_at = now() \
                 WHERE lower(code) = lower($1) AND deleted_at IS NULL \
                   AND (arrondissement_id IS NULL OR arrondissement_id <> $2)",
            )
            .bind(commune_code)
            .bind(arrondissement_id)
            .execute(&mut *tx)
            .await
            .map_err(map_database_error)?;
            if updated.rows_affected() > 0 {
                report.communes_linked += 1;
            }
        }
    }

    report.errors_truncated = report.errors_total > report.errors.len();

    if dry_run {
        tx.rollback().await?;
    } else {
        audit::record_for_commune_tx(
            &mut tx,
            None,
            actor,
            "GEOGRAPHY_IMPORTED_CSV",
            "geography",
            None,
            None,
            Some(json!({
                "rows": report.rows_total,
                "arrondissements_created": report.arrondissements.created,
                "quartiers_created": report.quartiers.created,
                "communes_linked": report.communes_linked,
                "errors": report.errors_total,
            })),
            None,
            None,
        )
        .await;
        tx.commit().await?;
    }

    Ok(report)
}

/// Rapproche par code si fourni, sinon par nom ; met à jour le nom quand la
/// correspondance se fait par code (c'est ainsi qu'un renommage est appliqué lors d'un
/// import ultérieur, « si la carte administrative change »).
#[allow(clippy::too_many_arguments)]
async fn upsert_child(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    table: &str,
    parent_column: &str,
    parent_id: Uuid,
    nom: &str,
    code: Option<&str>,
    cache: &mut HashMap<(Uuid, String), Uuid>,
    counts: &mut LevelCounts,
) -> Result<Uuid, ApiError> {
    let key = (parent_id, nom.to_lowercase());
    if let Some(id) = cache.get(&key) {
        counts.matched += 1;
        return Ok(*id);
    }

    let by_code: Option<(Uuid, String)> = match code {
        Some(code) => sqlx::query_as(&format!(
            "SELECT id, nom FROM {table} WHERE lower(code) = lower($1) AND deleted_at IS NULL"
        ))
        .bind(code)
        .fetch_optional(&mut **tx)
        .await
        .map_err(map_database_error)?,
        None => None,
    };

    if let Some((id, current_nom)) = by_code {
        if current_nom.to_lowercase() != nom.to_lowercase() {
            sqlx::query(&format!(
                "UPDATE {table} SET nom = $2, {parent_column} = $3, updated_at = now() WHERE id = $1"
            ))
            .bind(id)
            .bind(nom)
            .bind(parent_id)
            .execute(&mut **tx)
            .await
            .map_err(map_database_error)?;
            counts.updated += 1;
        } else {
            counts.matched += 1;
        }
        cache.insert(key, id);
        return Ok(id);
    }

    let by_name: Option<Uuid> = sqlx::query_scalar(&format!(
        "SELECT id FROM {table} \
         WHERE {parent_column} = $1 AND lower(nom) = lower($2) AND deleted_at IS NULL"
    ))
    .bind(parent_id)
    .bind(nom)
    .fetch_optional(&mut **tx)
    .await
    .map_err(map_database_error)?;

    if let Some(id) = by_name {
        // Correspondance par nom avec un code désormais connu : on le renseigne.
        if let Some(code) = code {
            sqlx::query(&format!(
                "UPDATE {table} SET code = $2, updated_at = now() WHERE id = $1 AND code IS NULL"
            ))
            .bind(id)
            .bind(code)
            .execute(&mut **tx)
            .await
            .map_err(map_database_error)?;
        }
        counts.matched += 1;
        cache.insert(key, id);
        return Ok(id);
    }

    let id = Uuid::new_v4();
    sqlx::query(&format!(
        "INSERT INTO {table} (id, {parent_column}, code, nom) VALUES ($1, $2, $3, $4)"
    ))
    .bind(id)
    .bind(parent_id)
    .bind(code)
    .bind(nom)
    .execute(&mut **tx)
    .await
    .map_err(map_database_error)?;
    counts.created += 1;
    cache.insert(key, id);
    Ok(id)
}

// ─────────────────────────────────────────────────────────────────────────────
// Cascade publique (signalement citoyen)
// ─────────────────────────────────────────────────────────────────────────────

async fn public_regions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<RegionOption>>, ApiError> {
    rate_limit(&state, &headers, "public:geography:regions")?;

    let rows = sqlx::query(&format!(
        r#"
        SELECT r.id, r.code, r.nom
        FROM regions r
        WHERE r.deleted_at IS NULL
          AND EXISTS (
            SELECT 1 FROM communes c
            WHERE c.region_id = r.id AND {VISIBLE_COMMUNE_PREDICATE}
          )
        ORDER BY r.nom
        "#
    ))
    .fetch_all(&state.db)
    .await?;

    Ok(Json(rows.into_iter().map(row_to_region).collect()))
}

async fn public_departements(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(region_id): Path<Uuid>,
) -> Result<Json<Vec<DepartementOption>>, ApiError> {
    rate_limit(&state, &headers, "public:geography:departements")?;

    let rows = sqlx::query(&format!(
        r#"
        SELECT d.id, d.region_id, d.nom
        FROM departements d
        WHERE d.deleted_at IS NULL
          AND d.region_id = $1
          AND EXISTS (
            SELECT 1 FROM communes c
            WHERE c.departement_id = d.id AND {VISIBLE_COMMUNE_PREDICATE}
          )
        ORDER BY d.nom
        "#
    ))
    .bind(region_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(rows.into_iter().map(row_to_departement).collect()))
}

/// Étape facultative de la cascade : tant que le répertoire national n'est pas chargé,
/// `communes.arrondissement_id` est NULL partout et cette liste est simplement vide —
/// le formulaire citoyen continue de fonctionner via `/departements/{id}/communes`.
async fn public_arrondissements(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(departement_id): Path<Uuid>,
) -> Result<Json<Vec<ArrondissementOption>>, ApiError> {
    rate_limit(&state, &headers, "public:geography:arrondissements")?;

    let rows = sqlx::query(&format!(
        r#"
        SELECT a.id, a.departement_id, a.nom
        FROM arrondissements a
        WHERE a.deleted_at IS NULL
          AND a.departement_id = $1
          AND EXISTS (
            SELECT 1 FROM communes c
            WHERE c.arrondissement_id = a.id AND {VISIBLE_COMMUNE_PREDICATE}
          )
        ORDER BY a.nom
        "#
    ))
    .bind(departement_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(|row| ArrondissementOption {
                id: row.get("id"),
                departement_id: row.get("departement_id"),
                nom: row.get("nom"),
            })
            .collect(),
    ))
}

async fn public_communes(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(departement_id): Path<Uuid>,
) -> Result<Json<Vec<PublicCommuneOption>>, ApiError> {
    rate_limit(&state, &headers, "public:geography:communes")?;
    public_communes_by(&state, "departement_id", departement_id).await
}

async fn public_communes_by_arrondissement(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(arrondissement_id): Path<Uuid>,
) -> Result<Json<Vec<PublicCommuneOption>>, ApiError> {
    rate_limit(&state, &headers, "public:geography:communes")?;
    public_communes_by(&state, "arrondissement_id", arrondissement_id).await
}

async fn public_communes_by(
    state: &AppState,
    column: &str,
    value: Uuid,
) -> Result<Json<Vec<PublicCommuneOption>>, ApiError> {
    let rows = sqlx::query(&format!(
        r#"
        SELECT id, code, nom, region, departement
        FROM communes c
        WHERE c.{column} = $1 AND {VISIBLE_COMMUNE_PREDICATE}
        ORDER BY nom
        "#
    ))
    .bind(value)
    .fetch_all(&state.db)
    .await?;

    let options = rows
        .into_iter()
        .map(|row| PublicCommuneOption {
            id: row.get("id"),
            code: row.get("code"),
            nom: row.get("nom"),
            region: row.get("region"),
            departement: row.get("departement"),
        })
        .collect();

    Ok(Json(options))
}

fn rate_limit(state: &AppState, headers: &HeaderMap, key: &str) -> Result<(), ApiError> {
    state.rate_limiter.check(
        key,
        headers,
        state.config.rate_limit_public_max,
        state.config.rate_limit_window_seconds,
    )
}

fn row_to_region(row: sqlx::postgres::PgRow) -> RegionOption {
    RegionOption {
        id: row.get("id"),
        code: row.get("code"),
        nom: row.get("nom"),
    }
}

fn row_to_departement(row: sqlx::postgres::PgRow) -> DepartementOption {
    DepartementOption {
        id: row.get("id"),
        region_id: row.get("region_id"),
        nom: row.get("nom"),
    }
}
