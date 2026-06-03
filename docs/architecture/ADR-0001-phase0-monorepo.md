# ADR-0001 - Initialisation Phase 0 du monorepo APMTRACK

Date : 2026-06-03

## Decision

Initialiser APMTRACK comme un monorepo autonome dans `APMTRACK`, avec :

- `apps/api` pour Rust Axum ;
- `apps/web-admin` pour Angular ;
- `apps/mobile-agent` pour Flutter ;
- `packages/api-contracts` pour OpenAPI ;
- `packages/design-tokens` pour les tokens visuels initiaux ;
- `packages/shared-config` pour les conventions transversales ;
- `infra` pour Docker, Nginx et scripts.

Les noms de dossiers suivent le PRD lorsque le PRD et le plan de deploiement divergent.

## Raisons

- Le PRD vise un produit institutionnel et multi-client; le monorepo limite la derive entre backend, web, mobile et contrats API.
- Les regles metier critiques doivent rester cote backend; Angular et Flutter sont des clients.
- La Phase 0 doit rester executable sans transformer l'initialisation en MVP trop large.

## Consequences

- Le backend expose seulement `/health`, `/health/db`, `/docs/openapi.json` et la racine reservee `/api/v1/`.
- Aucun schema metier n'est cree en Phase 0.
- Les migrations SQLx existent comme emplacement, mais restent vides.
- Les choix visuels sont volontairement minimaux; une vraie direction UI devra etre validee plus tard.

