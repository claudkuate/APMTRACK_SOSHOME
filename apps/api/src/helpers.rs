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
