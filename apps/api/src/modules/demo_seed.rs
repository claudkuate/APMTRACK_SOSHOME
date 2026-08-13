//! Données de démonstration (`seed-demo`) — découpage administratif réel du Cameroun.
//!
//! Couvre les 13 communes d'arrondissement de Yaoundé (région Centre, dépt Mfoundi)
//! et de Douala (région Littoral, dépt Wouri), chacune un tenant isolé avec ses
//! quartiers réels, son référentiel d'infractions municipales, ses agents, PV,
//! paiements, signalements et patrouilles. Les géométries PostGIS (centre + boundary
//! approximative) alimentent la carte interactive du back-office.
//!
//! Réservé aux environnements `development` / `test`. Idempotent via des UUID
//! déterministes (`uuid` v5) et le motif `ON CONFLICT ... DO UPDATE`.

use uuid::Uuid;

use crate::errors::ApiError;
use crate::modules::auth::{assign_roles, hash_password};
use crate::modules::rbac::Role;
use crate::modules::referentiel;
use crate::sequences::{next_document_sequence, SEQUENCE_FOURRIERE, SEQUENCE_PV};
use crate::storage::ObjectStorage;

/// Espace de noms fixe pour dériver des UUID stables à partir de clés textuelles.
const SEED_NS: Uuid = Uuid::from_u128(0x1000_0000_0000_0000_0000_0000_0000_0000);

/// UUID déterministe (idempotent) à partir d'une clé logique stable.
fn det_id(key: &str) -> Uuid {
    Uuid::new_v5(&SEED_NS, key.as_bytes())
}

const SUPER_ADMIN_KEY: &str = "user:superadmin";
const DEMO_AGENT_PHOTO_CONTENT_TYPE: &str = "image/jpeg";
const DEMO_AGENT_PHOTOS: [&[u8]; 2] = [
    include_bytes!("../../assets/demo-agents/agent-1.jpg"),
    include_bytes!("../../assets/demo-agents/agent-2.jpg"),
];

// ─────────────────────────────────────────────────────────────────────────────
// Catalogues
// ─────────────────────────────────────────────────────────────────────────────

struct ZoneSeed {
    nom: &'static str,
    type_zone: &'static str,
}

struct CommuneSeed {
    slug: &'static str,        // "yde1" — emails / matricules / clés UUID
    code: &'static str,        // "YDE1" — communes.code, utilisé dans pv_number
    nom: &'static str,         // "Commune d'arrondissement de Yaoundé Ier"
    region: &'static str,      // "Centre" / "Littoral"
    departement: &'static str, // "Mfoundi" / "Wouri"
    siege: &'static str,       // "Nlongkak", "Bonanjo"…
    theme_color: &'static str, // couleur du tenant
    centre: (f64, f64),        // (lon, lat) — approximatif
    zones: &'static [ZoneSeed],
}

const fn z(nom: &'static str, type_zone: &'static str) -> ZoneSeed {
    ZoneSeed { nom, type_zone }
}

/// Une infraction du référentiel municipal, répliquée pour chaque commune.
struct InterventionSeed {
    cat_slug: &'static str,
    cat_nom: &'static str,
    type_slug: &'static str,
    type_nom: &'static str,
    slug: &'static str,
    nom: &'static str,
    description: &'static str,
    montant_fcfa: i64,
    delai_jours: i32,
    penalite_pct: i32, // 10 => 10 %
}

const REFERENTIEL: &[InterventionSeed] = &[
    InterventionSeed {
        cat_slug: "voirie",
        cat_nom: "Voirie & domaine public",
        type_slug: "espace-public",
        type_nom: "Espace public",
        slug: "occupation-trottoir",
        nom: "Occupation illicite du trottoir",
        description: "Occupation non autorisée du domaine public communal",
        montant_fcfa: 25000,
        delai_jours: 7,
        penalite_pct: 10,
    },
    InterventionSeed {
        cat_slug: "voirie",
        cat_nom: "Voirie & domaine public",
        type_slug: "stationnement",
        type_nom: "Stationnement",
        slug: "stationnement-genant",
        nom: "Stationnement gênant ou interdit",
        description: "Stationnement sur un emplacement interdit ou gênant la circulation",
        montant_fcfa: 10000,
        delai_jours: 7,
        penalite_pct: 10,
    },
    InterventionSeed {
        cat_slug: "voirie",
        cat_nom: "Voirie & domaine public",
        type_slug: "construction",
        type_nom: "Construction",
        slug: "empietement-construction",
        nom: "Empiètement ou construction sans autorisation",
        description: "Construction ou empiètement sur le domaine public sans autorisation",
        montant_fcfa: 50000,
        delai_jours: 15,
        penalite_pct: 5,
    },
    InterventionSeed {
        cat_slug: "voirie",
        cat_nom: "Voirie & domaine public",
        type_slug: "occupation-domaine-public",
        type_nom: "Occupation du domaine public",
        slug: "occupation-illegale-domaine",
        nom: "Occupation illégale du domaine public",
        description: "Occupation sans titre ni autorisation du domaine public communal",
        montant_fcfa: 30000,
        delai_jours: 15,
        penalite_pct: 10,
    },
    InterventionSeed {
        cat_slug: "commerce",
        cat_nom: "Commerce & activités économiques",
        type_slug: "vente-sauvette",
        type_nom: "Vente à la sauvette",
        slug: "vente-sauvette",
        nom: "Vente à la sauvette sur la voie publique",
        description: "Commerce ambulant non autorisé sur la voie publique",
        montant_fcfa: 15000,
        delai_jours: 7,
        penalite_pct: 10,
    },
    InterventionSeed {
        cat_slug: "commerce",
        cat_nom: "Commerce & activités économiques",
        type_slug: "activite-non-declaree",
        type_nom: "Activité non déclarée",
        slug: "defaut-patente",
        nom: "Défaut de patente ou de déclaration",
        description: "Exercice d'une activité commerciale sans patente ni déclaration",
        montant_fcfa: 25000,
        delai_jours: 15,
        penalite_pct: 10,
    },
    InterventionSeed {
        cat_slug: "salubrite",
        cat_nom: "Salubrité & environnement",
        type_slug: "dechets",
        type_nom: "Déchets",
        slug: "depot-sauvage",
        nom: "Dépôt sauvage d'ordures",
        description: "Abandon ou dépôt d'ordures hors des points de collecte autorisés",
        montant_fcfa: 20000,
        delai_jours: 7,
        penalite_pct: 10,
    },
    InterventionSeed {
        cat_slug: "salubrite",
        cat_nom: "Salubrité & environnement",
        type_slug: "nuisances",
        type_nom: "Nuisances",
        slug: "tapage",
        nom: "Tapage ou nuisances sonores",
        description: "Nuisances sonores troublant la tranquillité publique",
        montant_fcfa: 10000,
        delai_jours: 7,
        penalite_pct: 10,
    },
    InterventionSeed {
        cat_slug: "salubrite",
        cat_nom: "Salubrité & environnement",
        type_slug: "divagation-animaux",
        type_nom: "Divagation d'animaux",
        slug: "divagation-animaux",
        nom: "Divagation d'animaux sur la voie publique",
        description: "Animaux errants ou en divagation sur le domaine public",
        montant_fcfa: 15000,
        delai_jours: 7,
        penalite_pct: 10,
    },
    InterventionSeed {
        cat_slug: "publicite",
        cat_nom: "Publicité & affichage",
        type_slug: "affichage",
        type_nom: "Affichage",
        slug: "affichage-non-autorise",
        nom: "Affichage ou publicité non autorisée",
        description: "Affichage publicitaire sans autorisation municipale",
        montant_fcfa: 30000,
        delai_jours: 15,
        penalite_pct: 10,
    },
];

const COMMUNES: &[CommuneSeed] = &[
    // ── Yaoundé — Région Centre, Département Mfoundi ────────────────────────────
    CommuneSeed {
        slug: "yde1",
        code: "YDE1",
        nom: "Commune d'arrondissement de Yaoundé Ier",
        region: "Centre",
        departement: "Mfoundi",
        siege: "Nlongkak",
        theme_color: "#1F7A4D",
        centre: (11.516, 3.886),
        zones: &[
            z("Bastos", "QUARTIER"),
            z("Nlongkak", "QUARTIER"),
            z("Mfandena", "QUARTIER"),
            z("Etoa-Meki", "QUARTIER"),
            z("Etoudi", "QUARTIER"),
            z("Olembé", "QUARTIER"),
            z("Boulevard du 20 Mai", "AXE_ROUTIER"),
        ],
    },
    CommuneSeed {
        slug: "yde2",
        code: "YDE2",
        nom: "Commune d'arrondissement de Yaoundé IIe",
        region: "Centre",
        departement: "Mfoundi",
        siege: "Tsinga",
        theme_color: "#1F7A4D",
        centre: (11.498, 3.876),
        zones: &[
            z("Tsinga", "QUARTIER"),
            z("Mokolo", "QUARTIER"),
            z("Cité Verte", "QUARTIER"),
            z("Briqueterie", "QUARTIER"),
            z("Madagascar", "QUARTIER"),
            z("Marché Mokolo", "MARCHE"),
        ],
    },
    CommuneSeed {
        slug: "yde3",
        code: "YDE3",
        nom: "Commune d'arrondissement de Yaoundé IIIe",
        region: "Centre",
        departement: "Mfoundi",
        siege: "Efoulan",
        theme_color: "#1F7A4D",
        centre: (11.498, 3.838),
        zones: &[
            z("Efoulan", "QUARTIER"),
            z("Obili", "QUARTIER"),
            z("Nsimeyong", "QUARTIER"),
            z("Mvog-Mbi", "QUARTIER"),
            z("Nsam", "QUARTIER"),
            z("Ngoa-Ekellé", "QUARTIER"),
        ],
    },
    CommuneSeed {
        slug: "yde4",
        code: "YDE4",
        nom: "Commune d'arrondissement de Yaoundé IVe",
        region: "Centre",
        departement: "Mfoundi",
        siege: "Kondengui",
        theme_color: "#1F7A4D",
        centre: (11.540, 3.853),
        zones: &[
            z("Kondengui", "QUARTIER"),
            z("Mvog-Ada", "QUARTIER"),
            z("Nkolndongo", "QUARTIER"),
            z("Ekounou", "QUARTIER"),
            z("Mimboman", "QUARTIER"),
            z("Awae", "QUARTIER"),
        ],
    },
    CommuneSeed {
        slug: "yde5",
        code: "YDE5",
        nom: "Commune d'arrondissement de Yaoundé Ve",
        region: "Centre",
        departement: "Mfoundi",
        siege: "Nkolmesseng",
        theme_color: "#1F7A4D",
        centre: (11.535, 3.892),
        zones: &[
            z("Nkolmesseng", "QUARTIER"),
            z("Essos", "QUARTIER"),
            z("Ngousso", "QUARTIER"),
            z("Emana", "QUARTIER"),
            z("Damas", "QUARTIER"),
        ],
    },
    CommuneSeed {
        slug: "yde6",
        code: "YDE6",
        nom: "Commune d'arrondissement de Yaoundé VIe",
        region: "Centre",
        departement: "Mfoundi",
        siege: "Biyem-Assi",
        theme_color: "#1F7A4D",
        centre: (11.475, 3.842),
        zones: &[
            z("Biyem-Assi", "QUARTIER"),
            z("Simbock", "QUARTIER"),
            z("Mendong", "QUARTIER"),
            z("Etoug-Ebe", "QUARTIER"),
            z("Mvog-Betsi", "QUARTIER"),
            z("Melen", "QUARTIER"),
        ],
    },
    CommuneSeed {
        slug: "yde7",
        code: "YDE7",
        nom: "Commune d'arrondissement de Yaoundé VIIe",
        region: "Centre",
        departement: "Mfoundi",
        siege: "Nkolbisson",
        theme_color: "#1F7A4D",
        centre: (11.440, 3.870),
        zones: &[
            z("Nkolbisson", "QUARTIER"),
            z("Oyom-Abang", "QUARTIER"),
            z("Ngoulmekong", "QUARTIER"),
            z("Nkolso", "QUARTIER"),
            z("Etetak", "QUARTIER"),
        ],
    },
    // ── Douala — Région Littoral, Département Wouri ─────────────────────────────
    CommuneSeed {
        slug: "dla1",
        code: "DLA1",
        nom: "Commune d'arrondissement de Douala Ier",
        region: "Littoral",
        departement: "Wouri",
        siege: "Bonanjo",
        theme_color: "#1565C0",
        centre: (9.690, 4.045),
        zones: &[
            z("Bonanjo", "QUARTIER"),
            z("Akwa", "QUARTIER"),
            z("Bonapriso", "QUARTIER"),
            z("Deïdo", "QUARTIER"),
            z("Bali", "QUARTIER"),
            z("Marché Central", "MARCHE"),
        ],
    },
    CommuneSeed {
        slug: "dla2",
        code: "DLA2",
        nom: "Commune d'arrondissement de Douala IIe",
        region: "Littoral",
        departement: "Wouri",
        siege: "New-Bell",
        theme_color: "#1565C0",
        centre: (9.715, 4.030),
        zones: &[
            z("New-Bell", "QUARTIER"),
            z("Nylon", "QUARTIER"),
            z("Kassalafam", "QUARTIER"),
            z("Bessengue", "QUARTIER"),
            z("Madagascar", "QUARTIER"),
        ],
    },
    CommuneSeed {
        slug: "dla3",
        code: "DLA3",
        nom: "Commune d'arrondissement de Douala IIIe",
        region: "Littoral",
        departement: "Wouri",
        siege: "Logbaba",
        theme_color: "#1565C0",
        centre: (9.760, 4.000),
        zones: &[
            z("Logbaba", "QUARTIER"),
            z("Ndogpassi", "QUARTIER"),
            z("Nyalla", "QUARTIER"),
            z("Japoma", "QUARTIER"),
            z("Logbessou", "QUARTIER"),
        ],
    },
    CommuneSeed {
        slug: "dla4",
        code: "DLA4",
        nom: "Commune d'arrondissement de Douala IVe",
        region: "Littoral",
        departement: "Wouri",
        siege: "Bonassama",
        theme_color: "#1565C0",
        centre: (9.670, 4.075),
        zones: &[
            z("Bonassama", "QUARTIER"),
            z("Bonabéri", "QUARTIER"),
            z("Ndobo", "QUARTIER"),
            z("Bonendale", "QUARTIER"),
            z("Mambanda", "QUARTIER"),
        ],
    },
    CommuneSeed {
        slug: "dla5",
        code: "DLA5",
        nom: "Commune d'arrondissement de Douala Ve",
        region: "Littoral",
        departement: "Wouri",
        siege: "Kotto",
        theme_color: "#1565C0",
        centre: (9.730, 4.090),
        zones: &[
            z("Kotto", "QUARTIER"),
            z("Bonamoussadi", "QUARTIER"),
            z("Makepe", "QUARTIER"),
            z("Logpom", "QUARTIER"),
            z("Ndogbong", "QUARTIER"),
        ],
    },
    CommuneSeed {
        slug: "dla6",
        code: "DLA6",
        nom: "Commune d'arrondissement de Douala VIe",
        region: "Littoral",
        departement: "Wouri",
        siege: "Manoka",
        theme_color: "#1565C0",
        centre: (9.620, 3.880),
        zones: &[
            z("Manoka", "QUARTIER"),
            z("Youpwé", "QUARTIER"),
            z("So-Boum", "QUARTIER"),
            z("Cap-Cameroun", "QUARTIER"),
        ],
    },
];

/// Noms fictifs mais réalistes pour les personnes verbalisées / agents.
const PERSONNES: &[(&str, &str)] = &[
    ("Jean", "Mballa"),
    ("Marie", "Ngono"),
    ("Pierre", "Fotso"),
    ("Alice", "Kamga"),
    ("Samuel", "Etoa"),
    ("Brice", "Nkodo"),
];

const TYPES_SIGNALEMENT: &[(&str, &str)] = &[
    (
        "Occupation abusive",
        "Des étalages bloquent la circulation piétonne.",
    ),
    (
        "Dépôt d'ordures",
        "Un dépôt sauvage d'ordures s'est formé au bord de la route.",
    ),
    (
        "Tapage nocturne",
        "Nuisances sonores répétées en soirée dans le quartier.",
    ),
];

// ─────────────────────────────────────────────────────────────────────────────
// Helpers géo (GeoJSON SRID 4326)
// ─────────────────────────────────────────────────────────────────────────────

fn bbox_polygon_geojson(lon: f64, lat: f64, d: f64) -> String {
    let (x0, x1, y0, y1) = (lon - d, lon + d, lat - d, lat + d);
    format!(
        r#"{{"type":"Polygon","coordinates":[[[{x0},{y0}],[{x1},{y0}],[{x1},{y1}],[{x0},{y1}],[{x0},{y0}]]]}}"#
    )
}

fn bbox_multipolygon_geojson(lon: f64, lat: f64, d: f64) -> String {
    let (x0, x1, y0, y1) = (lon - d, lon + d, lat - d, lat + d);
    format!(
        r#"{{"type":"MultiPolygon","coordinates":[[[[{x0},{y0}],[{x1},{y0}],[{x1},{y1}],[{x0},{y1}],[{x0},{y0}]]]]}}"#
    )
}

/// Centre approximatif d'une zone : décalage déterministe autour du centre de la commune.
fn zone_centre(commune_centre: (f64, f64), index: usize) -> (f64, f64) {
    let col = (index % 3) as f64 - 1.0;
    let row = (index / 3) as f64 - 1.0;
    (
        commune_centre.0 + col * 0.012,
        commune_centre.1 + row * 0.012,
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Point d'entrée
// ─────────────────────────────────────────────────────────────────────────────

pub async fn seed_demo(
    pool: &sqlx::PgPool,
    app_env: &str,
    storage: Option<&ObjectStorage>,
) -> anyhow::Result<()> {
    if !matches!(app_env, "development" | "test") {
        anyhow::bail!("seed-demo is only allowed in development or test environments");
    }

    let password =
        std::env::var("SEED_DEMO_PASSWORD").unwrap_or_else(|_| "change_me_demo_123".to_string());
    if password.len() < 12 {
        anyhow::bail!("SEED_DEMO_PASSWORD must be at least 12 characters");
    }
    let password_hash = hash_password(&password).map_err(|error| anyhow::anyhow!("{error}"))?;

    if storage.is_none() {
        tracing::warn!(
            "demo agent photos skipped because object storage is not configured"
        );
    }

    seed_super_admin_user(pool, &password_hash).await?;

    for commune in COMMUNES {
        seed_commune(pool, commune).await?;
        seed_users_and_agents(pool, commune, &password_hash, storage).await?;
        seed_zones(pool, commune).await?;
        seed_referentiel(pool, commune).await?;
        let fourriere_interv_id = seed_fourriere_referentiel(pool, commune).await?;
        seed_pvs_payments(pool, commune).await?;
        seed_signalements(pool, commune).await?;
        seed_fourrieres(pool, commune, fourriere_interv_id).await?;
        seed_patrouille(pool, commune).await?;
        seed_document_sequences(pool, commune).await?;
    }

    tracing::info!(
        communes = COMMUNES.len(),
        password_hint = "SEED_DEMO_PASSWORD or change_me_demo_123",
        "demo seed completed"
    );
    Ok(())
}

async fn seed_super_admin_user(pool: &sqlx::PgPool, password_hash: &str) -> Result<(), ApiError> {
    let id = det_id(SUPER_ADMIN_KEY);
    sqlx::query(
        r#"
        INSERT INTO users (id, email, password_hash, full_name, commune_id, active)
        VALUES ($1, 'superadmin@apmtrack.local', $2, 'Super administrateur APMTRACK', NULL, TRUE)
        ON CONFLICT (id) DO UPDATE SET
            email = EXCLUDED.email,
            password_hash = EXCLUDED.password_hash,
            full_name = EXCLUDED.full_name,
            active = TRUE,
            updated_at = now()
        "#,
    )
    .bind(id)
    .bind(password_hash)
    .execute(pool)
    .await?;
    assign_roles(pool, id, &[Role::SuperAdmin]).await?;
    Ok(())
}

async fn seed_commune(pool: &sqlx::PgPool, c: &CommuneSeed) -> Result<(), ApiError> {
    let id = det_id(&format!("commune:{}", c.code));
    let boundary = bbox_multipolygon_geojson(c.centre.0, c.centre.1, 0.015);
    let adresse = format!("Hôtel de ville, {}", c.siege);
    let email = format!("contact@{}.apmtrack.local", c.slug);

    sqlx::query(
        r#"
        INSERT INTO communes (
            id, code, nom, region, departement, adresse, telephone, email,
            theme_color, active, double_verbalisation_bloquant, centre, boundary
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, '+237 222 000 000', $7, $8, TRUE, TRUE,
            ST_SetSRID(ST_MakePoint($9, $10), 4326),
            ST_SetSRID(ST_GeomFromGeoJSON($11), 4326)
        )
        ON CONFLICT (id) DO UPDATE SET
            code = EXCLUDED.code,
            nom = EXCLUDED.nom,
            region = EXCLUDED.region,
            departement = EXCLUDED.departement,
            adresse = EXCLUDED.adresse,
            email = EXCLUDED.email,
            theme_color = EXCLUDED.theme_color,
            centre = EXCLUDED.centre,
            boundary = EXCLUDED.boundary,
            active = TRUE,
            updated_at = now()
        "#,
    )
    .bind(id)
    .bind(c.code)
    .bind(c.nom)
    .bind(c.region)
    .bind(c.departement)
    .bind(adresse)
    .bind(email)
    .bind(c.theme_color)
    .bind(c.centre.0)
    .bind(c.centre.1)
    .bind(boundary)
    .execute(pool)
    .await?;

    Ok(())
}

async fn seed_users_and_agents(
    pool: &sqlx::PgPool,
    c: &CommuneSeed,
    password_hash: &str,
    storage: Option<&ObjectStorage>,
) -> anyhow::Result<()> {
    let commune_id = det_id(&format!("commune:{}", c.code));
    let upper = c.code;

    let users: [(&str, String, String, Role); 4] = [
        (
            "admin",
            format!("admin.{}@apmtrack.local", c.slug),
            format!("Administrateur communal {upper}"),
            Role::AdminCommune,
        ),
        (
            "agent",
            format!("agent.{}@apmtrack.local", c.slug),
            format!("Agent terrain {upper}"),
            Role::ApmAgent,
        ),
        (
            "receveur",
            format!("receveur.{}@apmtrack.local", c.slug),
            format!("Receveur municipal {upper}"),
            Role::Receveur,
        ),
        (
            "superviseur",
            format!("superviseur.{}@apmtrack.local", c.slug),
            format!("Superviseur {upper}"),
            Role::Superviseur,
        ),
    ];

    for (role_key, email, full_name, role) in &users {
        let id = det_id(&format!("user:{role_key}:{}", c.code));
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
        .bind(email.as_str())
        .bind(password_hash)
        .bind(full_name.as_str())
        .bind(commune_id)
        .execute(pool)
        .await?;
        assign_roles(pool, id, &[*role]).await?;
    }

    let agent_user_id = det_id(&format!("user:agent:{}", c.code));

    // Agent 001 — lié au compte APM ; agent 002 — membre supplémentaire (sans compte).
    let agents: [(u32, Option<Uuid>); 2] = [(1, Some(agent_user_id)), (2, None)];

    for (num, user_id) in agents {
        let id = det_id(&format!("agent:{}:{num}", c.code));
        let matricule = format!("APM-{upper}-{num:03}");
        let (prenom, nom) = PERSONNES[(num as usize) % PERSONNES.len()];
        let full_name = format!("{prenom} {nom}");
        let email = format!("agent{num}.{}@apmtrack.local", c.slug);
        let photo_url = seed_agent_photo(storage, id, num).await?;
        sqlx::query(
            r#"
            INSERT INTO agents (
                id, matricule, full_name, commune_id, status,
                date_prise_fonction, photo_url, telephone, email, user_id
            )
            VALUES (
                $1, $2, $3, $4, 'ACTIF', '2024-01-15',
                $5, '+237 699 000 001', $6, $7
            )
            ON CONFLICT (id) DO UPDATE SET
                matricule = EXCLUDED.matricule,
                full_name = EXCLUDED.full_name,
                commune_id = EXCLUDED.commune_id,
                status = 'ACTIF',
                photo_url = EXCLUDED.photo_url,
                user_id = EXCLUDED.user_id,
                updated_at = now()
            "#,
        )
        .bind(id)
        .bind(matricule)
        .bind(full_name)
        .bind(commune_id)
        .bind(photo_url)
        .bind(email)
        .bind(user_id)
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn seed_agent_photo(
    storage: Option<&ObjectStorage>,
    agent_id: Uuid,
    agent_number: u32,
) -> anyhow::Result<Option<String>> {
    let Some(storage) = storage else {
        return Ok(None);
    };

    let object_key = format!("avatars/agents/{agent_id}.jpg");
    let photo = DEMO_AGENT_PHOTOS[((agent_number - 1) as usize) % DEMO_AGENT_PHOTOS.len()];
    storage
        .put(&object_key, photo, DEMO_AGENT_PHOTO_CONTENT_TYPE)
        .await
        .map_err(|error| {
            anyhow::anyhow!("failed to seed demo agent photo {object_key}: {error}")
        })?;

    Ok(Some(object_key))
}

async fn seed_zones(pool: &sqlx::PgPool, c: &CommuneSeed) -> Result<(), ApiError> {
    let commune_id = det_id(&format!("commune:{}", c.code));
    for (index, zone) in c.zones.iter().enumerate() {
        let id = det_id(&format!("zone:{}:{}", c.code, zone.nom));
        let (lon, lat) = zone_centre(c.centre, index);
        let boundary = bbox_polygon_geojson(lon, lat, 0.004);
        sqlx::query(
            r#"
            INSERT INTO zones (id, commune_id, nom, type_zone, active, centre, boundary)
            VALUES (
                $1, $2, $3, $4, TRUE,
                ST_SetSRID(ST_MakePoint($5, $6), 4326),
                ST_SetSRID(ST_GeomFromGeoJSON($7), 4326)
            )
            ON CONFLICT (id) DO UPDATE SET
                nom = EXCLUDED.nom,
                type_zone = EXCLUDED.type_zone,
                centre = EXCLUDED.centre,
                boundary = EXCLUDED.boundary,
                active = TRUE,
                updated_at = now()
            "#,
        )
        .bind(id)
        .bind(commune_id)
        .bind(zone.nom)
        .bind(zone.type_zone)
        .bind(lon)
        .bind(lat)
        .bind(boundary)
        .execute(pool)
        .await?;
    }
    Ok(())
}

async fn seed_referentiel(pool: &sqlx::PgPool, c: &CommuneSeed) -> Result<(), ApiError> {
    let commune_id = det_id(&format!("commune:{}", c.code));

    for interv in REFERENTIEL {
        let cat_id = det_id(&format!("cat:{}:{}", c.code, interv.cat_slug));
        let type_id = det_id(&format!("type:{}:{}", c.code, interv.type_slug));
        let interv_id = det_id(&format!("interv:{}:{}", c.code, interv.slug));
        let reference = format!("DEL-{}-2026-{}", c.code, interv.cat_slug);
        let basis_points = interv.penalite_pct * 100;

        sqlx::query(
            r#"
            INSERT INTO intervention_categories (id, commune_id, nom, active)
            VALUES ($1, $2, $3, TRUE)
            ON CONFLICT (id) DO UPDATE SET
                nom = EXCLUDED.nom, active = TRUE, updated_at = now()
            "#,
        )
        .bind(cat_id)
        .bind(commune_id)
        .bind(interv.cat_nom)
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO intervention_types (id, commune_id, category_id, nom, active)
            VALUES ($1, $2, $3, $4, TRUE)
            ON CONFLICT (id) DO UPDATE SET
                nom = EXCLUDED.nom, active = TRUE, updated_at = now()
            "#,
        )
        .bind(type_id)
        .bind(commune_id)
        .bind(cat_id)
        .bind(interv.type_nom)
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO interventions (
                id, commune_id, type_id, nom, description, sujet_paiement,
                montant, montant_fcfa, delai_paiement_jours, taux_penalite,
                taux_penalite_basis_points, reference_deliberation, active
            )
            VALUES ($1, $2, $3, $4, $5, TRUE, $6, $6, $7, $8, $9, $10, TRUE)
            ON CONFLICT (id) DO UPDATE SET
                nom = EXCLUDED.nom,
                description = EXCLUDED.description,
                sujet_paiement = TRUE,
                montant = EXCLUDED.montant,
                montant_fcfa = EXCLUDED.montant_fcfa,
                delai_paiement_jours = EXCLUDED.delai_paiement_jours,
                taux_penalite = EXCLUDED.taux_penalite,
                taux_penalite_basis_points = EXCLUDED.taux_penalite_basis_points,
                reference_deliberation = EXCLUDED.reference_deliberation,
                active = TRUE,
                updated_at = now()
            "#,
        )
        .bind(interv_id)
        .bind(commune_id)
        .bind(type_id)
        .bind(interv.nom)
        .bind(interv.description)
        .bind(interv.montant_fcfa)
        .bind(interv.delai_jours)
        .bind(interv.penalite_pct)
        .bind(basis_points)
        .bind(reference)
        .execute(pool)
        .await?;
    }

    Ok(())
}

const QR_PLACEHOLDER: &str =
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200"></svg>"#;

/// (seq, index dans REFERENTIEL, statut PV, ancienneté en jours)
///
/// L'ancienneté est explicite : sans PV réellement échu, la démo n'affiche aucune
/// pénalité et le correctif de comptabilité passe pour inopérant. Le PV `EN_RETARD`
/// est antidaté bien au-delà du délai de paiement du référentiel.
const PV_SPECS: &[(u32, usize, &str, i64)] = &[
    (1, 0, "EN_ATTENTE_PAIEMENT", 0),
    (2, 1, "PAYE", 2),
    (3, 5, "EN_RETARD", 45),
];

async fn seed_pvs_payments(pool: &sqlx::PgPool, c: &CommuneSeed) -> Result<(), ApiError> {
    let commune_id = det_id(&format!("commune:{}", c.code));
    let agent_id = det_id(&format!("agent:{}:1", c.code));
    let agent_user_id = det_id(&format!("user:agent:{}", c.code));
    let receveur_user_id = det_id(&format!("user:receveur:{}", c.code));

    for &(seq, interv_index, status, age_days) in PV_SPECS {
        let interv = &REFERENTIEL[interv_index];
        let interv_id = det_id(&format!("interv:{}:{}", c.code, interv.slug));
        let zone = &c.zones[interv_index % c.zones.len()];
        let zone_id = det_id(&format!("zone:{}:{}", c.code, zone.nom));
        let (lon, lat) = zone_centre(c.centre, interv_index % c.zones.len());

        let pv_id = det_id(&format!("pv:{}:{seq}", c.code));
        let pv_number = format!("PV-{}-2026-{seq:06}", c.code);
        let (prenom, nom) = PERSONNES[(seq as usize) % PERSONNES.len()];
        let person = format!("{prenom} {nom}");
        let plate = format!("{}-{:03}-AA", c.code, seq * 137 % 1000);
        let identifier = format!("CNI-{}-{:03}", c.code, seq * 71 % 1000);

        sqlx::query(
            r#"
            INSERT INTO pvs (
                id, commune_id, agent_id, pv_number, intervention_id, zone_id,
                verbalized_name, verbalized_identifier, vehicle_plate,
                location_description, gps_latitude, gps_longitude,
                amount_initial, amount_initial_fcfa, status, qr_code_svg, created_by,
                created_at
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
                $13, $13, $14, $15, $16,
                now() - ($17 * 86400) * INTERVAL '1 second'
            )
            ON CONFLICT (id) DO UPDATE SET
                status = EXCLUDED.status,
                verbalized_name = EXCLUDED.verbalized_name,
                vehicle_plate = EXCLUDED.vehicle_plate,
                location_description = EXCLUDED.location_description,
                gps_latitude = EXCLUDED.gps_latitude,
                gps_longitude = EXCLUDED.gps_longitude,
                amount_initial = EXCLUDED.amount_initial,
                amount_initial_fcfa = EXCLUDED.amount_initial_fcfa,
                created_at = EXCLUDED.created_at,
                updated_at = now()
            "#,
        )
        .bind(pv_id)
        .bind(commune_id)
        .bind(agent_id)
        .bind(pv_number)
        .bind(interv_id)
        .bind(zone_id)
        .bind(person)
        .bind(identifier)
        .bind(plate)
        .bind(zone.nom)
        .bind(lat)
        .bind(lon)
        .bind(interv.montant_fcfa)
        .bind(status)
        .bind(QR_PLACEHOLDER)
        .bind(agent_user_id)
        .bind(age_days)
        .execute(pool)
        .await?;

        // Lignes payantes snapshotées, comme le fait la création d'un PV via l'API.
        // Sans elles, la vue `pv_amounts_due` retombe sur le référentiel et surtout la
        // démo n'exerce pas le chemin nominal du calcul de pénalité.
        sqlx::query(
            r#"
            INSERT INTO pv_interventions (
                pv_id, intervention_id, order_index, nom, sujet_paiement,
                montant_fcfa, delai_paiement_jours, taux_penalite,
                taux_penalite_basis_points, penalite_fcfa
            )
            SELECT $1, i.id, 0, i.nom, i.sujet_paiement, i.montant_fcfa,
                   i.delai_paiement_jours, i.taux_penalite,
                   i.taux_penalite_basis_points, i.penalite_fcfa
            FROM interventions i
            WHERE i.id = $2
            ON CONFLICT (pv_id, intervention_id) DO UPDATE SET
                montant_fcfa = EXCLUDED.montant_fcfa,
                delai_paiement_jours = EXCLUDED.delai_paiement_jours,
                taux_penalite = EXCLUDED.taux_penalite,
                taux_penalite_basis_points = EXCLUDED.taux_penalite_basis_points,
                penalite_fcfa = EXCLUDED.penalite_fcfa,
                sujet_paiement = EXCLUDED.sujet_paiement
            "#,
        )
        .bind(pv_id)
        .bind(interv_id)
        .execute(pool)
        .await?;

        if status == "PAYE" {
            let payment_id = det_id(&format!("payment:{}:{seq}", c.code));
            let receipt = format!("REC-{}-2026-{:06}", c.code, seq);
            sqlx::query(
                r#"
                INSERT INTO payments (
                    id, pv_id, commune_id, amount_due, amount_penalty, amount_total,
                    amount_paid, amount_due_fcfa, amount_penalty_fcfa, amount_total_fcfa,
                    amount_paid_fcfa, receiver_user_id, paid_at, status, receipt_number
                )
                VALUES (
                    $1, $2, $3, $4, 0, $4, $4, $4, 0, $4, $4, $5, now(), 'PAYE', $6
                )
                ON CONFLICT (id) DO UPDATE SET
                    amount_paid_fcfa = EXCLUDED.amount_paid_fcfa,
                    paid_at = now(),
                    status = 'PAYE',
                    updated_at = now()
                "#,
            )
            .bind(payment_id)
            .bind(pv_id)
            .bind(commune_id)
            .bind(interv.montant_fcfa)
            .bind(receveur_user_id)
            .bind(receipt)
            .execute(pool)
            .await?;
        }
    }

    Ok(())
}

/// (seq, index zone, statut)
const SIGNALEMENT_SPECS: &[(u32, usize, &str)] = &[(1, 0, "RECU"), (2, 1, "EN_COURS")];

async fn seed_signalements(pool: &sqlx::PgPool, c: &CommuneSeed) -> Result<(), ApiError> {
    let commune_id = det_id(&format!("commune:{}", c.code));

    for &(seq, zone_index, status) in SIGNALEMENT_SPECS {
        let id = det_id(&format!("signalement:{}:{seq}", c.code));
        let numero = format!("SIG-{}-2026-{seq:06}", c.code);
        let zone = &c.zones[zone_index % c.zones.len()];
        let (lon, lat) = zone_centre(c.centre, zone_index % c.zones.len());
        let (type_incident, description) =
            TYPES_SIGNALEMENT[(seq as usize) % TYPES_SIGNALEMENT.len()];

        sqlx::query(
            r#"
            INSERT INTO signalements (
                id, commune_id, signalement_number, type_incident,
                location_description, description, gps_latitude, gps_longitude,
                contact_anonyme, status
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, TRUE, $9)
            ON CONFLICT (id) DO UPDATE SET
                type_incident = EXCLUDED.type_incident,
                location_description = EXCLUDED.location_description,
                description = EXCLUDED.description,
                gps_latitude = EXCLUDED.gps_latitude,
                gps_longitude = EXCLUDED.gps_longitude,
                status = EXCLUDED.status,
                updated_at = now()
            "#,
        )
        .bind(id)
        .bind(commune_id)
        .bind(numero)
        .bind(type_incident)
        .bind(zone.nom)
        .bind(description)
        .bind(lat)
        .bind(lon)
        .bind(status)
        .execute(pool)
        .await?;
    }

    Ok(())
}

/// Provisionne (ou retrouve) l'intervention système « Mise en fourrière » de la
/// commune. La migration 20260603000024 ou la création lazy à la première
/// fourrière peuvent déjà l'avoir posée — le helper du référentiel fait
/// find-or-create par `system_code` puis par noms.
async fn seed_fourriere_referentiel(
    pool: &sqlx::PgPool,
    c: &CommuneSeed,
) -> Result<Uuid, ApiError> {
    let commune_id = det_id(&format!("commune:{}", c.code));
    let mut tx = pool.begin().await?;
    let intervention_id =
        referentiel::get_or_create_fourriere_intervention_tx(&mut tx, commune_id).await?;
    tx.commit().await?;
    Ok(intervention_id)
}

/// Prochain numéro libre pour un document semé, tiré du compteur
/// `document_sequences` en sautant les numéros déjà pris. Un numéro codé en dur
/// entrerait en collision (violation UNIQUE, 23505) avec de vrais documents
/// ayant consommé la séquence avant la première insertion de la ligne semée —
/// même famille de bug que les reçus (cf. `payments::generate_receipt_number`).
async fn next_free_document_number(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    commune_id: Uuid,
    kind: &str,
    prefix: &str,
    commune_code: &str,
    exists_query: &str,
) -> Result<String, ApiError> {
    for _ in 0..100 {
        let (year, seq) = next_document_sequence(tx, commune_id, kind).await?;
        let candidate = format!("{prefix}-{commune_code}-{year}-{seq:06}");
        let exists: bool = sqlx::query_scalar(exists_query)
            .bind(&candidate)
            .fetch_one(&mut **tx)
            .await?;
        if !exists {
            return Ok(candidate);
        }
    }
    Err(ApiError::internal(
        "Impossible de generer un numero de document unique",
    ))
}

async fn seed_fourrieres(
    pool: &sqlx::PgPool,
    c: &CommuneSeed,
    intervention_id: Uuid,
) -> Result<(), ApiError> {
    let commune_id = det_id(&format!("commune:{}", c.code));
    let admin_user_id = det_id(&format!("user:admin:{}", c.code));
    let agent_id = det_id(&format!("agent:{}:1", c.code));
    let id = det_id(&format!("fourriere:{}:1", c.code));
    let plate = format!("{}-1234", c.code);
    let pv_id = det_id(&format!("pv:fourriere:{}:1", c.code));

    // Sur rejeu, les lignes existent (id déterministes) et gardent leur numéro.
    // À la première insertion, le numéro est tiré du compteur : l'ON CONFLICT
    // (id) ne protège pas d'un 23505 sur le numéro UNIQUE si de vrais documents
    // ont déjà consommé la séquence.
    let mut tx = pool.begin().await?;

    // PV lié — un PV est généré par tout type d'intervention, y compris la mise
    // en fourrière.
    let existing_pv_number: Option<String> =
        sqlx::query_scalar("SELECT pv_number FROM pvs WHERE id = $1")
            .bind(pv_id)
            .fetch_optional(&mut *tx)
            .await?;
    let pv_number = match existing_pv_number {
        Some(number) => number,
        None => {
            next_free_document_number(
                &mut tx,
                commune_id,
                SEQUENCE_PV,
                "PV",
                c.code,
                "SELECT EXISTS(SELECT 1 FROM pvs WHERE pv_number = $1)",
            )
            .await?
        }
    };

    let existing_numero: Option<String> =
        sqlx::query_scalar("SELECT fourriere_number FROM fourrieres WHERE id = $1")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    let numero = match existing_numero {
        Some(number) => number,
        None => {
            next_free_document_number(
                &mut tx,
                commune_id,
                SEQUENCE_FOURRIERE,
                "FOUR",
                c.code,
                "SELECT EXISTS(SELECT 1 FROM fourrieres WHERE fourriere_number = $1)",
            )
            .await?
        }
    };

    sqlx::query(
        r#"
        INSERT INTO pvs (
            id, commune_id, agent_id, pv_number, intervention_id, subject_type,
            vehicle_plate, location_description, amount_initial, amount_initial_fcfa,
            status, qr_code_svg, notes_internes, created_by
        )
        VALUES ($1, $2, $3, $4, $5, 'VEHICLE_ONLY', $6, $7, $8, $8,
                'EN_ATTENTE_PAIEMENT', $9, $10, $11)
        ON CONFLICT (id) DO UPDATE SET
            intervention_id = EXCLUDED.intervention_id,
            vehicle_plate = EXCLUDED.vehicle_plate,
            amount_initial = EXCLUDED.amount_initial,
            amount_initial_fcfa = EXCLUDED.amount_initial_fcfa,
            updated_at = now()
        "#,
    )
    .bind(pv_id)
    .bind(commune_id)
    .bind(agent_id)
    .bind(pv_number)
    .bind(intervention_id)
    .bind(&plate)
    .bind(c.siege)
    .bind(referentiel::FOURRIERE_DEFAULT_MONTANT_FCFA)
    .bind(QR_PLACEHOLDER)
    .bind(format!("Mise en fourrière {numero} — Stationnement gênant"))
    .bind(admin_user_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO fourrieres (
            id, commune_id, pv_id, fourriere_number, vehicle_plate, vehicle_type,
            motif, lieu_enlevement, status, daily_fee_fcfa, created_by
        )
        VALUES ($1, $2, $3, $4, $5, 'Berline', 'Stationnement gênant - enlèvement',
                $6, 'EN_FOURRIERE', 2000, $7)
        ON CONFLICT (id) DO UPDATE SET
            pv_id = EXCLUDED.pv_id,
            vehicle_plate = EXCLUDED.vehicle_plate,
            motif = EXCLUDED.motif,
            status = EXCLUDED.status,
            updated_at = now()
        "#,
    )
    .bind(id)
    .bind(commune_id)
    .bind(pv_id)
    .bind(numero)
    .bind(plate)
    .bind(c.siege)
    .bind(admin_user_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(())
}

async fn seed_patrouille(pool: &sqlx::PgPool, c: &CommuneSeed) -> Result<(), ApiError> {
    let commune_id = det_id(&format!("commune:{}", c.code));
    let admin_user_id = det_id(&format!("user:admin:{}", c.code));
    let zone = &c.zones[0];
    let zone_id = det_id(&format!("zone:{}:{}", c.code, zone.nom));
    let patrouille_id = det_id(&format!("patrouille:{}:1", c.code));

    sqlx::query(
        r#"
        INSERT INTO patrouilles (id, commune_id, zone_id, nom, description, status, created_by)
        VALUES ($1, $2, $3, $4, 'Contrôle de routine de la zone', 'PLANIFIEE', $5)
        ON CONFLICT (id) DO UPDATE SET
            nom = EXCLUDED.nom,
            zone_id = EXCLUDED.zone_id,
            status = 'PLANIFIEE',
            updated_at = now()
        "#,
    )
    .bind(patrouille_id)
    .bind(commune_id)
    .bind(zone_id)
    .bind(format!("Patrouille {}", zone.nom))
    .bind(admin_user_id)
    .execute(pool)
    .await?;

    for (num, role) in [(1u32, "CHEF"), (2u32, "MEMBRE")] {
        let agent_id = det_id(&format!("agent:{}:{num}", c.code));
        sqlx::query(
            r#"
            INSERT INTO patrouille_agents (patrouille_id, agent_id, role_patrouille)
            VALUES ($1, $2, $3)
            ON CONFLICT (patrouille_id, agent_id) DO UPDATE SET role_patrouille = EXCLUDED.role_patrouille
            "#,
        )
        .bind(patrouille_id)
        .bind(agent_id)
        .bind(role)
        .execute(pool)
        .await?;
    }

    Ok(())
}

/// Aligne les séquences documentaires sur les numéros déjà semés afin que la
/// génération serveur des prochains PV / reçus / signalements ne crée pas de collision.
async fn seed_document_sequences(pool: &sqlx::PgPool, c: &CommuneSeed) -> Result<(), ApiError> {
    let commune_id = det_id(&format!("commune:{}", c.code));
    // Les reçus semés reprennent le numéro de séquence du PV payé (REC-...-{seq}),
    // pas un compteur propre : le prochain reçu doit donc dépasser le plus grand
    // seq payé, sinon le premier encaissement réel entre en collision (23505).
    let max_paid_seq = PV_SPECS
        .iter()
        .filter(|spec| spec.2 == "PAYE")
        .map(|spec| spec.0 as i64)
        .max()
        .unwrap_or(0);

    let sequences: [(&str, i64); 4] = [
        // Plancher des PV codés en dur (PV_SPECS, seq 1..=3) — le PV de la
        // fourrière semée tire déjà son numéro du compteur (seed_fourrieres).
        ("PV", PV_SPECS.len() as i64 + 1),
        ("RECEIPT", max_paid_seq + 1),
        ("SIGNALEMENT", SIGNALEMENT_SPECS.len() as i64 + 1),
        // La fourrière semée tire aussi son numéro du compteur.
        ("FOURRIERE", 2),
    ];

    for (kind, next_value) in sequences {
        sqlx::query(
            r#"
            INSERT INTO document_sequences (commune_id, kind, year, next_value)
            VALUES ($1, $2, 2026, $3)
            ON CONFLICT (commune_id, kind, year) DO UPDATE SET
                next_value = GREATEST(document_sequences.next_value, EXCLUDED.next_value),
                updated_at = now()
            "#,
        )
        .bind(commune_id)
        .bind(kind)
        .bind(next_value)
        .execute(pool)
        .await?;
    }

    Ok(())
}
