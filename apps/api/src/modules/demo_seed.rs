use uuid::Uuid;

use crate::errors::ApiError;
use crate::modules::auth::{assign_roles, hash_password};
use crate::modules::rbac::Role;

const COMMUNE_ID: Uuid = Uuid::from_u128(0x10000000000000000000000000000001);
const ZONE_ID: Uuid = Uuid::from_u128(0x10000000000000000000000000000002);
const CATEGORY_ID: Uuid = Uuid::from_u128(0x10000000000000000000000000000003);
const TYPE_ID: Uuid = Uuid::from_u128(0x10000000000000000000000000000004);
const INTERVENTION_ID: Uuid = Uuid::from_u128(0x10000000000000000000000000000005);
const AGENT_USER_ID: Uuid = Uuid::from_u128(0x10000000000000000000000000000006);
const AGENT_ID: Uuid = Uuid::from_u128(0x10000000000000000000000000000007);
const ADMIN_USER_ID: Uuid = Uuid::from_u128(0x10000000000000000000000000000008);
const RECEVEUR_USER_ID: Uuid = Uuid::from_u128(0x10000000000000000000000000000009);
const SUPERVISEUR_USER_ID: Uuid = Uuid::from_u128(0x1000000000000000000000000000000a);
const SUPER_ADMIN_USER_ID: Uuid = Uuid::from_u128(0x1000000000000000000000000000000b);
const PV_PENDING_ID: Uuid = Uuid::from_u128(0x1000000000000000000000000000000c);
const PV_PAID_ID: Uuid = Uuid::from_u128(0x1000000000000000000000000000000d);
const PAYMENT_ID: Uuid = Uuid::from_u128(0x1000000000000000000000000000000e);
const SIGNALEMENT_ID: Uuid = Uuid::from_u128(0x1000000000000000000000000000000f);
const PATROUILLE_ID: Uuid = Uuid::from_u128(0x10000000000000000000000000000010);

pub async fn seed_demo(pool: &sqlx::PgPool, app_env: &str) -> anyhow::Result<()> {
    if !matches!(app_env, "development" | "test") {
        anyhow::bail!("seed-demo is only allowed in development or test environments");
    }

    let password = std::env::var("SEED_DEMO_PASSWORD")
        .unwrap_or_else(|_| "change_me_demo_123".to_string());
    if password.len() < 12 {
        anyhow::bail!("SEED_DEMO_PASSWORD must be at least 12 characters");
    }
    let password_hash = hash_password(&password).map_err(|error| anyhow::anyhow!("{error}"))?;

    seed_commune(pool).await?;
    seed_users(pool, &password_hash).await?;
    seed_agents_and_referentiel(pool).await?;
    seed_pvs_payments_signalements_patrouilles(pool).await?;

    assign_roles(pool, SUPER_ADMIN_USER_ID, &[Role::SuperAdmin]).await?;
    assign_roles(pool, ADMIN_USER_ID, &[Role::AdminCommune]).await?;
    assign_roles(pool, AGENT_USER_ID, &[Role::ApmAgent]).await?;
    assign_roles(pool, RECEVEUR_USER_ID, &[Role::Receveur]).await?;
    assign_roles(pool, SUPERVISEUR_USER_ID, &[Role::Superviseur]).await?;

    tracing::info!(
        password_hint = "SEED_DEMO_PASSWORD or change_me_demo_123",
        "demo seed completed"
    );
    Ok(())
}

async fn seed_commune(pool: &sqlx::PgPool) -> Result<(), ApiError> {
    sqlx::query(
        r#"
        INSERT INTO communes (
            id, code, nom, region, departement, adresse, telephone, email,
            theme_color, active, double_verbalisation_bloquant
        )
        VALUES (
            $1, 'DEMOYDE1', 'Commune demo Yaounde I', 'Centre', 'Mfoundi',
            'Hotel de ville de Yaounde I', '+237 222 000 000',
            'contact@yde1.apmtrack.local', '#1F7A4D', TRUE, TRUE
        )
        ON CONFLICT (id) DO UPDATE SET
            nom = EXCLUDED.nom,
            region = EXCLUDED.region,
            departement = EXCLUDED.departement,
            active = TRUE,
            updated_at = now()
        "#,
    )
    .bind(COMMUNE_ID)
    .execute(pool)
    .await?;

    Ok(())
}

async fn seed_users(pool: &sqlx::PgPool, password_hash: &str) -> Result<(), ApiError> {
    for (id, email, full_name, commune_id) in [
        (
            SUPER_ADMIN_USER_ID,
            "superadmin@apmtrack.local",
            "Super administrateur APMTRACK",
            None,
        ),
        (
            ADMIN_USER_ID,
            "admin.yde1@apmtrack.local",
            "Administrateur communal YDE1",
            Some(COMMUNE_ID),
        ),
        (
            AGENT_USER_ID,
            "agent.yde1@apmtrack.local",
            "Agent terrain YDE1",
            Some(COMMUNE_ID),
        ),
        (
            RECEVEUR_USER_ID,
            "receveur.yde1@apmtrack.local",
            "Receveur municipal YDE1",
            Some(COMMUNE_ID),
        ),
        (
            SUPERVISEUR_USER_ID,
            "superviseur.yde1@apmtrack.local",
            "Superviseur YDE1",
            Some(COMMUNE_ID),
        ),
    ] {
        sqlx::query(
            r#"
            INSERT INTO users (id, email, password_hash, full_name, commune_id, active)
            VALUES ($1, $2, $3, $4, $5, TRUE)
            ON CONFLICT (id) DO UPDATE SET
                email = EXCLUDED.email,
                password_hash = EXCLUDED.password_hash,
                full_name = EXCLUDED.full_name,
                commune_id = EXCLUDED.commune_id,
                active = TRUE,
                updated_at = now()
            "#,
        )
        .bind(id)
        .bind(email)
        .bind(password_hash)
        .bind(full_name)
        .bind(commune_id)
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn seed_agents_and_referentiel(pool: &sqlx::PgPool) -> Result<(), ApiError> {
    sqlx::query(
        r#"
        INSERT INTO agents (
            id, matricule, full_name, commune_id, grade, status,
            date_prise_fonction, formation_nasla, telephone, email, user_id
        )
        VALUES (
            $1, 'APM-YDE1-001', 'Jean Demo APM', $2, 'Agent municipal',
            'ACTIF', '2024-01-15', TRUE, '+237 699 000 001',
            'agent.yde1@apmtrack.local', $3
        )
        ON CONFLICT (id) DO UPDATE SET
            matricule = EXCLUDED.matricule,
            full_name = EXCLUDED.full_name,
            commune_id = EXCLUDED.commune_id,
            grade = EXCLUDED.grade,
            status = 'ACTIF',
            user_id = EXCLUDED.user_id,
            updated_at = now()
        "#,
    )
    .bind(AGENT_ID)
    .bind(COMMUNE_ID)
    .bind(AGENT_USER_ID)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO zones (id, commune_id, nom, type_zone, active)
        VALUES ($1, $2, 'Centre administratif', 'QUARTIER', TRUE)
        ON CONFLICT (id) DO UPDATE SET
            nom = EXCLUDED.nom,
            type_zone = EXCLUDED.type_zone,
            active = TRUE,
            updated_at = now()
        "#,
    )
    .bind(ZONE_ID)
    .bind(COMMUNE_ID)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO intervention_categories (id, commune_id, nom, description, active)
        VALUES ($1, $2, 'Verbalisation', 'Infractions donnant lieu a PV', TRUE)
        ON CONFLICT (id) DO UPDATE SET
            nom = EXCLUDED.nom,
            description = EXCLUDED.description,
            active = TRUE,
            updated_at = now()
        "#,
    )
    .bind(CATEGORY_ID)
    .bind(COMMUNE_ID)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO intervention_types (id, commune_id, category_id, nom, description, active)
        VALUES ($1, $2, $3, 'Espace public', 'Occupation et usage de l''espace public', TRUE)
        ON CONFLICT (id) DO UPDATE SET
            nom = EXCLUDED.nom,
            description = EXCLUDED.description,
            active = TRUE,
            updated_at = now()
        "#,
    )
    .bind(TYPE_ID)
    .bind(COMMUNE_ID)
    .bind(CATEGORY_ID)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO interventions (
            id, commune_id, type_id, nom, description, sujet_paiement,
            montant, montant_fcfa, delai_paiement_jours, taux_penalite,
            taux_penalite_basis_points, reference_deliberation, active
        )
        VALUES (
            $1, $2, $3, 'Occupation illicite du trottoir',
            'Occupation non autorisee du domaine public communal',
            TRUE, 25000, 25000, 7, 10, 1000, 'DEL-YDE1-2026-001', TRUE
        )
        ON CONFLICT (id) DO UPDATE SET
            nom = EXCLUDED.nom,
            description = EXCLUDED.description,
            sujet_paiement = TRUE,
            montant = 25000,
            montant_fcfa = 25000,
            delai_paiement_jours = 7,
            taux_penalite = 10,
            taux_penalite_basis_points = 1000,
            reference_deliberation = EXCLUDED.reference_deliberation,
            active = TRUE,
            updated_at = now()
        "#,
    )
    .bind(INTERVENTION_ID)
    .bind(COMMUNE_ID)
    .bind(TYPE_ID)
    .execute(pool)
    .await?;

    Ok(())
}

async fn seed_pvs_payments_signalements_patrouilles(pool: &sqlx::PgPool) -> Result<(), ApiError> {
    for (id, pv_number, status, person, plate) in [
        (
            PV_PENDING_ID,
            "PV-DEMOYDE1-2026-000001",
            "EN_ATTENTE_PAIEMENT",
            "Commercant Demo",
            "CE-123-AA",
        ),
        (
            PV_PAID_ID,
            "PV-DEMOYDE1-2026-000002",
            "PAYE",
            "Usager Demo",
            "CE-456-BB",
        ),
    ] {
        sqlx::query(
            r#"
            INSERT INTO pvs (
                id, commune_id, agent_id, pv_number, intervention_id, zone_id,
                verbalized_name, verbalized_identifier, vehicle_plate,
                location_description, amount_initial, amount_initial_fcfa,
                status, qr_code_svg, created_by
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9,
                'Boulevard du 20 Mai', 25000, 25000, $10,
                '<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200"></svg>',
                $11
            )
            ON CONFLICT (id) DO UPDATE SET
                status = EXCLUDED.status,
                verbalized_name = EXCLUDED.verbalized_name,
                vehicle_plate = EXCLUDED.vehicle_plate,
                updated_at = now()
            "#,
        )
        .bind(id)
        .bind(COMMUNE_ID)
        .bind(AGENT_ID)
        .bind(pv_number)
        .bind(INTERVENTION_ID)
        .bind(ZONE_ID)
        .bind(person)
        .bind(format!("CNI-{plate}"))
        .bind(plate)
        .bind(status)
        .bind(AGENT_USER_ID)
        .execute(pool)
        .await?;
    }

    sqlx::query(
        r#"
        INSERT INTO payments (
            id, pv_id, commune_id, amount_due, amount_penalty, amount_total,
            amount_paid, amount_due_fcfa, amount_penalty_fcfa, amount_total_fcfa,
            amount_paid_fcfa, receiver_user_id, paid_at, status, receipt_number
        )
        VALUES (
            $1, $2, $3, 25000, 0, 25000, 25000,
            25000, 0, 25000, 25000, $4, now(), 'PAYE', 'REC-DEMOYDE1-2026-000001'
        )
        ON CONFLICT (id) DO UPDATE SET
            amount_paid_fcfa = 25000,
            paid_at = now(),
            status = 'PAYE',
            updated_at = now()
        "#,
    )
    .bind(PAYMENT_ID)
    .bind(PV_PAID_ID)
    .bind(COMMUNE_ID)
    .bind(RECEVEUR_USER_ID)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO signalements (
            id, commune_id, signalement_number, type_incident,
            location_description, description, contact_anonyme, status
        )
        VALUES (
            $1, $2, 'SIG-DEMOYDE1-2026-000001', 'Occupation abusive',
            'Marche central', 'Des etalages bloquent la circulation pietonne.',
            TRUE, 'RECU'
        )
        ON CONFLICT (id) DO UPDATE SET
            status = 'RECU',
            updated_at = now()
        "#,
    )
    .bind(SIGNALEMENT_ID)
    .bind(COMMUNE_ID)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO patrouilles (id, commune_id, zone_id, nom, description, status, created_by)
        VALUES (
            $1, $2, $3, 'Patrouille centre administratif',
            'Controle de routine de la zone administrative', 'PLANIFIEE', $4
        )
        ON CONFLICT (id) DO UPDATE SET
            nom = EXCLUDED.nom,
            description = EXCLUDED.description,
            status = 'PLANIFIEE',
            updated_at = now()
        "#,
    )
    .bind(PATROUILLE_ID)
    .bind(COMMUNE_ID)
    .bind(ZONE_ID)
    .bind(ADMIN_USER_ID)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO patrouille_agents (patrouille_id, agent_id, role_patrouille)
        VALUES ($1, $2, 'CHEF')
        ON CONFLICT (patrouille_id, agent_id) DO UPDATE SET role_patrouille = 'CHEF'
        "#,
    )
    .bind(PATROUILLE_ID)
    .bind(AGENT_ID)
    .execute(pool)
    .await?;

    Ok(())
}
