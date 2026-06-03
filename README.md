# APMTRACK

Plateforme de gestion des activites de la Police Municipale.

Ce depot contient le socle Phase 1 backend : architecture monorepo, Docker Compose local, API Rust Axum, PostgreSQL, OpenAPI, authentification JWT, RBAC serveur, communes, utilisateurs, agents et audit minimal.

Il ne contient pas encore les modules metier MVP comme referentiel, PV, QR code PV, PDF, paiements, signalements ou offline mobile.

## Structure

```text
apps/
  api/            Backend Rust Axum
  web-admin/      Back-office Angular
  mobile-agent/   Application Flutter agent
packages/
  api-contracts/  OpenAPI et contrats partages
  design-tokens/  Tokens visuels de depart
  shared-config/  Conventions partagees
infra/
  nginx/          Configuration reverse/static serving
  scripts/        Scripts locaux
docs/
  architecture/   Decisions d'architecture
  deployment/     Notes de lancement local
  security/       Regles securite Phase 0
```

## Prerequis constates

- Node.js 24.15.0 et npm 11.12.1 disponibles.
- Flutter 3.44.0 disponible.
- Docker 29.5.2 et Docker Compose v5.1.4 disponibles.
- Rust/Cargo non disponibles localement au moment de l'initialisation; le backend se valide donc via Docker ou apres installation de Rust.

## Lancement local avec Docker

```powershell
docker compose -f docker-compose.dev.yml build
docker compose -f docker-compose.dev.yml up
```

Services :

- Web admin : http://localhost:4200
- API : http://localhost:8080
- OpenAPI JSON : http://localhost:8080/docs/openapi.json
- Adminer : http://localhost:8081
- PostgreSQL : localhost:5432

Verification API :

```powershell
Invoke-RestMethod http://localhost:8080/health
Invoke-RestMethod http://localhost:8080/health/db
```

Seed du premier super administrateur local :

```powershell
docker compose -f docker-compose.dev.yml run --rm `
  -e SEED_SUPER_ADMIN_EMAIL=admin@apmtrack.local `
  -e SEED_SUPER_ADMIN_PASSWORD=change_me_admin_123 `
  -e SEED_SUPER_ADMIN_FULL_NAME="APMTRACK Super Admin" `
  api seed-super-admin
```

Connexion API :

```powershell
Invoke-RestMethod `
  -Method Post `
  -Uri http://localhost:8080/api/v1/auth/login `
  -ContentType "application/json" `
  -Body '{"email":"admin@apmtrack.local","password":"change_me_admin_123"}'
```

## Developpement sans Docker

Angular :

```powershell
cd apps/web-admin
npm install
npm run build
npm start
```

Flutter :

```powershell
cd apps/mobile-agent
flutter pub get
flutter analyze
flutter test
flutter run --dart-define=API_URL=http://10.0.2.2:8080 --dart-define=APP_ENV=development
```

Backend Rust local, apres installation de Rust :

```powershell
cd apps/api
$env:DATABASE_URL="postgres://apmtrack:apmtrack_dev_password@localhost:5432/apmtrack"
$env:JWT_SECRET="dev_change_me_at_least_16_chars"
cargo test
cargo run
```

Test d'integration backend avec PostgreSQL local :

```powershell
docker compose -f docker-compose.dev.yml up -d postgres
docker run --rm --add-host=host.docker.internal:host-gateway `
  -e APMTRACK_RUN_DB_TESTS=1 `
  -e DATABASE_URL=postgres://apmtrack:apmtrack_dev_password@host.docker.internal:5432/apmtrack `
  -v "${PWD}:/workspace" `
  -w /workspace rust:1-bookworm `
  cargo test -p apmtrack-api --test phase1_api -- --nocapture
docker compose -f docker-compose.dev.yml down
```

## Regles Phase 1

- Les montants, paiements, PV, QR code PV et PDF ne sont pas encore implementes.
- `/api/v1/` expose la fondation backend : auth, users, communes, agents et verification publique agent.
- `CITOYEN_PUBLIC` reste un usage public non authentifie, pas un role de compte utilisateur.
- Les migrations sont lancees au demarrage local via `RUN_MIGRATIONS_ON_STARTUP=true`.
- Les secrets reels ne doivent jamais etre commits.
- `public/env.js` sert a configurer Angular en local et dans l'image Docker.
- `android:usesCleartextTraffic="true"` est uniquement une facilite de developpement HTTP local.
