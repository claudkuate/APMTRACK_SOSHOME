use serde_json::{json, Value};
use uuid::Uuid;

use crate::errors::ApiError;
use crate::modules::auth::AuthUser;
use crate::modules::rbac::Role;

pub fn resolve_commune_filter(
    auth_user: &AuthUser,
    requested: Option<Uuid>,
) -> Result<Option<Uuid>, ApiError> {
    if is_global_actor(auth_user) {
        return Ok(requested);
    }

    let user_commune = auth_user
        .commune_id
        .ok_or_else(|| ApiError::forbidden("Utilisateur non rattache a une commune"))?;
    if let Some(req) = requested {
        if req != user_commune {
            return Err(ApiError::forbidden("Acces refuse a cette commune"));
        }
    }
    Ok(Some(user_commune))
}

pub fn is_global_actor(auth_user: &AuthUser) -> bool {
    auth_user.has_role(Role::SuperAdmin)
        || (auth_user.has_role(Role::Superviseur) && auth_user.commune_id.is_none())
}

pub fn is_agent_only(auth_user: &AuthUser) -> bool {
    auth_user.has_role(Role::ApmAgent)
        && !auth_user.has_role(Role::SuperAdmin)
        && !auth_user.has_role(Role::AdminCommune)
        && !auth_user.has_role(Role::Superviseur)
        && !auth_user.has_role(Role::Receveur)
}

pub fn required_text(value: String, field: &'static str) -> Result<String, ApiError> {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request(format!("{field} est requis")));
    }
    Ok(trimmed)
}

pub fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn validate_text_len(value: &str, field: &'static str, max_len: usize) -> Result<(), ApiError> {
    if value.chars().count() > max_len {
        return Err(ApiError::bad_request(format!(
            "{field} doit contenir au plus {max_len} caracteres"
        )));
    }
    Ok(())
}

pub fn validate_optional_text_len(
    value: Option<&str>,
    field: &'static str,
    max_len: usize,
) -> Result<(), ApiError> {
    if let Some(value) = value {
        validate_text_len(value, field, max_len)?;
    }
    Ok(())
}

pub fn validate_email_like(value: Option<&str>, field: &'static str) -> Result<(), ApiError> {
    if let Some(value) = value {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Ok(());
        }
        let (local, domain) = trimmed
            .split_once('@')
            .ok_or_else(|| ApiError::bad_request(format!("{field} invalide")))?;
        if local.is_empty()
            || domain.is_empty()
            || !domain.contains('.')
            || trimmed.contains(' ')
            || trimmed.len() > 254
        {
            return Err(ApiError::bad_request(format!("{field} invalide")));
        }
    }
    Ok(())
}

pub fn validate_gps(latitude: Option<f64>, longitude: Option<f64>) -> Result<(), ApiError> {
    if let Some(latitude) = latitude {
        if !(-90.0..=90.0).contains(&latitude) {
            return Err(ApiError::bad_request(
                "gps_latitude doit etre comprise entre -90 et 90",
            ));
        }
    }
    if let Some(longitude) = longitude {
        if !(-180.0..=180.0).contains(&longitude) {
            return Err(ApiError::bad_request(
                "gps_longitude doit etre comprise entre -180 et 180",
            ));
        }
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Géospatial
// ─────────────────────────────────────────────────────────────────────────────

/// Limite serveur du nombre d'objets renvoyés par couche cartographique.
pub const GEO_MAX_FEATURES: i64 = 5000;

/// Parse une bbox `minLon,minLat,maxLon,maxLat` (ordre GeoJSON / Leaflet `toBBoxString`).
pub fn parse_bbox(value: &str) -> Result<(f64, f64, f64, f64), ApiError> {
    let parts: Vec<&str> = value.split(',').map(|s| s.trim()).collect();
    if parts.len() != 4 {
        return Err(ApiError::bad_request(
            "bbox doit etre au format minLon,minLat,maxLon,maxLat",
        ));
    }
    let mut nums = [0.0_f64; 4];
    for (i, raw) in parts.iter().enumerate() {
        nums[i] = raw
            .parse::<f64>()
            .map_err(|_| ApiError::bad_request("bbox contient une valeur non numerique"))?;
    }
    let (min_lon, min_lat, max_lon, max_lat) = (nums[0], nums[1], nums[2], nums[3]);
    validate_gps(Some(min_lat), Some(min_lon))?;
    validate_gps(Some(max_lat), Some(max_lon))?;
    if min_lon > max_lon || min_lat > max_lat {
        return Err(ApiError::bad_request("bbox: les bornes min/max sont inversees"));
    }
    Ok((min_lon, min_lat, max_lon, max_lat))
}

/// Valide qu'une valeur JSON est une géométrie GeoJSON `Polygon` ou `MultiPolygon`
/// dont les anneaux sont fermés (premier point == dernier point).
pub fn validate_geojson_polygon(value: &Value) -> Result<(), ApiError> {
    let geom_type = value
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::bad_request("boundary: GeoJSON sans champ 'type'"))?;

    let rings: Vec<&Value> = match geom_type {
        "Polygon" => value
            .get("coordinates")
            .and_then(Value::as_array)
            .map(|rings| rings.iter().collect())
            .ok_or_else(|| ApiError::bad_request("boundary: Polygon sans 'coordinates'"))?,
        "MultiPolygon" => {
            let polys = value
                .get("coordinates")
                .and_then(Value::as_array)
                .ok_or_else(|| ApiError::bad_request("boundary: MultiPolygon sans 'coordinates'"))?;
            polys
                .iter()
                .filter_map(Value::as_array)
                .flatten()
                .collect()
        }
        other => {
            return Err(ApiError::bad_request(format!(
                "boundary: type GeoJSON non supporte '{other}' (attendu Polygon ou MultiPolygon)"
            )))
        }
    };

    if rings.is_empty() {
        return Err(ApiError::bad_request("boundary: aucun anneau de coordonnees"));
    }

    for ring in rings {
        let points = ring
            .as_array()
            .ok_or_else(|| ApiError::bad_request("boundary: anneau invalide"))?;
        if points.len() < 4 {
            return Err(ApiError::bad_request(
                "boundary: un anneau doit comporter au moins 4 points (ferme)",
            ));
        }
        for point in points {
            let coords = point
                .as_array()
                .filter(|c| c.len() >= 2)
                .ok_or_else(|| ApiError::bad_request("boundary: point [lon, lat] invalide"))?;
            let lon = coords[0]
                .as_f64()
                .ok_or_else(|| ApiError::bad_request("boundary: longitude invalide"))?;
            let lat = coords[1]
                .as_f64()
                .ok_or_else(|| ApiError::bad_request("boundary: latitude invalide"))?;
            validate_gps(Some(lat), Some(lon))?;
        }
        if points.first() != points.last() {
            return Err(ApiError::bad_request(
                "boundary: chaque anneau doit etre ferme (premier point == dernier point)",
            ));
        }
    }
    Ok(())
}

/// Construit une `FeatureCollection` GeoJSON à partir de features déjà assemblées.
pub fn feature_collection(features: Vec<Value>) -> Value {
    json!({ "type": "FeatureCollection", "features": features })
}

/// Construit une `Feature` GeoJSON à partir d'une géométrie (déjà GeoJSON) et de propriétés.
pub fn geo_feature(geometry: Value, properties: Value) -> Value {
    json!({ "type": "Feature", "geometry": geometry, "properties": properties })
}

/// Formate un montant entier FCFA avec une espace fine comme séparateur de milliers
/// (« 27 500 »). Utilisé par les documents imprimés (reçu, PV) pour rester lisible sur
/// des montants à cinq ou six chiffres.
pub fn format_fcfa(amount: i64) -> String {
    let negative = amount < 0;
    let digits = amount.abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push(' ');
        }
        out.push(ch);
    }
    if negative {
        format!("-{out}")
    } else {
        out
    }
}

pub fn csv_safe_field(value: &str) -> String {
    let prefixed = match value.chars().next() {
        Some('=') | Some('+') | Some('-') | Some('@') | Some('\t') | Some('\r') | Some('\n') => {
            format!("'{value}")
        }
        _ => value.to_string(),
    };

    if prefixed.contains(',') || prefixed.contains('"') || prefixed.contains('\n') {
        format!("\"{}\"", prefixed.replace('"', "\"\""))
    } else {
        prefixed
    }
}
