use axum::extract::{Path, Query, State};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgPool, QueryBuilder, Row};
use uuid::Uuid;

use crate::errors::{map_database_error, ApiError};
use crate::extractors::ApiJson;
use crate::helpers::resolve_commune_filter;
use crate::modules::audit;
use crate::modules::auth::AuthUser;
use crate::modules::rbac::Role;
use crate::pagination::{Paginated, Pagination, PaginationQuery};
use crate::state::AppState;

// ─────────────────────────────────────────────────────────────────────────────
// Router
// ─────────────────────────────────────────────────────────────────────────────

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/referentiel/categories",
            axum::routing::get(list_categories).post(create_category),
        )
        .route(
            "/referentiel/categories/{id}",
            axum::routing::get(get_category)
                .patch(patch_category)
                .delete(delete_category),
        )
        .route(
            "/referentiel/types",
            axum::routing::get(list_types).post(create_type),
        )
        .route(
            "/referentiel/types/{id}",
            axum::routing::get(get_type)
                .patch(patch_type)
                .delete(delete_type),
        )
        .route(
            "/referentiel/interventions",
            axum::routing::get(list_interventions).post(create_intervention),
        )
        .route(
            "/referentiel/interventions/{id}",
            axum::routing::get(get_intervention)
                .patch(patch_intervention)
                .delete(delete_intervention),
        )
}

// ─────────────────────────────────────────────────────────────────────────────
// Catégories
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct CommuneFilterQuery {
    page: Option<i64>,
    page_size: Option<i64>,
    commune_id: Option<Uuid>,
    active: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CreateCategoryRequest {
    commune_id: Uuid,
    nom: String,
    description: Option<String>,
    active: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct PatchCategoryRequest {
    nom: Option<String>,
    description: Option<String>,
    active: Option<bool>,
}

#[derive(Debug, Serialize, Clone)]
pub struct CategoryResponse {
    pub id: Uuid,
    pub commune_id: Uuid,
    pub nom: String,
    pub description: Option<String>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

async fn list_categories(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<CommuneFilterQuery>,
) -> Result<Json<Paginated<CategoryResponse>>, ApiError> {
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
    let commune_filter = resolve_commune_filter(&auth_user, query.commune_id)?;

    let mut count_qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
        "SELECT COUNT(*) AS total FROM intervention_categories WHERE deleted_at IS NULL",
    );
    apply_commune_active(&mut count_qb, commune_filter, query.active);
    let total: i64 = count_qb.build().fetch_one(&state.db).await?.get("total");

    let mut qb: QueryBuilder<sqlx::Postgres> =
        QueryBuilder::new("SELECT * FROM intervention_categories WHERE deleted_at IS NULL");
    apply_commune_active(&mut qb, commune_filter, query.active);
    qb.push(" ORDER BY nom ASC LIMIT ")
        .push_bind(pagination.limit)
        .push(" OFFSET ")
        .push_bind(pagination.offset);

    let rows = qb.build().fetch_all(&state.db).await?;
    let items = rows.into_iter().map(row_to_category).collect();
    Ok(Json(Paginated::new(items, &pagination, total)))
}

async fn get_category(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<CategoryResponse>, ApiError> {
    auth_user.require_any_role(&[
        Role::SuperAdmin,
        Role::AdminCommune,
        Role::ApmAgent,
        Role::Superviseur,
        Role::Receveur,
    ])?;
    let cat = load_category(&state.db, id).await?;
    auth_user.require_commune_access(cat.commune_id)?;
    Ok(Json(cat))
}

async fn create_category(
    State(state): State<AppState>,
    auth_user: AuthUser,
    ApiJson(payload): ApiJson<CreateCategoryRequest>,
) -> Result<Json<CategoryResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    auth_user.require_commune_access(payload.commune_id)?;

    let nom = required_text(payload.nom, "nom")?;
    let id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO intervention_categories (id, commune_id, nom, description, active)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(id)
    .bind(payload.commune_id)
    .bind(&nom)
    .bind(clean_optional(payload.description))
    .bind(payload.active.unwrap_or(true))
    .execute(&state.db)
    .await
    .map_err(map_database_error)?;

    audit::record_for_commune(
        &state.db,
        Some(payload.commune_id),
        Some(auth_user.id),
        "CATEGORY_CREATED",
        "intervention_categories",
        Some(id),
        None,
        Some(json!({ "nom": nom, "commune_id": payload.commune_id })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok(Json(load_category(&state.db, id).await?))
}

async fn patch_category(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
    ApiJson(payload): ApiJson<PatchCategoryRequest>,
) -> Result<Json<CategoryResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    let existing = load_category(&state.db, id).await?;
    auth_user.require_commune_access(existing.commune_id)?;

    let nom = match payload.nom {
        Some(v) => required_text(v, "nom")?,
        None => existing.nom.clone(),
    };

    sqlx::query(
        r#"
        UPDATE intervention_categories
        SET nom = $2, description = $3, active = $4, updated_at = now()
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .bind(&nom)
    .bind(payload.description.or(existing.description.clone()))
    .bind(payload.active.unwrap_or(existing.active))
    .execute(&state.db)
    .await
    .map_err(map_database_error)?;

    audit::record_for_commune(
        &state.db,
        Some(existing.commune_id),
        Some(auth_user.id),
        "CATEGORY_UPDATED",
        "intervention_categories",
        Some(id),
        Some(json!({ "nom": existing.nom })),
        Some(json!({ "nom": nom })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok(Json(load_category(&state.db, id).await?))
}

async fn delete_category(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    let existing = load_category(&state.db, id).await?;
    auth_user.require_commune_access(existing.commune_id)?;

    let children: i64 = sqlx::query(
        "SELECT COUNT(*) AS total FROM intervention_types WHERE category_id = $1 AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?
    .get("total");

    if children > 0 {
        return Err(ApiError::conflict(
            "Impossible de supprimer une categorie ayant des types associes",
        ));
    }

    sqlx::query(
        "UPDATE intervention_categories SET deleted_at = now() WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(id)
    .execute(&state.db)
    .await?;

    audit::record_for_commune(
        &state.db,
        Some(existing.commune_id),
        Some(auth_user.id),
        "CATEGORY_DELETED",
        "intervention_categories",
        Some(id),
        Some(json!({ "nom": existing.nom })),
        None,
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok(Json(json!({ "deleted": true, "id": id })))
}

pub async fn load_category(pool: &PgPool, id: Uuid) -> Result<CategoryResponse, ApiError> {
    let row =
        sqlx::query("SELECT * FROM intervention_categories WHERE id = $1 AND deleted_at IS NULL")
            .bind(id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| ApiError::not_found("Categorie introuvable"))?;
    Ok(row_to_category(row))
}

fn row_to_category(row: sqlx::postgres::PgRow) -> CategoryResponse {
    CategoryResponse {
        id: row.get("id"),
        commune_id: row.get("commune_id"),
        nom: row.get("nom"),
        description: row.get("description"),
        active: row.get("active"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Types d'intervention
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct TypeFilterQuery {
    page: Option<i64>,
    page_size: Option<i64>,
    commune_id: Option<Uuid>,
    category_id: Option<Uuid>,
    active: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CreateTypeRequest {
    commune_id: Uuid,
    category_id: Uuid,
    nom: String,
    description: Option<String>,
    active: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct PatchTypeRequest {
    nom: Option<String>,
    description: Option<String>,
    active: Option<bool>,
}

#[derive(Debug, Serialize, Clone)]
pub struct TypeResponse {
    pub id: Uuid,
    pub commune_id: Uuid,
    pub category_id: Uuid,
    pub nom: String,
    pub description: Option<String>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

async fn list_types(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<TypeFilterQuery>,
) -> Result<Json<Paginated<TypeResponse>>, ApiError> {
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
    let commune_filter = resolve_commune_filter(&auth_user, query.commune_id)?;

    let mut count_qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
        "SELECT COUNT(*) AS total FROM intervention_types WHERE deleted_at IS NULL",
    );
    apply_commune_active(&mut count_qb, commune_filter, query.active);
    if let Some(cat_id) = query.category_id {
        count_qb.push(" AND category_id = ").push_bind(cat_id);
    }
    let total: i64 = count_qb.build().fetch_one(&state.db).await?.get("total");

    let mut qb: QueryBuilder<sqlx::Postgres> =
        QueryBuilder::new("SELECT * FROM intervention_types WHERE deleted_at IS NULL");
    apply_commune_active(&mut qb, commune_filter, query.active);
    if let Some(cat_id) = query.category_id {
        qb.push(" AND category_id = ").push_bind(cat_id);
    }
    qb.push(" ORDER BY nom ASC LIMIT ")
        .push_bind(pagination.limit)
        .push(" OFFSET ")
        .push_bind(pagination.offset);

    let rows = qb.build().fetch_all(&state.db).await?;
    let items = rows.into_iter().map(row_to_type).collect();
    Ok(Json(Paginated::new(items, &pagination, total)))
}

async fn get_type(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<TypeResponse>, ApiError> {
    auth_user.require_any_role(&[
        Role::SuperAdmin,
        Role::AdminCommune,
        Role::ApmAgent,
        Role::Superviseur,
        Role::Receveur,
    ])?;
    let item = load_type(&state.db, id).await?;
    auth_user.require_commune_access(item.commune_id)?;
    Ok(Json(item))
}

async fn create_type(
    State(state): State<AppState>,
    auth_user: AuthUser,
    ApiJson(payload): ApiJson<CreateTypeRequest>,
) -> Result<Json<TypeResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    auth_user.require_commune_access(payload.commune_id)?;

    let category = load_category(&state.db, payload.category_id).await?;
    if category.commune_id != payload.commune_id {
        return Err(ApiError::bad_request(
            "La categorie n'appartient pas a cette commune",
        ));
    }

    let nom = required_text(payload.nom, "nom")?;
    let id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO intervention_types (id, commune_id, category_id, nom, description, active)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(id)
    .bind(payload.commune_id)
    .bind(payload.category_id)
    .bind(&nom)
    .bind(clean_optional(payload.description))
    .bind(payload.active.unwrap_or(true))
    .execute(&state.db)
    .await
    .map_err(map_database_error)?;

    audit::record_for_commune(
        &state.db,
        Some(payload.commune_id),
        Some(auth_user.id),
        "INTERVENTION_TYPE_CREATED",
        "intervention_types",
        Some(id),
        None,
        Some(json!({ "nom": nom, "category_id": payload.category_id })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok(Json(load_type(&state.db, id).await?))
}

async fn patch_type(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
    ApiJson(payload): ApiJson<PatchTypeRequest>,
) -> Result<Json<TypeResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    let existing = load_type(&state.db, id).await?;
    auth_user.require_commune_access(existing.commune_id)?;

    let nom = match payload.nom {
        Some(v) => required_text(v, "nom")?,
        None => existing.nom.clone(),
    };

    sqlx::query(
        r#"
        UPDATE intervention_types
        SET nom = $2, description = $3, active = $4, updated_at = now()
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .bind(&nom)
    .bind(payload.description.or(existing.description.clone()))
    .bind(payload.active.unwrap_or(existing.active))
    .execute(&state.db)
    .await
    .map_err(map_database_error)?;

    audit::record_for_commune(
        &state.db,
        Some(existing.commune_id),
        Some(auth_user.id),
        "INTERVENTION_TYPE_UPDATED",
        "intervention_types",
        Some(id),
        Some(json!({ "nom": existing.nom })),
        Some(json!({ "nom": nom })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok(Json(load_type(&state.db, id).await?))
}

async fn delete_type(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    let existing = load_type(&state.db, id).await?;
    auth_user.require_commune_access(existing.commune_id)?;

    let children: i64 = sqlx::query(
        "SELECT COUNT(*) AS total FROM interventions WHERE type_id = $1 AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?
    .get("total");

    if children > 0 {
        return Err(ApiError::conflict(
            "Impossible de supprimer un type ayant des interventions associees",
        ));
    }

    sqlx::query(
        "UPDATE intervention_types SET deleted_at = now() WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(id)
    .execute(&state.db)
    .await?;

    audit::record_for_commune(
        &state.db,
        Some(existing.commune_id),
        Some(auth_user.id),
        "INTERVENTION_TYPE_DELETED",
        "intervention_types",
        Some(id),
        Some(json!({ "nom": existing.nom })),
        None,
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok(Json(json!({ "deleted": true, "id": id })))
}

pub async fn load_type(pool: &PgPool, id: Uuid) -> Result<TypeResponse, ApiError> {
    let row = sqlx::query("SELECT * FROM intervention_types WHERE id = $1 AND deleted_at IS NULL")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found("Type d'intervention introuvable"))?;
    Ok(row_to_type(row))
}

fn row_to_type(row: sqlx::postgres::PgRow) -> TypeResponse {
    TypeResponse {
        id: row.get("id"),
        commune_id: row.get("commune_id"),
        category_id: row.get("category_id"),
        nom: row.get("nom"),
        description: row.get("description"),
        active: row.get("active"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Interventions
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct InterventionFilterQuery {
    page: Option<i64>,
    page_size: Option<i64>,
    commune_id: Option<Uuid>,
    category_id: Option<Uuid>,
    type_id: Option<Uuid>,
    active: Option<bool>,
    sujet_paiement: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CreateInterventionRequest {
    commune_id: Uuid,
    category_id: Option<Uuid>, // optionnel — dérivé automatiquement du type_id
    type_id: Uuid,
    nom: String,
    description: Option<String>,
    requires_vehicle: Option<bool>,
    sujet_paiement: bool,
    montant: Option<f64>,
    montant_fcfa: Option<i64>,
    delai_paiement_jours: Option<i32>,
    taux_penalite: Option<f64>,
    taux_penalite_basis_points: Option<i32>,
    penalite_fcfa: Option<i64>,
    /// Unite de facturation deliberee (NULL = forfait) : la quantite devient
    /// saisissable au PV et le montant est multiplie par elle.
    unite: Option<String>,
    /// TRUE = `montant_fcfa` est un tarif journalier ; la duree est saisie au PV.
    facturation_par_jour: Option<bool>,
    reference_deliberation: Option<String>,
    piece_justificative: Option<String>,
    active: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct PatchInterventionRequest {
    nom: Option<String>,
    description: Option<String>,
    requires_vehicle: Option<bool>,
    sujet_paiement: Option<bool>,
    montant: Option<f64>,
    montant_fcfa: Option<i64>,
    delai_paiement_jours: Option<i32>,
    taux_penalite: Option<f64>,
    taux_penalite_basis_points: Option<i32>,
    penalite_fcfa: Option<i64>,
    unite: Option<String>,
    facturation_par_jour: Option<bool>,
    reference_deliberation: Option<String>,
    piece_justificative: Option<String>,
    active: Option<bool>,
}

#[derive(Debug, Serialize, Clone)]
pub struct InterventionResponse {
    pub id: Uuid,
    pub commune_id: Uuid,
    pub category_id: Uuid,
    pub type_id: Uuid,
    pub nom: String,
    pub description: Option<String>,
    pub requires_vehicle: bool,
    pub sujet_paiement: bool,
    pub montant: Option<f64>,
    pub montant_fcfa: Option<i64>,
    pub delai_paiement_jours: Option<i32>,
    pub taux_penalite: Option<f64>,
    pub taux_penalite_basis_points: Option<i32>,
    pub penalite_fcfa: Option<i64>,
    pub unite: Option<String>,
    pub facturation_par_jour: bool,
    pub reference_deliberation: Option<String>,
    pub piece_justificative: Option<String>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

async fn list_interventions(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<InterventionFilterQuery>,
) -> Result<Json<Paginated<InterventionResponse>>, ApiError> {
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
    let commune_filter = resolve_commune_filter(&auth_user, query.commune_id)?;

    // category_id est maintenant dérivé via intervention_types (JOIN)
    let base_count = "SELECT COUNT(*) AS total FROM interventions i JOIN intervention_types it ON i.type_id = it.id WHERE i.deleted_at IS NULL";
    let mut count_qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(base_count);
    apply_intervention_filters(
        &mut count_qb,
        commune_filter,
        query.active,
        query.category_id,
        query.type_id,
        query.sujet_paiement,
    );
    let total: i64 = count_qb.build().fetch_one(&state.db).await?.get("total");

    let base_select = r#"
        SELECT
            i.id, i.commune_id, it.category_id, i.type_id, i.nom, i.description,
            i.requires_vehicle, i.sujet_paiement,
            i.montant::DOUBLE PRECISION AS montant,
            i.montant_fcfa, i.delai_paiement_jours,
            i.taux_penalite::DOUBLE PRECISION AS taux_penalite,
            i.taux_penalite_basis_points,
            i.penalite_fcfa,
            i.unite, i.facturation_par_jour,
            i.reference_deliberation, i.piece_justificative, i.active,
            i.created_at, i.updated_at
        FROM interventions i
        JOIN intervention_types it ON i.type_id = it.id
        WHERE i.deleted_at IS NULL
    "#;
    let mut qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(base_select);
    apply_intervention_filters(
        &mut qb,
        commune_filter,
        query.active,
        query.category_id,
        query.type_id,
        query.sujet_paiement,
    );
    qb.push(" ORDER BY i.nom ASC LIMIT ")
        .push_bind(pagination.limit)
        .push(" OFFSET ")
        .push_bind(pagination.offset);

    let rows = qb.build().fetch_all(&state.db).await?;
    let items = rows.into_iter().map(row_to_intervention).collect();
    Ok(Json(Paginated::new(items, &pagination, total)))
}

async fn get_intervention(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<InterventionResponse>, ApiError> {
    auth_user.require_any_role(&[
        Role::SuperAdmin,
        Role::AdminCommune,
        Role::ApmAgent,
        Role::Superviseur,
        Role::Receveur,
    ])?;
    let item = load_intervention(&state.db, id).await?;
    auth_user.require_commune_access(item.commune_id)?;
    Ok(Json(item))
}

async fn create_intervention(
    State(state): State<AppState>,
    auth_user: AuthUser,
    ApiJson(payload): ApiJson<CreateInterventionRequest>,
) -> Result<Json<InterventionResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    auth_user.require_commune_access(payload.commune_id)?;

    // Valider que le type appartient à la commune (la catégorie est dérivée via le type)
    let interv_type = load_type(&state.db, payload.type_id).await?;
    if interv_type.commune_id != payload.commune_id {
        return Err(ApiError::bad_request(
            "Le type n'appartient pas a cette commune",
        ));
    }
    // Si category_id est fourni, vérifier la cohérence avec le type
    if let Some(cat_id) = payload.category_id {
        if interv_type.category_id != cat_id {
            return Err(ApiError::bad_request(
                "Le type n'appartient pas a la categorie indiquee",
            ));
        }
    }

    let montant_fcfa = normalize_fcfa(payload.montant_fcfa, payload.montant)?;
    let taux_penalite_basis_points =
        normalize_basis_points(payload.taux_penalite_basis_points, payload.taux_penalite)?;
    validate_penalite_fcfa(payload.penalite_fcfa)?;
    validate_paiement_rules(
        payload.sujet_paiement,
        payload.montant,
        montant_fcfa,
        payload.reference_deliberation.as_deref(),
    )?;
    let unite = normalize_unite(payload.unite.as_deref())?;

    let nom = required_text(payload.nom, "nom")?;
    let id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO interventions (
            id, commune_id, type_id, nom, description,
            requires_vehicle, sujet_paiement, montant, montant_fcfa, delai_paiement_jours,
            taux_penalite, taux_penalite_basis_points, penalite_fcfa,
            unite, facturation_par_jour,
            reference_deliberation, piece_justificative, active
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
        "#,
    )
    .bind(id)
    .bind(payload.commune_id)
    .bind(payload.type_id)
    .bind(&nom)
    .bind(clean_optional(payload.description))
    .bind(payload.requires_vehicle.unwrap_or(false))
    .bind(payload.sujet_paiement)
    .bind(payload.montant)
    .bind(montant_fcfa)
    .bind(payload.delai_paiement_jours)
    .bind(payload.taux_penalite)
    .bind(taux_penalite_basis_points)
    .bind(payload.penalite_fcfa)
    .bind(unite)
    .bind(payload.facturation_par_jour.unwrap_or(false))
    .bind(clean_optional(payload.reference_deliberation))
    .bind(clean_optional(payload.piece_justificative))
    .bind(payload.active.unwrap_or(true))
    .execute(&state.db)
    .await
    .map_err(map_database_error)?;

    audit::record_for_commune(
        &state.db,
        Some(payload.commune_id),
        Some(auth_user.id),
        "INTERVENTION_CREATED",
        "interventions",
        Some(id),
        None,
        Some(json!({ "nom": nom, "commune_id": payload.commune_id, "sujet_paiement": payload.sujet_paiement })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok(Json(load_intervention(&state.db, id).await?))
}

async fn patch_intervention(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
    ApiJson(payload): ApiJson<PatchInterventionRequest>,
) -> Result<Json<InterventionResponse>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    let existing = load_intervention(&state.db, id).await?;
    auth_user.require_commune_access(existing.commune_id)?;

    let nom = match payload.nom {
        Some(v) => required_text(v, "nom")?,
        None => existing.nom.clone(),
    };

    // Résoudre les valeurs finales avant validation
    let sujet_paiement = payload.sujet_paiement.unwrap_or(existing.sujet_paiement);
    let montant = payload.montant.or(existing.montant);
    let montant_fcfa = payload
        .montant_fcfa
        .or_else(|| payload.montant.map(|amount| amount.round() as i64))
        .or(existing.montant_fcfa);
    let reference_deliberation = payload
        .reference_deliberation
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .or(existing.reference_deliberation.clone());

    let taux_penalite_basis_points = normalize_basis_points(
        payload.taux_penalite_basis_points,
        payload.taux_penalite,
    )?
    .or(existing.taux_penalite_basis_points);

    validate_penalite_fcfa(payload.penalite_fcfa)?;
    // penalite_fcfa = 0 désactive le forfait (retour au taux) — voir migration 23.
    let penalite_fcfa = payload.penalite_fcfa.or(existing.penalite_fcfa);

    validate_paiement_rules(
        sujet_paiement,
        montant,
        montant_fcfa,
        reference_deliberation.as_deref(),
    )?;
    // Une chaîne vide remet l'intervention au forfait ; champ absent = inchangé.
    let unite = match payload.unite.as_deref() {
        Some(value) => normalize_unite(Some(value))?,
        None => existing.unite.clone(),
    };

    sqlx::query(
        r#"
        UPDATE interventions
        SET nom = $2,
            description = $3,
            requires_vehicle = $4,
            sujet_paiement = $5,
            montant = $6,
            montant_fcfa = $7,
            delai_paiement_jours = $8,
            taux_penalite = $9,
            taux_penalite_basis_points = $10,
            penalite_fcfa = $11,
            unite = $12,
            facturation_par_jour = $13,
            reference_deliberation = $14,
            piece_justificative = $15,
            active = $16,
            updated_at = now()
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .bind(&nom)
    .bind(payload.description.or(existing.description.clone()))
    .bind(payload.requires_vehicle.unwrap_or(existing.requires_vehicle))
    .bind(sujet_paiement)
    .bind(montant)
    .bind(montant_fcfa)
    .bind(
        payload
            .delai_paiement_jours
            .or(existing.delai_paiement_jours),
    )
    .bind(payload.taux_penalite.or(existing.taux_penalite))
    .bind(taux_penalite_basis_points)
    .bind(penalite_fcfa)
    .bind(unite)
    .bind(
        payload
            .facturation_par_jour
            .unwrap_or(existing.facturation_par_jour),
    )
    .bind(reference_deliberation)
    .bind(
        payload
            .piece_justificative
            .or(existing.piece_justificative.clone()),
    )
    .bind(payload.active.unwrap_or(existing.active))
    .execute(&state.db)
    .await
    .map_err(map_database_error)?;

    audit::record_for_commune(
        &state.db,
        Some(existing.commune_id),
        Some(auth_user.id),
        "INTERVENTION_UPDATED",
        "interventions",
        Some(id),
        Some(json!({ "nom": existing.nom, "montant": existing.montant })),
        Some(json!({ "nom": nom, "montant": montant, "montant_fcfa": montant_fcfa })),
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok(Json(load_intervention(&state.db, id).await?))
}

async fn delete_intervention(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth_user.require_any_role(&[Role::SuperAdmin, Role::AdminCommune])?;
    let existing = load_intervention(&state.db, id).await?;
    auth_user.require_commune_access(existing.commune_id)?;

    sqlx::query("UPDATE interventions SET deleted_at = now() WHERE id = $1 AND deleted_at IS NULL")
        .bind(id)
        .execute(&state.db)
        .await?;

    audit::record_for_commune(
        &state.db,
        Some(existing.commune_id),
        Some(auth_user.id),
        "INTERVENTION_DELETED",
        "interventions",
        Some(id),
        Some(json!({ "nom": existing.nom })),
        None,
        auth_user.ip_address.clone(),
        auth_user.user_agent.clone(),
    )
    .await;

    Ok(Json(json!({ "deleted": true, "id": id })))
}

pub async fn load_intervention(pool: &PgPool, id: Uuid) -> Result<InterventionResponse, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT
            i.id, i.commune_id, it.category_id, i.type_id, i.nom, i.description,
            i.requires_vehicle, i.sujet_paiement,
            i.montant::DOUBLE PRECISION AS montant,
            i.montant_fcfa, i.delai_paiement_jours,
            i.taux_penalite::DOUBLE PRECISION AS taux_penalite,
            i.taux_penalite_basis_points,
            i.penalite_fcfa,
            i.unite, i.facturation_par_jour,
            i.reference_deliberation, i.piece_justificative, i.active,
            i.created_at, i.updated_at
        FROM interventions i
        JOIN intervention_types it ON i.type_id = it.id
        WHERE i.id = $1 AND i.deleted_at IS NULL
        "#
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::not_found("Intervention introuvable"))?;
    Ok(row_to_intervention(row))
}

fn row_to_intervention(row: sqlx::postgres::PgRow) -> InterventionResponse {
    InterventionResponse {
        id: row.get("id"),
        commune_id: row.get("commune_id"),
        category_id: row.get("category_id"),
        type_id: row.get("type_id"),
        nom: row.get("nom"),
        description: row.get("description"),
        requires_vehicle: row.get("requires_vehicle"),
        sujet_paiement: row.get("sujet_paiement"),
        montant: row.get("montant"),
        montant_fcfa: row.get("montant_fcfa"),
        delai_paiement_jours: row.get("delai_paiement_jours"),
        taux_penalite: row.get("taux_penalite"),
        taux_penalite_basis_points: row.get("taux_penalite_basis_points"),
        penalite_fcfa: row.get("penalite_fcfa"),
        unite: row.get("unite"),
        facturation_par_jour: row.get("facturation_par_jour"),
        reference_deliberation: row.get("reference_deliberation"),
        piece_justificative: row.get("piece_justificative"),
        active: row.get("active"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers partagés
// ─────────────────────────────────────────────────────────────────────────────

fn apply_intervention_filters(
    qb: &mut QueryBuilder<sqlx::Postgres>,
    commune_filter: Option<Uuid>,
    active: Option<bool>,
    category_id: Option<Uuid>,
    type_id: Option<Uuid>,
    sujet_paiement: Option<bool>,
) {
    if let Some(id) = commune_filter {
        qb.push(" AND i.commune_id = ").push_bind(id);
    }
    if let Some(a) = active {
        qb.push(" AND i.active = ").push_bind(a);
    }
    if let Some(id) = category_id {
        qb.push(" AND it.category_id = ").push_bind(id);
    }
    if let Some(id) = type_id {
        qb.push(" AND i.type_id = ").push_bind(id);
    }
    if let Some(sp) = sujet_paiement {
        qb.push(" AND i.sujet_paiement = ").push_bind(sp);
    }
}

fn validate_paiement_rules(
    sujet_paiement: bool,
    montant: Option<f64>,
    montant_fcfa: Option<i64>,
    reference_deliberation: Option<&str>,
) -> Result<(), ApiError> {
    if !sujet_paiement {
        return Ok(());
    }
    if montant_fcfa.map(|m| m <= 0).unwrap_or_else(|| montant.map(|m| m <= 0.0).unwrap_or(true)) {
        return Err(ApiError::bad_request(
            "Une intervention payante doit avoir un montant positif",
        ));
    }
    if reference_deliberation
        .map(|s| s.trim().is_empty())
        .unwrap_or(true)
    {
        return Err(ApiError::bad_request(
            "Une intervention payante doit avoir une reference de deliberation",
        ));
    }
    Ok(())
}

fn normalize_fcfa(value: Option<i64>, legacy: Option<f64>) -> Result<Option<i64>, ApiError> {
    let amount = value.or_else(|| legacy.map(|amount| amount.round() as i64));
    if amount.map(|amount| amount < 0).unwrap_or(false) {
        return Err(ApiError::bad_request("montant_fcfa doit etre positif"));
    }
    Ok(amount)
}

/// Unites de facturation autorisees — doit rester aligne sur la contrainte
/// `interventions_unite_check` (migration 29). Le CHECK est le garde-fou ultime ;
/// ce controle existe pour rendre une erreur 400 lisible plutot qu'un 23514 brut.
const UNITES: &[&str] = &["BETE", "UNITE", "BOUTIQUE", "MAISON", "HEURE", "JOUR", "M2"];

/// `None` = forfait. Une chaine vide vaut `None` (le formulaire web envoie "").
fn normalize_unite(value: Option<&str>) -> Result<Option<String>, ApiError> {
    let Some(raw) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let upper = raw.to_ascii_uppercase();
    if !UNITES.contains(&upper.as_str()) {
        return Err(ApiError::bad_request(format!(
            "unite invalide: {raw} (valeurs possibles: {})",
            UNITES.join(", ")
        )));
    }
    Ok(Some(upper))
}

fn validate_penalite_fcfa(value: Option<i64>) -> Result<(), ApiError> {
    if value.map(|v| v < 0).unwrap_or(false) {
        return Err(ApiError::bad_request("penalite_fcfa doit etre positif"));
    }
    Ok(())
}

fn normalize_basis_points(value: Option<i32>, legacy: Option<f64>) -> Result<Option<i32>, ApiError> {
    let rate = value.or_else(|| legacy.map(|rate| (rate * 100.0).round() as i32));
    if rate.map(|rate| rate < 0).unwrap_or(false) {
        return Err(ApiError::bad_request(
            "taux_penalite_basis_points doit etre positif",
        ));
    }
    Ok(rate)
}

fn apply_commune_active(
    qb: &mut QueryBuilder<sqlx::Postgres>,
    commune_filter: Option<Uuid>,
    active: Option<bool>,
) {
    if let Some(id) = commune_filter {
        qb.push(" AND commune_id = ").push_bind(id);
    }
    if let Some(a) = active {
        qb.push(" AND active = ").push_bind(a);
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
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

// ─────────────────────────────────────────────────────────────────────────────
// Intervention système « Mise en fourrière »
// ─────────────────────────────────────────────────────────────────────────────

/// Identifiant stable de l'intervention système « Mise en fourrière »
/// (robuste au renommage par l'admin commune).
pub(crate) const FOURRIERE_SYSTEM_CODE: &str = "FOURRIERE";
pub(crate) const FOURRIERE_CATEGORY_NAME: &str = "Fourrière";
pub(crate) const FOURRIERE_INTERVENTION_NAME: &str = "Mise en fourrière";
pub(crate) const FOURRIERE_DEFAULT_MONTANT_FCFA: i64 = 25_000;
pub(crate) const FOURRIERE_DEFAULT_DELAI_JOURS: i32 = 7;
pub(crate) const FOURRIERE_DEFAULT_TAUX_BASIS_POINTS: i32 = 1_000;

/// Cherche l'intervention système de la commune. Une intervention désactivée
/// est un conflit explicite : la commune doit la réactiver dans le référentiel
/// (le montant vient du référentiel, on ne le contourne pas).
async fn find_fourriere_intervention_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    commune_id: Uuid,
) -> Result<Option<Uuid>, ApiError> {
    let row = sqlx::query(
        "SELECT id, active FROM interventions \
         WHERE commune_id = $1 AND system_code = $2 AND deleted_at IS NULL LIMIT 1",
    )
    .bind(commune_id)
    .bind(FOURRIERE_SYSTEM_CODE)
    .fetch_optional(&mut **tx)
    .await?;
    match row {
        None => Ok(None),
        Some(row) => {
            if !row.get::<bool, _>("active") {
                return Err(ApiError::conflict(
                    "L'intervention « Mise en fourrière » est désactivée pour cette commune — réactivez-la dans le référentiel",
                ));
            }
            Ok(Some(row.get("id")))
        }
    }
}

/// Retourne l'intervention système « Mise en fourrière » de la commune, en la
/// provisionnant (catégorie + type + intervention) si absente — couvre les
/// communes créées après la migration 20260603000024 et la re-création après
/// soft-delete. Doit être appelée dans la transaction de l'appelant.
pub(crate) async fn get_or_create_fourriere_intervention_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    commune_id: Uuid,
) -> Result<Uuid, ApiError> {
    if let Some(id) = find_fourriere_intervention_tx(tx, commune_id).await? {
        return Ok(id);
    }

    // Sérialise le provisionnement par commune : deux mises en fourrière
    // simultanées sur une commune fraîche ne doivent créer qu'un référentiel.
    let key = format!("fourriere-referentiel:{commune_id}");
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(key)
        .execute(&mut **tx)
        .await?;
    if let Some(id) = find_fourriere_intervention_tx(tx, commune_id).await? {
        return Ok(id);
    }

    let category_id: Uuid = match sqlx::query_scalar(
        "SELECT id FROM intervention_categories \
         WHERE commune_id = $1 AND lower(nom) = lower($2) AND deleted_at IS NULL LIMIT 1",
    )
    .bind(commune_id)
    .bind(FOURRIERE_CATEGORY_NAME)
    .fetch_optional(&mut **tx)
    .await?
    {
        Some(id) => id,
        None => {
            let id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO intervention_categories (id, commune_id, nom, description, active) \
                 VALUES ($1, $2, $3, $4, TRUE)",
            )
            .bind(id)
            .bind(commune_id)
            .bind(FOURRIERE_CATEGORY_NAME)
            .bind("Mises en fourrière (référentiel système)")
            .execute(&mut **tx)
            .await
            .map_err(map_database_error)?;
            id
        }
    };

    let type_id: Uuid = match sqlx::query_scalar(
        "SELECT id FROM intervention_types \
         WHERE category_id = $1 AND lower(nom) = lower($2) AND deleted_at IS NULL LIMIT 1",
    )
    .bind(category_id)
    .bind(FOURRIERE_INTERVENTION_NAME)
    .fetch_optional(&mut **tx)
    .await?
    {
        Some(id) => id,
        None => {
            let id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO intervention_types (id, commune_id, category_id, nom, description, active) \
                 VALUES ($1, $2, $3, $4, $5, TRUE)",
            )
            .bind(id)
            .bind(commune_id)
            .bind(category_id)
            .bind(FOURRIERE_INTERVENTION_NAME)
            .bind("Enlèvement et mise en fourrière (véhicules et autres objets)")
            .execute(&mut **tx)
            .await
            .map_err(map_database_error)?;
            id
        }
    };

    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO interventions (
            id, commune_id, type_id, nom, description, sujet_paiement,
            montant, montant_fcfa, delai_paiement_jours,
            taux_penalite, taux_penalite_basis_points,
            reference_deliberation, requires_vehicle, active, system_code
        ) VALUES ($1, $2, $3, $4, $5, TRUE, $6, $7, $8, $9, $10, $11, FALSE, TRUE, $12)
        "#,
    )
    .bind(id)
    .bind(commune_id)
    .bind(type_id)
    .bind(FOURRIERE_INTERVENTION_NAME)
    .bind("Frais forfaitaires de mise en fourrière (montant à ajuster par la commune)")
    .bind(FOURRIERE_DEFAULT_MONTANT_FCFA as f64)
    .bind(FOURRIERE_DEFAULT_MONTANT_FCFA)
    .bind(FOURRIERE_DEFAULT_DELAI_JOURS)
    .bind((FOURRIERE_DEFAULT_TAUX_BASIS_POINTS as f64) / 100.0)
    .bind(FOURRIERE_DEFAULT_TAUX_BASIS_POINTS)
    .bind("A_COMPLETER")
    .bind(FOURRIERE_SYSTEM_CODE)
    .execute(&mut **tx)
    .await
    .map_err(map_database_error)?;

    Ok(id)
}
