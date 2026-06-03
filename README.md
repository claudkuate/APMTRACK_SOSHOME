# APMTRACK

Plateforme de gestion des activites de la Police Municipale.

Ce depot est initialise en Phase 0 : architecture, squelettes executables, Docker Compose local, contrats API minimaux et documentation technique. Il ne contient pas encore les modules metier MVP comme auth, RBAC, communes, agents, PV ou paiements.

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

## Regles Phase 0

- Les montants, paiements, PV, roles et permissions ne sont pas encore implementes.
- `/api/v1/` est reserve pour la Phase 1.
- Les secrets reels ne doivent jamais etre commits.
- `public/env.js` sert a configurer Angular en local et dans l'image Docker.
- `android:usesCleartextTraffic="true"` est uniquement une facilite de developpement HTTP local.

