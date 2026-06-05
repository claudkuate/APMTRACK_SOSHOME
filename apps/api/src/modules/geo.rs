use axum::extract::{Query, State};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{PgPool, QueryBuilder, Row};
use uuid::Uuid;

use crate::errors::ApiError;
use crate::helpers::{
    feature_collection, geo_feature, parse_bbox, resolve_commune_filter, validate_gps,
    GEO_MAX_FEATURES,
};
use crate::modules::auth::AuthUser;
use crate::modules::rbac::Role;
use crate::state::AppState;

// ─────────────────────────────────────────────────────────────────────────────
// Router
// ─────────────────────────────────────────────────────────────────────────────

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/geo/overview", axum::routing::get(overview))
        .route("/geo/pvs", axum::routing::get(geo_pvs))
        .route("/geo/signalements", axum::routing::get(geo_signalements))
        .route("/geo/zones", axum::routing::get(geo_zones))
        .route("/geo/communes", axum::routing::get(geo_communes))
        .route("/geo/nearby", axum::routing::get(nearby))
}

const READ_ROLES: &[Role] = &[
    Role::SuperAdmin,
    Role::AdminCommune,
    Role::ApmAgent,
    Role::Superviseur,
    Role::Receveur,
];

// ─────────────────────────────────────────────────────────────────────────────
// Query params
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct GeoQuery {
    pub commune_id: Option<Uuid>,
    /// Boîte englobante `minLon,minLat,maxLon,maxLat`.
    pub bbox: Option<String>,
    /// Couches désirées (overview seulement), ex. `pvs,signalements,zones`.
    pub layers: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NearbyQuery {
    pub lat: f64,
    pub lon: f64,
    /// Rayon en mètres (défaut 1000, max 50000).
    pub radius_m: Option<f64>,
    /// `pvs` ou `signalements` (défaut `pvs`).
    pub layer: Option<String>,
}

#[derive(Debug, Serialize)]
struct NearbyHit {
    layer: &'static str,
    distance_m: f64,
    feature: Value,
}

type Bbox = Option<(f64, f64, f64, f64)>;

// ─────────────────────────────────────────────────────────────────────────────
// Overview — toutes les couches en un appel
// ─────────────────────────────────────────────────────────────────────────────

async fn overview(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<GeoQuery>,
) -> Result<Json<Value>, ApiError> {
    auth_user.require_any_role(READ_ROLES)?;
    let commune_filter = resolve_commune_filter(&auth_user, query.commune_id)?;
    let bbox = match query.bbox {
        Some(ref raw) => Some(parse_bbox(raw)?),
        None => None,
    };

    let requested: Vec<String> = query
        .layers
        .as_deref()
        .map(|s| {
            s.split(',')
                .map(|p| p.trim().to_ascii_lowercase())
                .filter(|p| !p.is_empty())
                .collect()
        })
        .unwrap_or_else(|| {
            vec![
                "pvs".into(),
                "signalements".into(),
                "zones".into(),
                "communes".into(),
                "patrouilles".into(),
            ]
        });

    let mut result = serde_json::Map::new();
    let status = query.status.as_deref();
    for layer in requested {
        let fc = match layer.as_str() {
            "pvs" => pvs_features(&state.db, commune_filter, bbox, status).await?,
            "signalements" => {
                signalements_features(&state.db, commune_filter, bbox, status).await?
            }
            "zones" => zones_features(&state.db, commune_filter, bbox).await?,
            "communes" => communes_features(&state.db, commune_filter, bbox).await?,
            "patrouilles" => patrouilles_features(&state.db, commune_filter).await?,
            _ => continue,
        };
        result.insert(layer, fc);
    }

    Ok(Json(Value::Object(result)))
}

// ─────────────────────────────────────────────────────────────────────────────
// Endpoints par couche
// ─────────────────────────────────────────────────────────────────────────────

async fn geo_pvs(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<GeoQuery>,
) -> Result<Json<Value>, ApiError> {
    auth_user.require_any_role(READ_ROLES)?;
    let commune_filter = resolve_commune_filter(&auth_user, query.commune_id)?;
    let bbox = match query.bbox {
        Some(ref raw) => Some(parse_bbox(raw)?),
        None => None,
    };
    Ok(Json(
        pvs_features(&state.db, commune_filter, bbox, query.status.as_deref()).await?,
    ))
}

async fn geo_signalements(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<GeoQuery>,
) -> Result<Json<Value>, ApiError> {
    auth_user.require_any_role(READ_ROLES)?;
    let commune_filter = resolve_commune_filter(&auth_user, query.commune_id)?;
    let bbox = match query.bbox {
        Some(ref raw) => Some(parse_bbox(raw)?),
        None => None,
    };
    Ok(Json(
        signalements_features(&state.db, commune_filter, bbox, query.status.as_deref()).await?,
    ))
}

async fn geo_zones(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<GeoQuery>,
) -> Result<Json<Value>, ApiError> {
    auth_user.require_any_role(READ_ROLES)?;
    let commune_filter = resolve_commune_filter(&auth_user, query.commune_id)?;
    let bbox = match query.bbox {
        Some(ref raw) => Some(parse_bbox(raw)?),
        None => None,
    };
    Ok(Json(zones_features(&state.db, commune_filter, bbox).await?))
}

async fn geo_communes(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<GeoQuery>,
) -> Result<Json<Value>, ApiError> {
    auth_user.require_any_role(READ_ROLES)?;
    let commune_filter = resolve_commune_filter(&auth_user, query.commune_id)?;
    let bbox = match query.bbox {
        Some(ref raw) => Some(parse_bbox(raw)?),
        None => None,
    };
    Ok(Json(
        communes_features(&state.db, commune_filter, bbox).await?,
    ))
}

async fn nearby(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Query(query): Query<NearbyQuery>,
) -> Result<Json<Vec<NearbyHit>>, ApiError> {
    auth_user.require_any_role(READ_ROLES)?;
    validate_gps(Some(query.lat), Some(query.lon))?;
    let radius = query.radius_m.unwrap_or(1000.0).clamp(1.0, 50_000.0);
    let commune_filter = resolve_commune_filter(&auth_user, None)?;
    let layer = query.layer.as_deref().unwrap_or("pvs");

    let (table, number_col, table_layer): (&str, &str, &'static str) = match layer {
        "signalements" => ("signalements", "signalement_number", "signalements"),
        "pvs" => ("pvs", "pv_number", "pvs"),
        other => {
            return Err(ApiError::bad_request(format!(
                "layer invalide '{other}' (attendu pvs ou signalements)"
            )))
        }
    };

    let deleted_clause = if table == "pvs" {
        " AND deleted_at IS NULL"
    } else {
        ""
    };

    let mut qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new("SELECT id, ");
    qb.push(number_col)
        .push(" AS number, status, commune_id, ST_AsGeoJSON(geom) AS geojson, ST_Distance(geom::geography, ST_SetSRID(ST_MakePoint(")
        .push_bind(query.lon)
        .push(", ")
        .push_bind(query.lat)
        .push("), 4326)::geography) AS distance_m FROM ")
        .push(table)
        .push(" WHERE geom IS NOT NULL")
        .push(deleted_clause)
        .push(" AND ST_DWithin(geom::geography, ST_SetSRID(ST_MakePoint(")
        .push_bind(query.lon)
        .push(", ")
        .push_bind(query.lat)
        .push("), 4326)::geography, ")
        .push_bind(radius)
        .push(")");
    if let Some(id) = commune_filter {
        qb.push(" AND commune_id = ").push_bind(id);
    }
    qb.push(" ORDER BY distance_m ASC LIMIT ")
        .push_bind(GEO_MAX_FEATURES);

    let rows = qb.build().fetch_all(&state.db).await?;
    let hits = rows
        .into_iter()
        .map(|row| {
            let distance_m: f64 = row.get("distance_m");
            let feature = geo_feature(
                parse_geojson(row.get::<Option<String>, _>("geojson")),
                json!({
                    "id": row.get::<Uuid, _>("id"),
                    "number": row.get::<String, _>("number"),
                    "status": row.get::<String, _>("status"),
                    "commune_id": row.get::<Uuid, _>("commune_id"),
                }),
            );
            NearbyHit {
                layer: table_layer,
                distance_m,
                feature,
            }
        })
        .collect();

    Ok(Json(hits))
}

// ─────────────────────────────────────────────────────────────────────────────
// Construction des FeatureCollections (réutilisé par overview + endpoints)
// ─────────────────────────────────────────────────────────────────────────────

fn parse_geojson(raw: Option<String>) -> Value {
    raw.and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .unwrap_or(Value::Null)
}

fn apply_bbox(qb: &mut QueryBuilder<sqlx::Postgres>, column: &str, bbox: Bbox) {
    if let Some((min_lon, min_lat, max_lon, max_lat)) = bbox {
        qb.push(" AND ")
            .push(column)
            .push(" && ST_MakeEnvelope(")
            .push_bind(min_lon)
            .push(", ")
            .push_bind(min_lat)
            .push(", ")
            .push_bind(max_lon)
            .push(", ")
            .push_bind(max_lat)
            .push(", 4326)");
    }
}

async fn pvs_features(
    pool: &PgPool,
    commune_filter: Option<Uuid>,
    bbox: Bbox,
    status: Option<&str>,
) -> Result<Value, ApiError> {
    let mut qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
        "SELECT id, pv_number, status, commune_id, agent_id, zone_id, \
         amount_initial_fcfa, verbalized_name, verbalized_identity_number, \
         vehicle_plate, vehicle_registration_card_number, ST_AsGeoJSON(geom) AS geojson \
         FROM pvs WHERE deleted_at IS NULL AND geom IS NOT NULL",
    );
    if let Some(id) = commune_filter {
        qb.push(" AND commune_id = ").push_bind(id);
    }
    if let Some(s) = status {
        qb.push(" AND status = ").push_bind(s.to_string());
    }
    apply_bbox(&mut qb, "geom", bbox);
    qb.push(" ORDER BY created_at DESC LIMIT ")
        .push_bind(GEO_MAX_FEATURES);

    let rows = qb.build().fetch_all(pool).await?;
    let features = rows
        .into_iter()
        .map(|row| {
            geo_feature(
                parse_geojson(row.get("geojson")),
                json!({
                    "id": row.get::<Uuid, _>("id"),
                    "layer": "pvs",
                    "pv_number": row.get::<String, _>("pv_number"),
                    "status": row.get::<String, _>("status"),
                    "commune_id": row.get::<Uuid, _>("commune_id"),
                    "agent_id": row.get::<Uuid, _>("agent_id"),
                    "zone_id": row.get::<Option<Uuid>, _>("zone_id"),
                    "amount_initial_fcfa": row.get::<Option<i64>, _>("amount_initial_fcfa"),
                    "verbalized_name": row.get::<Option<String>, _>("verbalized_name"),
                    "verbalized_identity_number": row.get::<Option<String>, _>("verbalized_identity_number"),
                    "vehicle_plate": row.get::<Option<String>, _>("vehicle_plate"),
                    "vehicle_registration_card_number": row.get::<Option<String>, _>("vehicle_registration_card_number"),
                    "route": format!("/pvs/{}", row.get::<Uuid, _>("id")),
                }),
            )
        })
        .collect();
    Ok(feature_collection(features))
}

async fn signalements_features(
    pool: &PgPool,
    commune_filter: Option<Uuid>,
    bbox: Bbox,
    status: Option<&str>,
) -> Result<Value, ApiError> {
    let mut qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
        "SELECT id, signalement_number, type_incident, status, commune_id, \
         location_description, ST_AsGeoJSON(geom) AS geojson \
         FROM signalements WHERE geom IS NOT NULL",
    );
    if let Some(id) = commune_filter {
        qb.push(" AND commune_id = ").push_bind(id);
    }
    if let Some(s) = status {
        qb.push(" AND status = ").push_bind(s.to_string());
    }
    apply_bbox(&mut qb, "geom", bbox);
    qb.push(" ORDER BY created_at DESC LIMIT ")
        .push_bind(GEO_MAX_FEATURES);

    let rows = qb.build().fetch_all(pool).await?;
    let features = rows
        .into_iter()
        .map(|row| {
            geo_feature(
                parse_geojson(row.get("geojson")),
                json!({
                    "id": row.get::<Uuid, _>("id"),
                    "layer": "signalements",
                    "signalement_number": row.get::<String, _>("signalement_number"),
                    "type_incident": row.get::<String, _>("type_incident"),
                    "status": row.get::<String, _>("status"),
                    "commune_id": row.get::<Uuid, _>("commune_id"),
                    "location_description": row.get::<Option<String>, _>("location_description"),
                    "route": format!("/signalements/{}", row.get::<Uuid, _>("id")),
                }),
            )
        })
        .collect();
    Ok(feature_collection(features))
}

async fn zones_features(
    pool: &PgPool,
    commune_filter: Option<Uuid>,
    bbox: Bbox,
) -> Result<Value, ApiError> {
    let mut qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
        "SELECT id, nom, type_zone, commune_id, active, ST_AsGeoJSON(boundary) AS geojson \
         FROM zones WHERE deleted_at IS NULL AND boundary IS NOT NULL",
    );
    if let Some(id) = commune_filter {
        qb.push(" AND commune_id = ").push_bind(id);
    }
    apply_bbox(&mut qb, "boundary", bbox);
    qb.push(" ORDER BY nom ASC LIMIT ")
        .push_bind(GEO_MAX_FEATURES);

    let rows = qb.build().fetch_all(pool).await?;
    let features = rows
        .into_iter()
        .map(|row| {
            geo_feature(
                parse_geojson(row.get("geojson")),
                json!({
                    "id": row.get::<Uuid, _>("id"),
                    "layer": "zones",
                    "nom": row.get::<String, _>("nom"),
                    "type_zone": row.get::<String, _>("type_zone"),
                    "commune_id": row.get::<Uuid, _>("commune_id"),
                    "active": row.get::<bool, _>("active"),
                    "route": format!("/zones/{}", row.get::<Uuid, _>("id")),
                }),
            )
        })
        .collect();
    Ok(feature_collection(features))
}

async fn communes_features(
    pool: &PgPool,
    commune_filter: Option<Uuid>,
    bbox: Bbox,
) -> Result<Value, ApiError> {
    let mut qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
        "SELECT id, code, nom, region, ST_AsGeoJSON(boundary) AS geojson \
         FROM communes WHERE deleted_at IS NULL AND boundary IS NOT NULL",
    );
    if let Some(id) = commune_filter {
        qb.push(" AND id = ").push_bind(id);
    }
    apply_bbox(&mut qb, "boundary", bbox);
    qb.push(" ORDER BY nom ASC LIMIT ")
        .push_bind(GEO_MAX_FEATURES);

    let rows = qb.build().fetch_all(pool).await?;
    let features = rows
        .into_iter()
        .map(|row| {
            geo_feature(
                parse_geojson(row.get("geojson")),
                json!({
                    "id": row.get::<Uuid, _>("id"),
                    "layer": "communes",
                    "code": row.get::<String, _>("code"),
                    "nom": row.get::<String, _>("nom"),
                    "region": row.get::<String, _>("region"),
                    "route": format!("/communes/{}", row.get::<Uuid, _>("id")),
                }),
            )
        })
        .collect();
    Ok(feature_collection(features))
}

/// Traces des patrouilles ayant au moins 2 positions (LineString reconstruite).
async fn patrouilles_features(
    pool: &PgPool,
    commune_filter: Option<Uuid>,
) -> Result<Value, ApiError> {
    let mut qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
        "SELECT p.id, p.nom, p.status, p.commune_id, \
         ST_AsGeoJSON(ST_MakeLine(pp.geom ORDER BY pp.recorded_at)) AS geojson \
         FROM patrouilles p \
         JOIN patrouille_positions pp ON pp.patrouille_id = p.id \
         WHERE p.deleted_at IS NULL",
    );
    if let Some(id) = commune_filter {
        qb.push(" AND p.commune_id = ").push_bind(id);
    }
    qb.push(" GROUP BY p.id, p.nom, p.status, p.commune_id HAVING COUNT(pp.id) >= 2 LIMIT ")
        .push_bind(GEO_MAX_FEATURES);

    let rows = qb.build().fetch_all(pool).await?;
    let features = rows
        .into_iter()
        .map(|row| {
            geo_feature(
                parse_geojson(row.get("geojson")),
                json!({
                    "id": row.get::<Uuid, _>("id"),
                    "layer": "patrouilles",
                    "nom": row.get::<String, _>("nom"),
                    "status": row.get::<String, _>("status"),
                    "commune_id": row.get::<Uuid, _>("commune_id"),
                    "route": format!("/patrouilles/{}", row.get::<Uuid, _>("id")),
                }),
            )
        })
        .collect();
    Ok(feature_collection(features))
}
