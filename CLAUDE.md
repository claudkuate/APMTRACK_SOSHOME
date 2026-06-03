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

---

## Required Environment Variables

| Variable | Required | Default | Notes |
|----------|----------|---------|-------|
| `DATABASE_URL` | ✅ | — | PostgreSQL connection string |
| `JWT_SECRET` | ✅ | — | **Minimum 32 chars** — enforced at startup |
| `APP_PORT` | ❌ | 8080 | |
| `CORS_ALLOWED_ORIGINS` | ❌ | `http://localhost:4200` | Comma-separated |
| `PUBLIC_API_URL` | ❌ | `http://localhost:8080` | Used in QR code URLs |
| `RUN_MIGRATIONS_ON_STARTUP` | ❌ | false | Set `true` in Docker |
| `JWT_ACCESS_TOKEN_TTL_MINUTES` | ❌ | 15 | |
| `JWT_REFRESH_TOKEN_TTL_DAYS` | ❌ | 7 | |

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
  /payments/...            ← modules::payments::router()
  /signalements/...        ← modules::signalements::router()
  /patrouilles/...         ← modules::patrouilles::router()
  /dashboard/...           ← modules::dashboard::router()
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

**Audit logging** — call after every mutating action:
```rust
audit::record(&state.db, Some(auth_user.id), "ACTION_NAME", "table", Some(entity_id),
    old_value, new_value, auth_user.ip_address.clone(), auth_user.user_agent.clone()).await;
```
Never `?` on audit — it logs a warning and continues.

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
- All monetary amounts: `NUMERIC(12,2)` in DB, mapped to `f64` in Rust responses
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
```

**Important**: `interventions.category_id` was removed (migration 5). Retrieve `category_id` via `JOIN intervention_types it ON i.type_id = it.id` — never read it directly from the `interventions` table.

---

## Business Rules (Critical)

- **Amounts come from the referentiel** — agents cannot set free amounts on a PV. `amount_initial` is copied from `interventions.montant` at creation time.
- **PV number is server-generated** — format `PV-{COMMUNE_CODE}-{YEAR}-{SEQ:06}`.
- **Penalty calculation is server-side** — `penalty = amount × rate%` if `now > created_at + delai_paiement_jours`.
- **Only `RECEVEUR` validates payments** — and only within their own commune.
- **Only active agents create PVs** — check `agents.status = 'ACTIF'` before insert.
- **Double-verbalization** — blocked (or warned) when same `(commune_id, intervention_id, verbalized_identifier/vehicle_plate)` exists with a non-terminal status. Configured per commune via `double_verbalisation_bloquant`.
- **Status history** — every PV status change inserts into `pv_status_history` via `pvs::record_status_change()`.
- **QR codes** — generated server-side (no external service) using the `qrcode` crate. SVG stored in `pvs.qr_code_svg`, returned only via `GET /pvs/{id}/qr`, not in the main list response.

---

## Shared Utilities (`src/helpers.rs`)

```rust
resolve_commune_filter(auth_user, requested_commune_id) // → Result<Option<Uuid>>
required_text(value, "field_name")                      // → Result<String>
clean_optional(value)                                   // → Option<String>
```

These replace the per-module duplicates. Import with `use crate::helpers::{resolve_commune_filter, required_text, clean_optional}`.

---

## Adding a New Module

1. Create `src/modules/my_module.rs` with `pub fn router() -> Router<AppState>`
2. Add `pub mod my_module;` to `src/modules/mod.rs`
3. Add `.merge(modules::my_module::router())` in `src/routes/api.rs`
4. If public endpoints needed, add `pub fn public_router()` and merge it inside the `"/public"` nest

Follow the pattern in any existing module: `list_*`, `get_*`, `create_*`, `patch_*` — each with role check → commune access check → DB query → audit.
