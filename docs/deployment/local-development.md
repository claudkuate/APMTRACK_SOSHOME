# Developpement local

## Docker Compose

Commande principale :

```powershell
docker compose -f docker-compose.dev.yml up --build
```

Endpoints attendus :

- `GET http://localhost:8080/health`
- `GET http://localhost:8080/health/db`
- `GET http://localhost:8080/docs/openapi.json`
- `POST http://localhost:8080/api/v1/auth/login`
- `GET http://localhost:4200`

## Base de donnees

Parametres locaux :

```text
host: localhost
port: 5432
database: apmtrack
user: apmtrack
password: apmtrack_dev_password
```

Adminer est disponible sur `http://localhost:8081`.

Les migrations Phase 1 sont lancees automatiquement par le service API local avec `RUN_MIGRATIONS_ON_STARTUP=true`.

Pour creer le premier super administrateur :

```powershell
docker compose -f docker-compose.dev.yml run --rm `
  -e SEED_SUPER_ADMIN_EMAIL=admin@apmtrack.local `
  -e SEED_SUPER_ADMIN_PASSWORD=change_me_admin_123 `
  -e SEED_SUPER_ADMIN_FULL_NAME="APMTRACK Super Admin" `
  api seed-super-admin
```

## Notes Windows

Le README n'utilise pas `make`, car l'environnement cible initial est Windows PowerShell.

Si Rust n'est pas installe, utiliser Docker pour compiler et lancer l'API.
