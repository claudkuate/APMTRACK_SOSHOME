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

## Notes Windows

Le README n'utilise pas `make`, car l'environnement cible initial est Windows PowerShell.

Si Rust n'est pas installe, utiliser Docker pour compiler et lancer l'API.

