# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

---

## What is APMTRACK

APMTRACK is a multi-tenant municipal enforcement management system (gestion APM — Agents de Police Municipale). It tracks field agents, issues procès-verbaux (PVs/tickets), manages payments, and handles citizen incident reports. Each **commune** (municipality) is an isolated tenant — data never crosses commune boundaries unless the user holds `SUPER_ADMIN` or global `SUPERVISEUR`.

---

## Monorepo Layout

```
apps/api/          — Rust + Axum REST API (primary focus)
apps/web-admin/    — Angular 22 back-office
apps/mobile-agent/ — Flutter mobile app for field agents
docs/              — PRD, architecture decisions, deployment plan
migrations/        — SQLx migration files (also in apps/api/migrations/)
docker-compose.dev.yml
```

---

## Commands

### API (Rust — primary development target)

```powershell
# Run the full stack (recommended)
docker compose -f docker-compose.dev.yml up

# Run API locally (set env vars first)
$env:DATABASE_URL="postgres://apmtrack:apmtrack_dev_password@localhost:5432/apmtrack"
$env:JWT_SECRET="dev_secret_minimum_32_chars_apmtrack_2026"
cargo run -p apmtrack-api

# Build
cargo build --release -p apmtrack-api

# Tests
cargo test -p apmtrack-api

# Format check (CI gate)
cargo fmt --all -- --check

# Lint
cargo clippy -p apmtrack-api -- -D warnings

# Run a single test by name
cargo test -p apmtrack-api test_name_here

# Seed super-admin after first startup
docker compose -f docker-compose.dev.yml run --rm \
  -e SEED_SUPER_ADMIN_EMAIL=admin@apmtrack.local \
  -e SEED_SUPER_ADMIN_PASSWORD=change_me_admin_123 \
  -e SEED_SUPER_ADMIN_FULL_NAME="Super Admin" \
  api seed-super-admin
```

### Angular web-admin

```bash
cd apps/web-admin
npm install
npm start          # dev server → http://localhost:4200
npm run build
ng test
```

### Flutter mobile-agent

```bash
cd apps/mobile-agent
flutter pub get
flutter analyze
flutter test
flutter run --dart-define=API_URL=http://10.0.2.2:8080 --dart-define=APP_ENV=development
```

### Dev URLs

| Service | URL |
|---------|-----|
| API | http://localhost:8080 |
| OpenAPI JSON | http://localhost:8080/docs/openapi.json |
| Web admin | http://localhost:4200 |
| Adminer (DB UI) | http://localhost:8081 |
| PostgreSQL | localhost:5432 |
| MinIO S3 API | http://localhost:9000 |
| MinIO console | http://localhost:9001 (apmtrack / apmtrack_dev_password) |

---

## Required Environment Variables

| Variable | Required | Default | Notes |
|----------|----------|---------|-------|
| `DATABASE_URL` | ✅ | — | PostgreSQL connection string |
| `JWT_SECRET` | ✅ | — | **Minimum 32 chars** — enforced at startup |
| `APP_PORT` | ❌ | 8080 | |
| `APP_TIMEZONE` | ❌ | `Africa/Douala` | Fuseau IANA bornant la « journée » de caisse (`paid_at` est en UTC) |
| `CORS_ALLOWED_ORIGINS` | ❌ | `http://localhost:4200` | Comma-separated |
| `PUBLIC_API_URL` | ❌ | `http://localhost:8080` | Used in QR code URLs |
| `RUN_MIGRATIONS_ON_STARTUP` | ❌ | false | Set `true` in Docker |
| `JWT_ACCESS_TOKEN_TTL_MINUTES` | ❌ | 15 | |
| `JWT_REFRESH_TOKEN_TTL_DAYS` | ❌ | 7 | |
| `RATE_LIMIT_ENABLED` | ❌ | true | Toggle rate limiting |
| `RATE_LIMIT_WINDOW_SECONDS` | ❌ | 60 | Rolling window size |
| `RATE_LIMIT_LOGIN_MAX` | ❌ | 10 | Max login attempts per window per IP |
| `RATE_LIMIT_PUBLIC_MAX` | ❌ | 60 | Max public endpoint requests per window per IP |
| `S3_ENDPOINT` | ❌ | — | Object storage (MinIO/S3) for PV photos. If unset, photo endpoints report storage disabled |
| `S3_REGION` | ❌ | `us-east-1` | |
| `S3_BUCKET` | ❌ | `apmtrack-pv-photos` | |
| `S3_ACCESS_KEY` | ❌ | — | Required (with secret + endpoint) to enable photo storage |
| `S3_SECRET_KEY` | ❌ | — | |
| `PUBLIC_WEB_URL` | ❌ | = `PUBLIC_API_URL` | Public front (citizen portal) base URL — used in WhatsApp tracking links |
| `WHATSAPP_API_BASE_URL` | ❌ | `https://graph.facebook.com/v21.0` | Meta Cloud API base |
| `WHATSAPP_PHONE_NUMBER_ID` | ❌ | — | Required (with token) to enable WhatsApp delivery of the signalement tracking number |
| `WHATSAPP_ACCESS_TOKEN` | ❌ | — | Meta WhatsApp Business access token |

See `apps/api/.env.example` for a full template.

---

## API Architecture

### Entry Points

- `apps/api/src/main.rs` — tokio main, config load, optional migration/seed, TCP listen
- `apps/api/src/lib.rs` — `build_app(state)` assembles the Axum router with CORS/tracing layers
- `apps/api/src/state.rs` — `AppState { config: AppConfig, db: PgPool }` — cloned into every handler

### Router Nesting

```
/health, /health/db        ← routes/health.rs
/docs/openapi.json         ← routes/docs.rs
/api/v1/
  /auth/...                ← modules::auth::router()
  /users/...               ← modules::users::router()
  /communes/...            ← modules::communes::router()
  /agents/...              ← modules::agents::router()
  /zones/...               ← modules::zones::router()
  /referentiel/...         ← modules::referentiel::router()
  /pvs/...                 ← modules::pvs::router()
    /pvs/:id/photos        ← POST upload (multipart) / GET list; :photo_id GET content / DELETE — object storage
  /payments/...            ← modules::payments::router()
  /signalements/...        ← modules::signalements::router()
  /patrouilles/...         ← modules::patrouilles::router()
  /dashboard/...           ← modules::dashboard::router()
  /geography/...           ← modules::geography::router()
    /geography/{regions,departements,arrondissements,quartiers}       ← CRUD, écriture SUPER_ADMIN
    /geography/import-csv, /geography/import-template.csv             ← répertoire national
  /fourrieres/...          ← modules::fourrieres::router()
  /audit-logs/...          ← modules::audit_logs::router()
  /exports/...             ← modules::exports::router()
  /public/
    /agents/verify/:mat    ← public — no auth
    /pvs/:pv_number        ← public — no auth
    /signalements/         ← public POST + GET :numero_suivi
```

Each module owns its `pub fn router() -> Router<AppState>`, merged in `routes/api.rs`.

### Key Patterns

**Request handling:**
```rust
async fn create_something(
    State(state): State<AppState>,
    auth_user: AuthUser,           // ← extracted from Bearer JWT
    ApiJson(payload): ApiJson<T>,  // ← validated JSON body
) -> Result<Json<Response>, ApiError>
```

**Error responses** always serialise as:
```json
{ "error": { "code": "NOT_FOUND", "message": "...", "details": null } }
```
Map DB constraint violations in `errors::map_database_error()` (codes 23505, 23503, 23514).

**Pagination** — every list endpoint uses `Paginated<T>`:
```rust
let pagination = Pagination::from_query(PaginationQuery { page, page_size })?;
// page_size capped at 100
Paginated::new(items, &pagination, total)
// → { items, page, page_size, total }
```

**Audit logging** — three variants, never `?` on any of them (logs warning and continues):
```rust
// Standard — commune_id stored as NULL in audit_logs
audit::record(&state.db, Some(auth_user.id), "ACTION_NAME", "table", Some(entity_id),
    old_value, new_value, auth_user.ip_address.clone(), auth_user.user_agent.clone()).await;

// With commune scope — stores commune_id for filtered audit queries
audit::record_for_commune(&state.db, commune_id, Some(auth_user.id), "ACTION_NAME",
    "table", Some(entity_id), old_value, new_value,
    auth_user.ip_address.clone(), auth_user.user_agent.clone()).await;

// Inside a transaction (use when the audit must share the same tx)
audit::record_for_commune_tx(&mut tx, commune_id, Some(auth_user.id), ...).await;
```
Use `record_for_commune` in all commune-scoped modules so audit logs are filterable per commune.
Use plain `record` (commune_id NULL) for **global reference data** (`geography`): attributing national
data to one arbitrary tenant would corrupt per-commune audit filtering.

**Soft deletes** — all domain tables have `deleted_at TIMESTAMPTZ`. Always filter `WHERE deleted_at IS NULL`.

---

## Role-Based Access Control

Five roles, checked via `auth_user.require_any_role(&[...])`:

| Role | Code | Scope |
|------|------|-------|
| `SUPER_ADMIN` | global | All communes, all data |
| `ADMIN_COMMUNE` | commune | Own commune only |
| `APM_AGENT` | commune | Own commune — create PVs |
| `SUPERVISEUR` | global or commune | Read-only; global if `commune_id IS NULL` |
| `RECEVEUR` | commune | Payment validation only |

**Commune isolation** — use `resolve_commune_filter()` from `src/helpers.rs` at the top of every list handler. Do not access `auth_user.commune_id` directly for filtering.

Agents are identified by their linked `user_id` on the `agents` table. A `SUPER_ADMIN` user never has a `commune_id`.

---

## Database Conventions

- UUIDs everywhere; generated with `gen_random_uuid()` in SQL or `Uuid::new_v4()` in Rust
- All timestamps are `TIMESTAMPTZ` (UTC)
- Column naming: `snake_case` in SQL, same in Rust structs
- **Monetary amounts are integer FCFA** — stored as `BIGINT` in DB, mapped to `i64` in Rust. Column names carry the `_fcfa` suffix (e.g., `amount_initial_fcfa`, `amount_paid_fcfa`). Never use `f64` for money.
- Status columns are `TEXT` with `CHECK` constraints (not enums) for migration flexibility
- Indexes always include `WHERE deleted_at IS NULL` for soft-delete tables
- Use `sqlx::QueryBuilder` for dynamic queries; never raw string concatenation

### Migration Sequence

```
20260603000001 — roles, communes, users, user_roles, agents, refresh_tokens, audit_logs
20260603000002 — zones, intervention_categories, intervention_types, interventions
20260603000003 — pvs, pv_status_history, payments, signalements
20260603000004 — patrouilles, patrouille_agents
20260603000005 — constraint fixes (cascade, FK rules, drop redundant category_id)
20260603000006 — pilot hardening (integer FCFA columns, basis points)
20260603000007 — geospatial PostGIS (geom on pvs/signalements, boundary on zones/communes, patrouille_positions)
20260603000008 — fix intervention FCFA check
20260603000009 — pv_photos
20260603000010 — pv_interventions (multi-infractions) + backfill
20260603000011 — pv subject/vehicle details
20260603000012 — user photo
20260603000013 — patrouille planning
20260603000014 — v3 refonte (subscription_status, drop agents.grade/formation_nasla)
20260603000015 — geography hierarchy (regions, departements, communes.region_id/departement_id + trigger)
20260603000016 — signalement escalade
20260603000017 — signalement plainte
20260603000018 — commune maire_email
20260603000019 — fourrieres
20260603000020 — fourriere sequence kind
20260603000021 — pv personne morale
20260603000022 — fourriere item_type
20260603000023 — pénalité forfaitaire (interventions/pv_interventions.penalite_fcfa)
20260603000024 — intervention fourrière système
20260603000025 — réalignement document_sequences
20260603000026 — vue pv_amounts_due (base / pénalité / total — source unique des montants dus)
20260603000027 — découpage administratif : arrondissements, quartiers, communes.arrondissement_id,
                 zones.quartier_id, index uniques CI partiels (suppression/recréation possible)
20260603000028 — users.must_change_password (provisionnement automatique du compte agent)
20260603000029 — tarifs unitaires/journaliers (interventions.unite + facturation_par_jour,
                 pv_interventions.quantite + duree_jours, vue pv_amounts_due multipliée)
20260603000030 — activation des mairies par paiement/essai, registre append-only des paiements,
                 accès effectif centralisé et grâce de migration de 60 jours
20260603000031 — garde-fous DB des confirmations d'abonnement (date non future,
                 confirmateur SUPER_ADMIN, référence normalisée)
```

**Next migration number: `20260603000032`.**

**PostGIS** — the Postgres image is `postgis/postgis` (see `docker-compose.dev.yml`). Migration 7 runs
`CREATE EXTENSION postgis`. Geometry columns use SRID 4326. Never decode `geometry` into Rust directly:
read via `ST_AsGeoJSON(col)` → `serde_json::Value`, write via `ST_SetSRID(ST_GeomFromGeoJSON($n), 4326)`.
Point columns (`pvs.geom`, `signalements.geom`) are `GENERATED ALWAYS` from `gps_longitude/gps_latitude`.

**Important**: `interventions.category_id` was removed (migration 5). Retrieve `category_id` via `JOIN intervention_types it ON i.type_id = it.id` — never read it directly from the `interventions` table.

---

## Business Rules (Critical)

- **Amounts come from the referentiel** — agents cannot set free amounts on a PV. `amount_initial_fcfa` (integer FCFA) is copied from `interventions.montant_fcfa` at creation time.
- **PV number is server-generated** — format `PV-{COMMUNE_CODE}-{YEAR}-{SEQ:06}`.
- **Amounts due come from the `pv_amounts_due` view** (migration 26) — **single source of truth** for
  `amount_base_fcfa` / `amount_penalty_fcfa` / `amount_total_fcfa` / `due_date` / `is_late`.
  `dashboard.rs`, `payments.rs`, `exports.rs` and the public QR lookup all read it, so the amount shown
  to the receveur, the amount required at validation and the dashboard aggregate cannot diverge.
  Never re-implement the penalty formula in Rust, and never sum `pvs.amount_initial_fcfa` as an
  "amount due" — that column is the **base only** and structurally cannot carry a penalty.
- **Tarifs unitaires et journaliers** (migration 29) — le référentiel déclare
  `interventions.unite` (NULL = forfait) et `interventions.facturation_par_jour` ;
  le PV porte le constat dans `pv_interventions.quantite` / `duree_jours`.
  Le montant de ligne est **`montant_fcfa × quantite × duree_jours`**, dans la vue
  `pv_amounts_due` **comme** dans `pvs::total_amount_fcfa()` — les deux doivent rester
  identiques, sinon `pvs.amount_initial_fcfa` (base du repli de la vue) devient faux.
  L'API refuse une quantité sur un forfait ou une durée sur un tarif non journalier :
  un agent ne fixe jamais un montant que la délibération n'autorise pas. La pénalité
  **forfaitaire** n'est pas multipliée (c'est une sanction, pas un prix unitaire).
- **Penalty calculation** — flat `penalite_fcfa` wins over the rate; rate is
  `taux_penalite_basis_points/100`, falling back to `taux_penalite`; applied once `now > due_date`.
  A PV with no paying `pv_interventions` row still accrues its penalty, rebuilt from `interventions`
  via `pvs.intervention_id`.
- **Payment must be the exact total** — `amount_paid_fcfa != amount_total_fcfa` is rejected. There is
  no partial payment (`payments.pv_id` is UNIQUE), so this keeps
  « encaissé = somme des totaux dus » an invariant.
- **"En retard" is derived from dates**, never from `pvs.status` (nothing transitions a PV to
  `EN_RETARD` automatically). Read `is_late` / `pending_late_count`.
- **Only `RECEVEUR` validates payments** — and only within their own commune.
- **Un agent est d'office un utilisateur mobile** — `POST /agents` et `POST /agents/import-csv`
  provisionnent son compte dans la **même transaction** que la fiche : rôle `APM_AGENT`, adresse
  technique `{matricule}@agents.apmtrack.cm` (`users.email` est NOT NULL et unique, un agent de
  terrain n'a pas toujours d'adresse), mot de passe temporaire tiré sur `OsRng` et
  `must_change_password = TRUE`. Désactivable par `create_account: false` / `?create_accounts=false`.
  Le mot de passe temporaire n'est **restitué qu'une fois**, dans la réponse de création
  (`account`) ou d'import (`accounts`) — il n'est stocké nulle part en clair.
  `provision_agent_account` est idempotent : elle ne touche jamais un agent déjà rattaché à un
  compte, donc une réimportation ne réinitialise aucun mot de passe en service.
- **Ne jamais dériver un mot de passe du matricule** — celui-ci est public
  (`GET /public/agents/verify/{matricule}`), un secret qui en découlerait serait devinable.
- **Connexion par matricule ou email** — `POST /auth/login` accepte les deux dans le champ
  `email` (alias `identifier`, `matricule`) ; la résolution par matricule passe par
  `agents.user_id`, donc un agent sans compte lié reste non connectable.
  `POST /auth/change-password` est la sortie du provisionnement : elle lève le drapeau et
  révoque tous les refresh tokens.
- **Only active agents create PVs** — check `agents.status = 'ACTIF'` before insert.
- **Double-verbalization** — blocked (or warned) when same `(commune_id, intervention_id, verbalized_identifier/vehicle_plate)` exists with a non-terminal status. Configured per commune via `double_verbalisation_bloquant`.
- **Status history** — every PV status change inserts into `pv_status_history` via `pvs::record_status_change()`.
- **QR codes** — generated server-side (no external service) using the `qrcode` crate. SVG stored in `pvs.qr_code_svg`, returned only via `GET /pvs/{id}/qr`, not in the main list response.

---

## Découpage administratif (données globales, hors tenant)

Hiérarchie nationale à 4 niveaux : `regions` → `departements` → `arrondissements` → `quartiers`.
Ces tables **n'ont pas de `commune_id`** : `resolve_commune_filter` / `require_commune_access` ne
s'y appliquent pas, donc **seul le `SUPER_ADMIN` écrit** (lecture ouverte aux 5 rôles).

- `communes.arrondissement_id` relie une commune d'arrondissement à son arrondissement. Renseigner
  ce seul champ suffit : le trigger `communes_link_geography()` remonte département puis région.
  Toute nouvelle colonne géographique doit être **ajoutée à la liste `UPDATE OF` du trigger**, sinon
  un PATCH ne le déclenche pas.
- **`quartiers` (national) ≠ `zones` (communal)** : une `zone` est une aire opérationnelle d'une
  commune (quartier, marché, axe routier, zone sensible) et reste le support de `pvs.zone_id`.
  `zones.quartier_id` est un pont facultatif vers le quartier officiel.
- Unicité par **index partiels insensibles à la casse** (`WHERE deleted_at IS NULL`) : indispensable
  pour que « enlever puis rajouter » une entité avec le même code reste possible.
- Chargement des données : `POST /api/v1/geography/import-csv` ou
  `apmtrack-api seed-geography <fichier.csv>` (même fonction). Le gabarit est servi par
  `GET /api/v1/geography/import-template.csv`. L'import **crée et met à jour, ne supprime jamais** ;
  une région inconnue est une erreur de ligne, jamais une création.

## Shared Utilities (`src/helpers.rs`)

Import what you need with `use crate::helpers::{...}`. All modules should use these instead of local duplicates.

```rust
// Commune access
resolve_commune_filter(auth_user, requested_commune_id) // → Result<Option<Uuid>>
is_global_actor(auth_user)   // true for SuperAdmin or global Superviseur (no commune_id)
is_agent_only(auth_user)     // true for pure APM_AGENT with no elevated roles

// Text validation
required_text(value, "field_name")                      // → Result<String> (trims, rejects empty)
clean_optional(value)                                   // → Option<String> (trims, None if empty)
validate_text_len(value, "field", max_len)              // → Result<()>
validate_optional_text_len(opt_value, "field", max_len) // → Result<()>
validate_email_like(opt_value, "field")                 // → Result<()> (format check, not DNS)

// Coordinates
validate_gps(latitude, longitude)                       // → Result<()> (ranges: lat ±90, lon ±180)

// CSV safety (injection prevention)
csv_safe_field(value)                                   // → String (prefixes =+-@, quotes commas)

// Géospatial (PostGIS — voir module `geo`)
parse_bbox("minLon,minLat,maxLon,maxLat")               // → Result<(f64,f64,f64,f64)>
validate_geojson_polygon(&value)                        // → Result<()> (Polygon/MultiPolygon fermé)
feature_collection(features)                             // → Value (GeoJSON FeatureCollection)
geo_feature(geometry, properties)                       // → Value (GeoJSON Feature)
```

---

## Adding a New Module

1. Create `src/modules/my_module.rs` with `pub fn router() -> Router<AppState>`
2. Add `pub mod my_module;` to `src/modules/mod.rs`
3. Add `.merge(modules::my_module::router())` in `src/routes/api.rs`
4. If public endpoints needed, add `pub fn public_router()` and merge it inside the `"/public"` nest

Follow the pattern in any existing module: `list_*`, `get_*`, `create_*`, `patch_*` — each with role check → commune access check → DB query → `audit::record_for_commune(...)`.

Use `is_global_actor(auth_user)` / `is_agent_only(auth_user)` from `helpers.rs` instead of re-implementing role combinations inline.
