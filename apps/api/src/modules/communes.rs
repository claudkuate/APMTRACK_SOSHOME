use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgPool, QueryBuilder, Row};
use uuid::Uuid;

use crate::errors::{map_database_error, ApiError};
use crate::extractors::ApiJson;
use crate::helpers::validate_geojson_polygon;
use crate::modules::audit;
use crate::modules::auth::AuthUser;
use crate::modules::rbac::Role;
use crate::pagination::{Paginated, Pagination, PaginationQuery};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/communes",
            axum::routing::get(list_communes).post(create_commune),
        )
        .route(
            "/communes/{id}",
            axum::routing::get(get_commune).patch(patch_commune),
        )
        .route(
            "/communes/{id}/subscription-payments",
            axum::routing::get(list_subscription_payments)
                .post(confirm_subscription_payment),
        )
        .route(
            "/communes/{id}/trial",
            axum::routing::post(start_subscription_trial),
        )
}

/// Routes publiques (sans authentification) — recherche de communes pour les
/// formulaires citoyens (ex. dépôt de signalement). N'expose qu'un sous-ensemble
/// minimal de colonnes, limité aux communes actives.
pub fn public_router() -> Router<AppState> {
    Router::new()
        .route("/communes", axum::routing::get(search_communes_public))
        .route(
            "/communes/{id}/signalement-options",
            axum::routing::get(public_signalement_options),
        )
}

#[derive(Debug, Deserialize)]
struct PublicCommuneSearchQuery {
    search: Option<String>,
    limit: Option<i64>,
}

#[derive(Debug, Serialize)]
struct PublicCommuneOption {
    id: Uuid,
    code: String,
    nom: String,
    region: String,
    departement: String,
}

#[derive(Debug, Serialize)]
struct PublicIncidentTypeOption {
    id: Uuid,
    nom: String,
    category_id: Uuid,
    category_nom: String,
}

#[derive(Debug, Serialize)]
struct PublicZoneOption {
    id: Uuid,
    nom: String,
    type_zone: String,
}

#[derive(Debug, Serialize)]
struct PublicSignalementOptions {
    incident_types: Vec<PublicIncidentTypeOption>,
    zones: Vec<PublicZoneOption>,
}

/// Recherche publique de communes actives par nom ou code (autocomplete citoyen).
async fn search_communes_public(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PublicCommuneSearchQuery>,
) -> Result<Json<Vec<PublicCommuneOption>>, ApiError> {
    state.rate_limiter.check(
        "public:communes:search",
        &headers,
        state.config.rate_limit_public_max,
        state.config.rate_limit_window_seconds,
    )?;

    let search = clean_optional(query.search);
    let limit = query.limit.unwrap_or(400).clamp(1, 400);

    let rows = sqlx::query(
        r#"
        SELECT id, code, nom, region, departement
        FROM communes
        WHERE deleted_at IS NULL
          AND commune_subscription_is_active(id, now())
          AND (
            $1::text IS NULL
            OR nom ILIKE '%' || $1 || '%'
            OR code ILIKE '%' || $1 || '%'
          )
        ORDER BY nom
        LIMIT $2
        "#,
    )
    .bind(search.as_deref())
    .bind(limit)
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

async fn public_signalement_options(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(commune_id): Path<Uuid>,
) -> Result<Json<PublicSignalementOptions>, ApiError> {
    state.rate_limiter.check(
        "public:communes:signalement-options",
        &headers,
        state.config.rate_limit_public_max,
        state.config.rate_limit_window_seconds,
    )?;

    ensure_public_commune_visible(&state.db, commune_id).await?;

    let type_rows = sqlx::query(
        r#"
        SELECT
            it.id,
            it.nom,
            it.category_id,
            c.nom AS category_nom
        FROM intervention_types it
        INNER JOIN intervention_categories c ON c.id = it.category_id
        WHERE it.commune_id = $1
          AND it.deleted_at IS NULL
          AND c.deleted_at IS NULL
          AND it.active = true
          AND c.active = true
        ORDER BY c.nom, it.nom
        "#,
    )
    .bind(commune_id)
    .fetch_all(&state.db)
    .await?;

    let zone_rows = sqlx::query(
        r#"
        SELECT id, nom, type_zone
        FROM zones
        WHERE commune_id = $1
          AND deleted_at IS NULL
          AND active = true
        ORDER BY nom
        "#,
    )
    .bind(commune_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(PublicSignalementOptions {
        incident_types: type_rows
            .into_iter()
            .map(|row| PublicIncidentTypeOption {
                id: row.get("id"),
                nom: row.get("nom"),
                category_id: row.get("category_id"),
                category_nom: row.get("category_nom"),
            })
            .collect(),
        zones: zone_rows
            .into_iter()
            .map(|row| PublicZoneOption {
                id: row.get("id"),
                nom: row.get("nom"),
                type_zone: row.get("type_zone"),
            })
            .collect(),
    }))
}

/// Filtres de `GET /communes`. Les filtres géographiques alimentent les panneaux
/// « Communes » des pages de détail Région / Département / Arrondissement.
#[derive(Debug, Deserialize)]
struct CommuneFilterQuery {
    page: Option<i64>,
    page_size: Option<i64>,
    region_id: Option<Uuid>,
    departement_id: Option<Uuid>,
    arrondissement_id: Option<Uuid>,
    active: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CreateCommuneRequest {
    code: String,
    nom: String,
    /// Région en texte (rétro-compatible) — résolue depuis `region_id` si absente.
    region: Option<String>,
    /// Département en texte (rétro-compatible) — résolu depuis `departement_id` si absent.
    departement: Option<String>,
    /// Référence vers la table `regions` (cascade géographique).
    region_id: Option<Uuid>,
    /// Référence vers la table `departements` (cascade géographique).
    departement_id: Option<Uuid>,
    /// Référence vers la table `arrondissements`. Renseigner ce seul champ suffit :
    /// le trigger `communes_link_geography` remonte département puis région.
    arrondissement_id: Option<Uuid>,
    adresse: Option<String>,
    telephone: Option<String>,
    email: Option<String>,
    site_web: Option<String>,
    logo_url: Option<String>,
    theme_color: Option<String>,
    active: Option<bool>,
    subscription_status: Option<String>,
    subscription_started_at: Option<DateTime<Utc>>,
    subscription_expires_at: Option<DateTime<Utc>>,
    /// Contour GeoJSON (Polygon ou MultiPolygon) optionnel.
    boundary: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct PatchCommuneRequest {
    code: Option<String>,
    nom: Option<String>,
    region: Option<String>,
    departement: Option<String>,
    region_id: Option<Uuid>,
    departement_id: Option<Uuid>,
    arrondissement_id: Option<Uuid>,
    adresse: Option<String>,
    telephone: Option<String>,
    email: Option<String>,
    site_web: Option<String>,
    logo_url: Option<String>,
    theme_color: Option<String>,
    active: Option<bool>,
    subscription_status: Option<String>,
    subscription_started_at: Option<DateTime<Utc>>,
    subscription_expires_at: Option<DateTime<Utc>>,
    /// Contour GeoJSON (Polygon ou MultiPolygon) optionnel — remplace l'existant si fourni.
    boundary: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct ConfirmSubscriptionPaymentRequest {
    payment_reference: String,
    amount_fcfa: i64,
    paid_at: DateTime<Utc>,
    period_started_at: DateTime<Utc>,
    period_expires_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct StartSubscriptionTrialRequest {
    period_started_at: DateTime<Utc>,
    period_expires_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct SubscriptionPaymentResponse {
    id: Uuid,
    commune_id: Uuid,
    payment_reference: String,
    amount_fcfa: i64,
    paid_at: DateTime<Utc>,
    period_started_at: DateTime<Utc>,
    period_expires_at: DateTime<Utc>,
    confirmed_at: DateTime<Utc>,
    confirmed_by_user_id: Uuid,
    confirmed_by_full_name: Option<String>,
    confirmed_by_email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SubscriptionPaymentQuery {
    page: Option<i64>,
    page_size: Option<i64>,
}

#[derive(Debug, Serialize, Clone)]
pub struct CommuneResponse {
    id: Uuid,
    code: String,
    nom: String,
    region: String,
    departement: String,
    region_id: Option<Uuid>,
    departement_id: Option<Uuid>,
    arrondissement_id: Option<Uuid>,
    adresse: Option<String>,
    telephone: Option<String>,
    email: Option<String>,
    site_web: Option<String>,
    logo_url: Option<String>,
    theme_color: Option<String>,
    active: bool,
    subscription_status: String,
    subscription_started_at: Option<DateTime<Utc>>,
    subscription_expires_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing)]
    subscription_legacy_access_until: Option<DateTime<Utc>>,
    /// Droit confirme non expire, eventuellement non commence ou suspendu.
    /// `subscription_active` reste l'unique indicateur d'acces effectif.
    subscription_entitlement_current: bool,
    subscription_active: bool,
    public_visible: bool,
    /// Contour GeoJSON (MultiPolygon) ou null.
    boundary: Option<serde_json::Value>,
    /// Centre GeoJSON (Point) calculé depuis le contour, ou null.
    centre: Option<serde_json::Value>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

/// Colonnes exposées par les SELECT (le contour est converti en GeoJSON texte).
const COMMUNE_COLUMNS: &str = "id, code, nom, region, departement, region_id, departement_id, \
    arrondissement_id, \
    adresse, telephone, email, \
    site_web, logo_url, theme_color, active, subscription_status, subscription_started_at, \
    subscription_expires_at, subscription_legacy_access_until, \
    commune_subscription_entitlement_is_current(id, now()) AS subscription_entitlement_current, \
    commune_subscription_is_active(id, now()) AS subscription_active, \
    commune_subscription_is_active(id, now()) AS public_visible, \
    ST_AsGeoJSON(boundary) AS boundary_geojson, ST_AsGeoJSON(centre) AS centre_geojson, \
    created_at, updated_at";

async fn get_commune(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(commune_id): Path<Uuid>,
) -> Result<Json<CommuneResponse>, ApiError> {
    auth_user.require_any_role(&[
        Role::SuperAdmin,
        Role::AdminCommune,
        Role::ApmAgent,
        Role::Superviseur,
        Role::Receveur,
    ])?;
    let commune = load_commune(&state.db, commune_id).await?;
    auth_user.require_commune_access(commune.id)?;
    Ok(Json(commune))
}

async fn list_communes(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<CommuneFilterQuery>,
) -> Result<Json<Paginated<CommuneResponse>>, ApiError> {
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

    let (rows, total) = if auth_user.has_role(Role::SuperAdmin)
        || (auth_user.has_role(Role::Superviseur) && auth_user.commune_id.is_none())
    {
        // Filtres géographiques : la page de détail d'une région/département/
        // arrondissement liste ses communes via `?<foreignKey>=<id>`. Sans filtrage
        // serveur, elle recevrait les 100 premières communes du pays.
        let mut count_qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
            "SELECT COUNT(*) AS total FROM communes WHERE deleted_at IS NULL",
        );
        let mut qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(format!(
            "SELECT {COMMUNE_COLUMNS} FROM communes WHERE deleted_at IS NULL"
        ));
        for builder in [&mut count_qb, &mut qb] {
            if let Some(value) = query.region_id {
                builder.push(" AND region_id = ").push_bind(value);
            }
            if let Some(value) = query.departement_id {
                builder.push(" AND departement_id = ").push_bind(value);
            }
            if let Some(value) = query.arrondissement_id {
                builder.push(" AND arrondissement_id = ").push_bind(value);
            }
            if let Some(value) = query.active {
                builder.push(" AND active = ").push_bind(value);
            }
        }
        let total: i64 = count_qb.build().fetch_one(&state.db).await?.get("total");
        qb.push(" ORDER BY created_at DESC LIMIT ")
            .push_bind(pagination.limit)
            .push(" OFFSET ")
            .push_bind(pagination.offset);
        let rows = qb.build().fetch_all(&state.db).await?;
        (rows, total)
    } else {
        let commune_id = auth_user
            .commune_id
            .ok_or_else(|| ApiError::forbidden("Utilisateur non rattache a une commune"))?;
        let total = sqlx::query(
            "SELECT COUNT(*) AS total FROM communes WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(commune_id)
        .fetch_one(&state.db)
        .await?
        .get("total");
        let rows = sqlx::query(&format!(
            r#"
            SELECT {COMMUNE_COLUMNS}
            FROM communes
            WHERE id = $1 AND deleted_at IS NULL
            LIMIT $2 OFFSET $3
            "#
        ))
        .bind(commune_id)
        .bind(pagination.limit)
        .bind(pagination.offset)
        .fetch_all(&state.db)
        .await?;
        (rows, total)
    };

    let communes = rows.into_iter().map(row_to_commune).collect::<Vec<_>>();
    Ok(Json(Paginated::new(communes, &pagination, total)))
}

async fn create_commune(
    State(state): State<AppState>,
    auth_user: AuthUser,
    ApiJson(payload): ApiJson<CreateCommuneRequest>,
) -> Result<Json<CommuneResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin])?;

    let boundary_json = prepare_boundary(payload.boundary)?;
    if payload.active == Some(true)
        || payload
            .subscription_status
            .as_deref()
            .is_some_and(|status| !status.eq_ignore_ascii_case("SUSPENDED"))
        || payload.subscription_started_at.is_some()
        || payload.subscription_expires_at.is_some()
    {
        return Err(ApiError::bad_request(
            "Une nouvelle commune doit etre activee par un paiement confirme ou une periode d'essai",
        ));
    }
    // region/departement peuvent être fournis en texte OU via leurs identifiants
    // (cascade géographique) — le trigger `communes_link_geography` réconcilie.
    let region = clean_optional(payload.region);
    let departement = clean_optional(payload.departement);
    if region.is_none() && payload.region_id.is_none() {
        return Err(ApiError::bad_request("region est requis"));
    }
    if departement.is_none() && payload.departement_id.is_none() {
        return Err(ApiError::bad_request("departement est requis"));
    }
    let commune_id = Uuid::new_v4();
    // $18 (contour GeoJSON) alimente `boundary` (forcé MultiPolygon) et le `centre` (centroïde).
    sqlx::query(
        r#"
        INSERT INTO communes (
            id, code, nom, region, departement, region_id, departement_id,
            adresse, telephone, email, site_web, logo_url, theme_color, active,
            subscription_status, subscription_started_at, subscription_expires_at,
            boundary, centre, arrondissement_id
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7,
            $8, $9, $10, $11, $12, $13, $14,
            $15, $16, $17,
            ST_Multi(ST_SetSRID(ST_GeomFromGeoJSON($18), 4326)),
            ST_Centroid(ST_SetSRID(ST_GeomFromGeoJSON($18), 4326)),
            $19
        )
        "#,
    )
    .bind(commune_id)
    .bind(required_text(payload.code, "code")?)
    .bind(required_text(payload.nom, "nom")?)
    .bind(region)
    .bind(departement)
    .bind(payload.region_id)
    .bind(payload.departement_id)
    .bind(clean_optional(payload.adresse))
    .bind(clean_optional(payload.telephone))
    .bind(clean_optional(payload.email))
    .bind(clean_optional(payload.site_web))
    .bind(clean_optional(payload.logo_url))
    .bind(clean_optional(payload.theme_color))
    .bind(false)
    .bind("SUSPENDED")
    .bind(Option::<DateTime<Utc>>::None)
    .bind(Option::<DateTime<Utc>>::None)
    .bind(&boundary_json)
    .bind(payload.arrondissement_id)
    .execute(&state.db)
    .await
    .map_err(map_database_error)?;

    audit::record_for_commune(
        &state.db,
        Some(commune_id),
        Some(auth_user.id),
        "COMMUNE_CREATED",
        "communes",
        Some(commune_id),
        None,
        Some(json!({ "id": commune_id })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok(Json(load_commune(&state.db, commune_id).await?))
}

async fn patch_commune(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(commune_id): Path<Uuid>,
    ApiJson(payload): ApiJson<PatchCommuneRequest>,
) -> Result<Json<CommuneResponse>, ApiError> {
    let is_super_admin = auth_user.has_role(Role::SuperAdmin);
    if !is_super_admin {
        auth_user.require_any_role(&[Role::AdminCommune])?;
        auth_user.require_commune_access(commune_id)?;
    }

    let existing = load_commune(&state.db, commune_id).await?;
    let code = payload.code.map_or(Ok(existing.code.clone()), |value| {
        required_text(value, "code")
    })?;
    let nom = payload.nom.map_or(Ok(existing.nom.clone()), |value| {
        required_text(value, "nom")
    })?;
    // Identifiants géographiques : nouvelle valeur fournie, sinon l'existant.
    let region_id = payload.region_id.or(existing.region_id);
    let departement_id = payload.departement_id.or(existing.departement_id);
    // Texte : explicite > recalcul par le trigger (None) si un nouvel id arrive > existant.
    let region: Option<String> = match payload.region {
        Some(value) => Some(required_text(value, "region")?),
        None if payload.region_id.is_some() => None,
        None => Some(existing.region.clone()),
    };
    let departement: Option<String> = match payload.departement {
        Some(value) => Some(required_text(value, "departement")?),
        None if payload.departement_id.is_some() => None,
        None => Some(existing.departement.clone()),
    };
    // Un PATCH generique ne peut jamais creer ou prolonger un droit d'acces.
    // Ces transitions passent exclusivement par les endpoints paiement/essai.
    if payload.subscription_status.is_some()
        || payload.subscription_started_at.is_some()
        || payload.subscription_expires_at.is_some()
    {
        return Err(ApiError::bad_request(
            "L'abonnement se modifie uniquement par confirmation de paiement ou activation d'un essai",
        ));
    }
    if is_super_admin && payload.active == Some(true) && !existing.active {
        return Err(ApiError::bad_request(
            "La commune doit etre activee par un paiement confirme ou une periode d'essai",
        ));
    }
    let active = if is_super_admin && payload.active == Some(false) {
        false
    } else {
        existing.active
    };
    let subscription_status = if !active {
        "SUSPENDED".to_string()
    } else {
        existing.subscription_status.clone()
    };
    let subscription_started_at = existing.subscription_started_at;
    let subscription_expires_at = existing.subscription_expires_at;
    // L'interrupteur administratif coupe l'acces sans effacer un essai deja
    // accorde. Le bridge conserve seulement le droit confirme non expire : il
    // bloque un nouvel essai et impose un renouvellement continu.
    let subscription_legacy_access_until = if existing.active
        && !active
        && existing.subscription_status == "TRIAL"
        && existing.subscription_entitlement_current
    {
        match (
            existing.subscription_legacy_access_until,
            subscription_expires_at,
        ) {
            (Some(legacy), Some(expires_at)) => Some(legacy.max(expires_at)),
            (None, expires_at) => expires_at,
            (legacy, None) => legacy,
        }
    } else {
        existing.subscription_legacy_access_until
    };
    let boundary_json = prepare_boundary(payload.boundary)?;

    // Le contour ($18) n'est mis à jour que s'il est fourni (COALESCE conserve l'existant sinon).
    sqlx::query(
        r#"
        UPDATE communes
        SET code = $2,
            nom = $3,
            region = $4,
            departement = $5,
            region_id = $6,
            departement_id = $7,
            adresse = $8,
            telephone = $9,
            email = $10,
            site_web = $11,
            logo_url = $12,
            theme_color = $13,
            active = $14,
            subscription_status = $15,
            subscription_started_at = $16,
            subscription_expires_at = $17,
            boundary = COALESCE(ST_Multi(ST_SetSRID(ST_GeomFromGeoJSON($18), 4326)), boundary),
            centre = COALESCE(ST_Centroid(ST_SetSRID(ST_GeomFromGeoJSON($18), 4326)), centre),
            arrondissement_id = COALESCE($19, arrondissement_id),
            subscription_legacy_access_until = $20,
            updated_at = now()
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(commune_id)
    .bind(&code)
    .bind(&nom)
    .bind(&region)
    .bind(&departement)
    .bind(region_id)
    .bind(departement_id)
    .bind(payload.adresse.or(existing.adresse.clone()))
    .bind(payload.telephone.or(existing.telephone.clone()))
    .bind(payload.email.or(existing.email.clone()))
    .bind(payload.site_web.or(existing.site_web.clone()))
    .bind(payload.logo_url.or(existing.logo_url.clone()))
    .bind(payload.theme_color.or(existing.theme_color.clone()))
    .bind(active)
    .bind(&subscription_status)
    .bind(subscription_started_at)
    .bind(subscription_expires_at)
    .bind(&boundary_json)
    .bind(payload.arrondissement_id)
    .bind(subscription_legacy_access_until)
    .execute(&state.db)
    .await
    .map_err(map_database_error)?;

    audit::record_for_commune(
        &state.db,
        Some(commune_id),
        Some(auth_user.id),
        if existing.active && !active {
            "COMMUNE_SUSPENDED"
        } else {
            "COMMUNE_UPDATED"
        },
        "communes",
        Some(commune_id),
        Some(json!({
            "code": existing.code,
            "nom": existing.nom,
            "active": existing.active,
            "subscription_status": existing.subscription_status,
            "subscription_started_at": existing.subscription_started_at,
            "subscription_expires_at": existing.subscription_expires_at,
            "subscription_legacy_access_until": existing.subscription_legacy_access_until,
        })),
        Some(json!({
            "code": code,
            "nom": nom,
            "active": active,
            "subscription_status": subscription_status,
            "subscription_started_at": subscription_started_at,
            "subscription_expires_at": subscription_expires_at,
            "subscription_legacy_access_until": subscription_legacy_access_until,
        })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok(Json(load_commune(&state.db, commune_id).await?))
}

async fn confirm_subscription_payment(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(commune_id): Path<Uuid>,
    ApiJson(payload): ApiJson<ConfirmSubscriptionPaymentRequest>,
) -> Result<Json<SubscriptionPaymentResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin])?;

    let payment_reference = required_text(payload.payment_reference, "payment_reference")?;
    if payment_reference.len() > 160 {
        return Err(ApiError::bad_request(
            "payment_reference ne doit pas depasser 160 caracteres",
        ));
    }
    if payload.amount_fcfa <= 0 {
        return Err(ApiError::bad_request(
            "amount_fcfa doit etre strictement positif",
        ));
    }
    let now = Utc::now();
    if payload.paid_at > now {
        return Err(ApiError::bad_request(
            "paid_at ne peut pas etre dans le futur",
        ));
    }
    validate_subscription_period(payload.period_started_at, payload.period_expires_at)?;
    if payload.period_started_at < payload.paid_at {
        return Err(ApiError::bad_request(
            "La periode d'abonnement ne peut pas commencer avant le paiement",
        ));
    }

    let mut transaction = state.db.begin().await?;
    let commune = sqlx::query(
        r#"
        SELECT active, subscription_status, subscription_started_at,
               subscription_expires_at, subscription_legacy_access_until,
               commune_subscription_entitlement_is_current(id, now()) AS entitlement_current
        FROM communes
        WHERE id = $1 AND deleted_at IS NULL
        FOR UPDATE
        "#,
    )
    .bind(commune_id)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or_else(|| ApiError::not_found("Commune introuvable"))?;

    let entitlement_current: bool = commune.get("entitlement_current");
    let current_started_at: Option<DateTime<Utc>> = commune.get("subscription_started_at");
    let current_expires_at: Option<DateTime<Utc>> = commune.get("subscription_expires_at");
    let current_status: String = commune.get("subscription_status");
    let current_active: bool = commune.get("active");
    let current_legacy_until: Option<DateTime<Utc>> =
        commune.get("subscription_legacy_access_until");

    if entitlement_current {
        let current_expires_at = current_expires_at.expect("active entitlement has an expiry");
        // `datetime-local` inputs are precise to the second while PostgreSQL may
        // retain sub-second precision. Values in the same second are one boundary.
        if payload
            .period_started_at
            .signed_duration_since(current_expires_at)
            .num_seconds()
            .abs()
            > 0
        {
            return Err(ApiError::bad_request(
                "Le renouvellement doit commencer exactement a l'echeance actuelle",
            ));
        }
    }

    let aggregate_started_at = if entitlement_current {
        current_started_at.unwrap_or(payload.period_started_at)
    } else {
        payload.period_started_at
    };
    // Lorsqu'un essai encore valide est converti en abonnement paye, cette
    // passerelle conserve l'acces jusqu'au debut contigu de la periode payee.
    let legacy_bridge_until = if entitlement_current && current_status == "TRIAL" {
        match (current_legacy_until, current_expires_at) {
            (Some(legacy), Some(expires)) => Some(legacy.max(expires)),
            (None, expires) => expires,
            (legacy, None) => legacy,
        }
    } else {
        current_legacy_until
    };

    let payment_id = Uuid::new_v4();
    let payment_row = sqlx::query(
        r#"
        INSERT INTO commune_subscription_payments (
            id, commune_id, payment_reference, amount_fcfa, paid_at,
            period_started_at, period_expires_at, confirmed_by_user_id
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id, commune_id, payment_reference, amount_fcfa, paid_at,
                  period_started_at, period_expires_at, confirmed_at,
                  confirmed_by_user_id
        "#,
    )
    .bind(payment_id)
    .bind(commune_id)
    .bind(&payment_reference)
    .bind(payload.amount_fcfa)
    .bind(payload.paid_at)
    .bind(payload.period_started_at)
    .bind(payload.period_expires_at)
    .bind(auth_user.id)
    .fetch_one(&mut *transaction)
    .await
    .map_err(map_database_error)?;

    sqlx::query(
        r#"
        UPDATE communes
        SET active = TRUE,
            subscription_status = 'ACTIVE',
            subscription_started_at = $2,
            subscription_expires_at = $3,
            subscription_legacy_access_until = $4,
            updated_at = now()
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(commune_id)
    .bind(aggregate_started_at)
    .bind(payload.period_expires_at)
    .bind(legacy_bridge_until)
    .execute(&mut *transaction)
    .await
    .map_err(map_database_error)?;

    audit::record_for_commune_tx(
        &mut transaction,
        Some(commune_id),
        Some(auth_user.id),
        "COMMUNE_SUBSCRIPTION_PAYMENT_CONFIRMED",
        "commune_subscription_payments",
        Some(payment_id),
        Some(json!({
            "active": current_active,
            "subscription_status": current_status,
            "subscription_started_at": current_started_at,
            "subscription_expires_at": current_expires_at,
            "subscription_legacy_access_until": current_legacy_until,
        })),
        Some(json!({
            "active": true,
            "subscription_status": "ACTIVE",
            "subscription_started_at": aggregate_started_at,
            "subscription_expires_at": payload.period_expires_at,
            "subscription_legacy_access_until": legacy_bridge_until,
            "payment_reference": payment_reference,
            "amount_fcfa": payload.amount_fcfa,
            "paid_at": payload.paid_at,
            "period_started_at": payload.period_started_at,
            "period_expires_at": payload.period_expires_at,
        })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;
    transaction.commit().await?;

    let mut response = row_to_subscription_payment(payment_row);
    response.confirmed_by_full_name = Some(auth_user.full_name);
    response.confirmed_by_email = Some(auth_user.email);
    Ok(Json(response))
}

async fn list_subscription_payments(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(commune_id): Path<Uuid>,
    Query(query): Query<SubscriptionPaymentQuery>,
) -> Result<Json<Paginated<SubscriptionPaymentResponse>>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin])?;
    let pagination = Pagination::from_query(PaginationQuery {
        page: query.page,
        page_size: query.page_size,
    })?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM communes WHERE id = $1 AND deleted_at IS NULL)",
    )
    .bind(commune_id)
    .fetch_one(&state.db)
    .await?;
    if !exists {
        return Err(ApiError::not_found("Commune introuvable"));
    }

    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM commune_subscription_payments WHERE commune_id = $1",
    )
    .bind(commune_id)
    .fetch_one(&state.db)
    .await?;
    let rows = sqlx::query(
        r#"
        SELECT sp.id, sp.commune_id, sp.payment_reference, sp.amount_fcfa, sp.paid_at,
               sp.period_started_at, sp.period_expires_at, sp.confirmed_at,
               sp.confirmed_by_user_id, u.full_name AS confirmed_by_full_name,
               u.email AS confirmed_by_email
        FROM commune_subscription_payments sp
        LEFT JOIN users u ON u.id = sp.confirmed_by_user_id
        WHERE sp.commune_id = $1
        ORDER BY sp.confirmed_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(commune_id)
    .bind(pagination.limit)
    .bind(pagination.offset)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(Paginated::new(
        rows.into_iter().map(row_to_subscription_payment).collect(),
        &pagination,
        total,
    )))
}

async fn start_subscription_trial(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(commune_id): Path<Uuid>,
    ApiJson(payload): ApiJson<StartSubscriptionTrialRequest>,
) -> Result<Json<CommuneResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin])?;
    validate_subscription_period(payload.period_started_at, payload.period_expires_at)?;
    if payload.period_expires_at < Utc::now() {
        return Err(ApiError::bad_request(
            "La periode d'essai doit expirer dans le futur",
        ));
    }

    let mut transaction = state.db.begin().await?;
    let row = sqlx::query(
        r#"
        SELECT active, subscription_status, subscription_started_at,
               subscription_expires_at, subscription_legacy_access_until,
               commune_subscription_entitlement_is_current(id, now()) AS entitlement_current
        FROM communes
        WHERE id = $1 AND deleted_at IS NULL
        FOR UPDATE
        "#,
    )
    .bind(commune_id)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or_else(|| ApiError::not_found("Commune introuvable"))?;
    if row.get::<bool, _>("entitlement_current") {
        return Err(ApiError::conflict(
            "Une commune ayant deja un abonnement valide ne peut pas passer en essai",
        ));
    }

    let old_value = json!({
        "active": row.get::<bool, _>("active"),
        "subscription_status": row.get::<String, _>("subscription_status"),
        "subscription_started_at": row.get::<Option<DateTime<Utc>>, _>("subscription_started_at"),
        "subscription_expires_at": row.get::<Option<DateTime<Utc>>, _>("subscription_expires_at"),
        "subscription_legacy_access_until": row.get::<Option<DateTime<Utc>>, _>("subscription_legacy_access_until"),
    });
    sqlx::query(
        r#"
        UPDATE communes
        SET active = TRUE,
            subscription_status = 'TRIAL',
            subscription_started_at = $2,
            subscription_expires_at = $3,
            subscription_legacy_access_until = NULL,
            updated_at = now()
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(commune_id)
    .bind(payload.period_started_at)
    .bind(payload.period_expires_at)
    .execute(&mut *transaction)
    .await
    .map_err(map_database_error)?;

    audit::record_for_commune_tx(
        &mut transaction,
        Some(commune_id),
        Some(auth_user.id),
        "COMMUNE_SUBSCRIPTION_TRIAL_STARTED",
        "communes",
        Some(commune_id),
        Some(old_value),
        Some(json!({
            "active": true,
            "subscription_status": "TRIAL",
            "subscription_started_at": payload.period_started_at,
            "subscription_expires_at": payload.period_expires_at,
            "subscription_legacy_access_until": null,
        })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;
    transaction.commit().await?;

    Ok(Json(load_commune(&state.db, commune_id).await?))
}

fn validate_subscription_period(
    started_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
) -> Result<(), ApiError> {
    if expires_at <= started_at {
        return Err(ApiError::bad_request(
            "La fin de la periode doit etre posterieure a son debut",
        ));
    }
    Ok(())
}

fn row_to_subscription_payment(row: sqlx::postgres::PgRow) -> SubscriptionPaymentResponse {
    SubscriptionPaymentResponse {
        id: row.get("id"),
        commune_id: row.get("commune_id"),
        payment_reference: row.get("payment_reference"),
        amount_fcfa: row.get("amount_fcfa"),
        paid_at: row.get("paid_at"),
        period_started_at: row.get("period_started_at"),
        period_expires_at: row.get("period_expires_at"),
        confirmed_at: row.get("confirmed_at"),
        confirmed_by_user_id: row.get("confirmed_by_user_id"),
        confirmed_by_full_name: row.try_get("confirmed_by_full_name").ok(),
        confirmed_by_email: row.try_get("confirmed_by_email").ok(),
    }
}

pub async fn load_commune(pool: &PgPool, commune_id: Uuid) -> Result<CommuneResponse, ApiError> {
    let row = sqlx::query(&format!(
        r#"
        SELECT {COMMUNE_COLUMNS}
        FROM communes
        WHERE id = $1 AND deleted_at IS NULL
        "#
    ))
    .bind(commune_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::not_found("Commune introuvable"))?;

    Ok(row_to_commune(row))
}

fn row_to_commune(row: sqlx::postgres::PgRow) -> CommuneResponse {
    CommuneResponse {
        id: row.get("id"),
        code: row.get("code"),
        nom: row.get("nom"),
        region: row.get("region"),
        departement: row.get("departement"),
        region_id: row.get("region_id"),
        departement_id: row.get("departement_id"),
        arrondissement_id: row.get("arrondissement_id"),
        adresse: row.get("adresse"),
        telephone: row.get("telephone"),
        email: row.get("email"),
        site_web: row.get("site_web"),
        logo_url: row.get("logo_url"),
        theme_color: row.get("theme_color"),
        active: row.get("active"),
        subscription_status: row.get("subscription_status"),
        subscription_started_at: row.get("subscription_started_at"),
        subscription_expires_at: row.get("subscription_expires_at"),
        subscription_legacy_access_until: row.get("subscription_legacy_access_until"),
        subscription_entitlement_current: row.get("subscription_entitlement_current"),
        subscription_active: row.get("subscription_active"),
        public_visible: row.get("public_visible"),
        boundary: row
            .get::<Option<String>, _>("boundary_geojson")
            .and_then(|s| serde_json::from_str(&s).ok()),
        centre: row
            .get::<Option<String>, _>("centre_geojson")
            .and_then(|s| serde_json::from_str(&s).ok()),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

/// Valide un contour GeoJSON optionnel et le sérialise en texte pour `ST_GeomFromGeoJSON`.
fn prepare_boundary(boundary: Option<serde_json::Value>) -> Result<Option<String>, ApiError> {
    match boundary {
        Some(value) if !value.is_null() => {
            validate_geojson_polygon(&value)?;
            Ok(Some(value.to_string()))
        }
        _ => Ok(None),
    }
}

fn required_text(value: String, field: &'static str) -> Result<String, ApiError> {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request(format!("{field} est requis")));
    }
    Ok(trimmed)
}

async fn ensure_public_commune_visible(pool: &PgPool, commune_id: Uuid) -> Result<(), ApiError> {
    let visible: Option<bool> = sqlx::query_scalar(
        r#"
        SELECT commune_subscription_is_active(id, now())
        FROM communes
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(commune_id)
    .fetch_optional(pool)
    .await?;

    match visible {
        Some(true) => Ok(()),
        Some(false) => Err(ApiError::forbidden("Commune non disponible")),
        None => Err(ApiError::not_found("Commune introuvable")),
    }
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|candidate| candidate.trim().to_string())
        .filter(|candidate| !candidate.is_empty())
}
