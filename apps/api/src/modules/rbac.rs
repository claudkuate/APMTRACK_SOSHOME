use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::errors::ApiError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Role {
    SuperAdmin,
    AdminCommune,
    ApmAgent,
    Superviseur,
    Receveur,
}

impl Role {
    pub fn code(self) -> &'static str {
        match self {
            Self::SuperAdmin => "SUPER_ADMIN",
            Self::AdminCommune => "ADMIN_COMMUNE",
            Self::ApmAgent => "APM_AGENT",
            Self::Superviseur => "SUPERVISEUR",
            Self::Receveur => "RECEVEUR",
        }
    }

    pub fn from_code(value: &str) -> Option<Self> {
        match value {
            "SUPER_ADMIN" => Some(Self::SuperAdmin),
            "ADMIN_COMMUNE" => Some(Self::AdminCommune),
            "APM_AGENT" => Some(Self::ApmAgent),
            "SUPERVISEUR" => Some(Self::Superviseur),
            "RECEVEUR" => Some(Self::Receveur),
            _ => None,
        }
    }

    pub fn all_seeded() -> [Self; 5] {
        [
            Self::SuperAdmin,
            Self::AdminCommune,
            Self::ApmAgent,
            Self::Superviseur,
            Self::Receveur,
        ]
    }
}

pub fn parse_roles(values: &[String]) -> Result<Vec<Role>, ApiError> {
    if values.is_empty() {
        return Err(ApiError::bad_request("Au moins un role est requis"));
    }

    values
        .iter()
        .map(|value| {
            Role::from_code(value.trim())
                .ok_or_else(|| ApiError::bad_request(format!("Role inconnu: {value}")))
        })
        .collect()
}

pub fn has_role(roles: &[Role], role: Role) -> bool {
    roles.iter().any(|candidate| *candidate == role)
}

pub fn has_any_role(roles: &[Role], allowed: &[Role]) -> bool {
    allowed.iter().any(|role| has_role(roles, *role))
}

pub fn can_access_commune(roles: &[Role], actor_commune_id: Option<Uuid>, target: Uuid) -> bool {
    has_role(roles, Role::SuperAdmin)
        || (has_role(roles, Role::Superviseur) && actor_commune_id.is_none())
        || actor_commune_id == Some(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_roles() {
        let roles = parse_roles(&["SUPER_ADMIN".to_string()]).expect("valid role");

        assert_eq!(roles, vec![Role::SuperAdmin]);
    }

    #[test]
    fn rejects_unknown_roles() {
        assert!(parse_roles(&["CITOYEN_PUBLIC".to_string()]).is_err());
    }

    #[test]
    fn global_supervisor_can_access_any_commune() {
        let target = Uuid::new_v4();

        assert!(can_access_commune(&[Role::Superviseur], None, target));
    }
}
