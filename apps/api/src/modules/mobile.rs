use axum::extract::State;
use axum::{Json, Router};
use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::errors::ApiError;
use crate::modules::auth::AuthUser;
use crate::modules::rbac::Role;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/mobile/me", axum::routing::get(me))
        .route("/mobile/interventions", axum::routing::get(interventions))
        .route(
            "/mobile/patrouille-active",
            axum::routing::get(active_patrouille),
        )
        .route("/mobile/patrouilles", axum::routing::get(mobile_patrouilles))
}

#[derive(Debug, Serialize)]
pub struct MobileUserResponse {
    pub id: Uuid,
    pub email: String,
    pub full_name: String,
    pub commune_id: Option<Uuid>,
    pub roles: Vec<String>,
    pub active: bool,
}

#[derive(Debug, Serialize)]
pub struct MobileCommuneResponse {
    pub id: Uuid,
    pub code: String,
    pub nom: String,
    pub region: String,
    pub departement: String,
}

#[derive(Debug, Serialize)]
pub struct MobileAgentResponse {
    pub id: Uuid,
    pub matricule: String,
    pub full_name: String,
    pub commune_id: Uuid,
    pub status: String,
    pub date_prise_fonction: Option<NaiveDate>,
    pub photo_url: Option<String>,
    pub telephone: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MobileMeResponse {
    pub user: MobileUserResponse,
    pub commune: MobileCommuneResponse,
    pub agent: MobileAgentResponse,
}

#[derive(Debug, Serialize)]
pub struct MobileInterventionResponse {
    pub id: Uuid,
    pub commune_id: Uuid,
    pub category_id: Uuid,
    pub category_nom: String,
    pub type_id: Uuid,
    pub type_nom: String,
    pub nom: String,
    pub description: Option<String>,
    pub requires_vehicle: bool,
    pub sujet_paiement: bool,
    pub montant: Option<f64>,
    pub montant_fcfa: Option<i64>,
    pub delai_paiement_jours: Option<i32>,
    pub taux_penalite: Option<f64>,
    pub taux_penalite_basis_points: Option<i32>,
    /// Unite de facturation (NULL = forfait) : pilote la saisie d'une quantite au PV.
    pub unite: Option<String>,
    /// TRUE = tarif journalier : une duree en jours est saisie au PV.
    pub facturation_par_jour: bool,
    pub reference_deliberation: Option<String>,
    pub piece_justificative: Option<String>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct MobilePatrouilleResponse {
    pub id: Uuid,
    pub commune_id: Uuid,
    pub zone_id: Option<Uuid>,
    pub nom: String,
    pub description: Option<String>,
    pub status: String,
    pub date_debut: Option<DateTime<Utc>>,
    pub date_fin: Option<DateTime<Utc>>,
    pub date_debut_prevue: Option<DateTime<Utc>>,
    pub date_fin_prevue: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct MobilePatrouilleAgentResponse {
    pub agent_id: Uuid,
    pub matricule: String,
    pub full_name: String,
    pub role_patrouille: String,
}

#[derive(Debug, Serialize)]
pub struct MobilePatrouilleActiveResponse {
    pub patrouille: Option<MobilePatrouilleResponse>,
    pub agents: Vec<MobilePatrouilleAgentResponse>,
}

struct AgentContext {
    id: Uuid,
    commune_id: Uuid,
    status: String,
}

async fn me(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> Result<Json<MobileMeResponse>, ApiError> {
    auth_user.require_any_role(&[Role::ApmAgent])?;
    Ok(Json(load_mobile_me(&state.db, &auth_user).await?))
}

async fn interventions(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> Result<Json<Vec<MobileInterventionResponse>>, ApiError> {
    let ctx = active_agent_context(&state.db, &auth_user).await?;
    let rows = sqlx::query(
        r#"
        SELECT
            i.id, i.commune_id, it.category_id, ic.nom AS category_nom,
            i.type_id, it.nom AS type_nom, i.nom, i.description,
            i.requires_vehicle, i.sujet_paiement,
            i.montant::DOUBLE PRECISION AS montant,
            i.montant_fcfa, i.delai_paiement_jours,
            i.taux_penalite::DOUBLE PRECISION AS taux_penalite,
            i.taux_penalite_basis_points,
            i.unite, i.facturation_par_jour,
            i.reference_deliberation, i.piece_justificative, i.active,
            i.created_at, i.updated_at
        FROM interventions i
        JOIN intervention_types it ON i.type_id = it.id
        JOIN intervention_categories ic ON ic.id = it.category_id
        JOIN communes c ON c.id = i.commune_id
        WHERE i.commune_id = $1
          AND i.active = TRUE
          AND i.deleted_at IS NULL
          AND it.deleted_at IS NULL
          AND it.active = TRUE
          AND ic.deleted_at IS NULL
          AND ic.active = TRUE
          AND c.deleted_at IS NULL
          AND c.active = TRUE
          AND c.subscription_status IN ('ACTIVE', 'TRIAL')
          AND (c.subscription_expires_at IS NULL OR c.subscription_expires_at >= now())
        ORDER BY ic.nom ASC, it.nom ASC, i.nom ASC
        "#,
    )
    .bind(ctx.commune_id)
    .fetch_all(&state.db)
    .await?;

    let items = rows
        .into_iter()
        .map(|row| MobileInterventionResponse {
            id: row.get("id"),
            commune_id: row.get("commune_id"),
            category_id: row.get("category_id"),
            category_nom: row.get("category_nom"),
            type_id: row.get("type_id"),
            type_nom: row.get("type_nom"),
            nom: row.get("nom"),
            description: row.get("description"),
            requires_vehicle: row.get("requires_vehicle"),
            sujet_paiement: row.get("sujet_paiement"),
            montant: row.get("montant"),
            montant_fcfa: row.get("montant_fcfa"),
            delai_paiement_jours: row.get("delai_paiement_jours"),
            taux_penalite: row.get("taux_penalite"),
            taux_penalite_basis_points: row.get("taux_penalite_basis_points"),
            unite: row.get("unite"),
            facturation_par_jour: row.get("facturation_par_jour"),
            reference_deliberation: row.get("reference_deliberation"),
            piece_justificative: row.get("piece_justificative"),
            active: row.get("active"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
        .collect();

    Ok(Json(items))
}

async fn active_patrouille(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> Result<Json<MobilePatrouilleActiveResponse>, ApiError> {
    let ctx = active_agent_context(&state.db, &auth_user).await?;
    let row = sqlx::query(
        r#"
        SELECT p.id, p.commune_id, p.zone_id, p.nom, p.description, p.status,
               p.date_debut, p.date_fin, p.date_debut_prevue, p.date_fin_prevue,
               p.created_at, p.updated_at
        FROM patrouilles p
        JOIN patrouille_agents pa ON pa.patrouille_id = p.id
        WHERE pa.agent_id = $1
          AND p.commune_id = $2
          AND p.status = 'EN_COURS'
          AND p.deleted_at IS NULL
        ORDER BY p.date_debut DESC NULLS LAST, p.created_at DESC
        LIMIT 1
        "#,
    )
    .bind(ctx.id)
    .bind(ctx.commune_id)
    .fetch_optional(&state.db)
    .await?;

    let Some(row) = row else {
        return Ok(Json(MobilePatrouilleActiveResponse {
            patrouille: None,
            agents: Vec::new(),
        }));
    };

    let patrouille = row_to_mobile_patrouille(&row);
    let agents = load_patrouille_agents(&state.db, patrouille.id).await?;

    Ok(Json(MobilePatrouilleActiveResponse {
        patrouille: Some(patrouille),
        agents,
    }))
}

/// Patrouilles affectées à l'agent, hors clôturées (EN_COURS d'abord, puis
/// PLANIFIEE), pour l'aperçu « Mes patrouilles » du mobile.
async fn mobile_patrouilles(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> Result<Json<Vec<MobilePatrouilleResponse>>, ApiError> {
    let ctx = active_agent_context(&state.db, &auth_user).await?;
    let rows = sqlx::query(
        r#"
        SELECT p.id, p.commune_id, p.zone_id, p.nom, p.description, p.status,
               p.date_debut, p.date_fin, p.date_debut_prevue, p.date_fin_prevue,
               p.created_at, p.updated_at
        FROM patrouilles p
        JOIN patrouille_agents pa ON pa.patrouille_id = p.id
        WHERE pa.agent_id = $1
          AND p.commune_id = $2
          AND p.status IN ('EN_COURS', 'PLANIFIEE')
          AND p.deleted_at IS NULL
        ORDER BY CASE p.status WHEN 'EN_COURS' THEN 0 ELSE 1 END,
                 p.date_debut_prevue ASC NULLS LAST, p.created_at DESC
        LIMIT 50
        "#,
    )
    .bind(ctx.id)
    .bind(ctx.commune_id)
    .fetch_all(&state.db)
    .await?;

    let items = rows.iter().map(row_to_mobile_patrouille).collect();
    Ok(Json(items))
}

fn row_to_mobile_patrouille(row: &sqlx::postgres::PgRow) -> MobilePatrouilleResponse {
    MobilePatrouilleResponse {
        id: row.get("id"),
        commune_id: row.get("commune_id"),
        zone_id: row.get("zone_id"),
        nom: row.get("nom"),
        description: row.get("description"),
        status: row.get("status"),
        date_debut: row.get("date_debut"),
        date_fin: row.get("date_fin"),
        date_debut_prevue: row.get("date_debut_prevue"),
        date_fin_prevue: row.get("date_fin_prevue"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

async fn load_mobile_me(pool: &PgPool, auth_user: &AuthUser) -> Result<MobileMeResponse, ApiError> {
    let commune_id = auth_user
        .commune_id
        .ok_or_else(|| ApiError::forbidden("Agent non rattache a une commune"))?;
    let row = sqlx::query(
        r#"
        SELECT
            a.id AS agent_id, a.matricule, a.full_name AS agent_full_name,
            a.commune_id AS agent_commune_id, a.status, a.date_prise_fonction,
            a.photo_url, a.telephone, a.email AS agent_email,
            c.id AS commune_id, c.code AS commune_code, c.nom AS commune_nom,
            c.region AS commune_region, c.departement AS commune_departement
        FROM agents a
        JOIN communes c ON c.id = a.commune_id
        WHERE a.user_id = $1
          AND a.commune_id = $2
          AND a.deleted_at IS NULL
          AND c.deleted_at IS NULL
          AND c.active = TRUE
          AND c.subscription_status IN ('ACTIVE', 'TRIAL')
          AND (c.subscription_expires_at IS NULL OR c.subscription_expires_at >= now())
        ORDER BY (a.status = 'ACTIF') DESC, a.created_at DESC
        LIMIT 1
        "#,
    )
    .bind(auth_user.id)
    .bind(commune_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::forbidden("Agent introuvable pour cet utilisateur"))?;

    Ok(MobileMeResponse {
        user: MobileUserResponse {
            id: auth_user.id,
            email: auth_user.email.clone(),
            full_name: auth_user.full_name.clone(),
            commune_id: auth_user.commune_id,
            roles: auth_user
                .roles
                .iter()
                .map(|role| role.code().to_string())
                .collect(),
            active: true,
        },
        commune: MobileCommuneResponse {
            id: row.get("commune_id"),
            code: row.get("commune_code"),
            nom: row.get("commune_nom"),
            region: row.get("commune_region"),
            departement: row.get("commune_departement"),
        },
        agent: MobileAgentResponse {
            id: row.get("agent_id"),
            matricule: row.get("matricule"),
            full_name: row.get("agent_full_name"),
            commune_id: row.get("agent_commune_id"),
            status: row.get("status"),
            date_prise_fonction: row.get("date_prise_fonction"),
            photo_url: row.get("photo_url"),
            telephone: row.get("telephone"),
            email: row.get("agent_email"),
        },
    })
}

async fn active_agent_context(
    pool: &PgPool,
    auth_user: &AuthUser,
) -> Result<AgentContext, ApiError> {
    auth_user.require_any_role(&[Role::ApmAgent])?;
    let ctx = agent_context(pool, auth_user).await?;
    if ctx.status != "ACTIF" {
        return Err(ApiError::forbidden("Agent non actif"));
    }
    Ok(ctx)
}

async fn agent_context(pool: &PgPool, auth_user: &AuthUser) -> Result<AgentContext, ApiError> {
    let commune_id = auth_user
        .commune_id
        .ok_or_else(|| ApiError::forbidden("Agent non rattache a une commune"))?;
    let row = sqlx::query(
        r#"
        SELECT a.id, a.commune_id, a.status
        FROM agents a
        JOIN communes c ON c.id = a.commune_id
        WHERE a.user_id = $1
          AND a.commune_id = $2
          AND a.deleted_at IS NULL
          AND c.deleted_at IS NULL
          AND c.active = TRUE
          AND c.subscription_status IN ('ACTIVE', 'TRIAL')
          AND (c.subscription_expires_at IS NULL OR c.subscription_expires_at >= now())
        ORDER BY (a.status = 'ACTIF') DESC, a.created_at DESC
        LIMIT 1
        "#,
    )
    .bind(auth_user.id)
    .bind(commune_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::forbidden("Agent introuvable pour cet utilisateur"))?;

    Ok(AgentContext {
        id: row.get("id"),
        commune_id: row.get("commune_id"),
        status: row.get("status"),
    })
}

async fn load_patrouille_agents(
    pool: &PgPool,
    patrouille_id: Uuid,
) -> Result<Vec<MobilePatrouilleAgentResponse>, ApiError> {
    let rows = sqlx::query(
        r#"
        SELECT a.id, a.matricule, a.full_name, pa.role_patrouille
        FROM patrouille_agents pa
        JOIN agents a ON a.id = pa.agent_id
        WHERE pa.patrouille_id = $1
          AND a.deleted_at IS NULL
        ORDER BY pa.role_patrouille DESC, a.full_name ASC
        "#,
    )
    .bind(patrouille_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| MobilePatrouilleAgentResponse {
            agent_id: row.get("id"),
            matricule: row.get("matricule"),
            full_name: row.get("full_name"),
            role_patrouille: row.get("role_patrouille"),
        })
        .collect())
}
