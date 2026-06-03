# APMTRACK Web Admin

Back-office Angular Phase 0.

Cette application contient uniquement un shell administratif sobre et une page de statut technique qui appelle :

- `GET /health`
- `GET /health/db`

## Configuration API

En developpement, l'URL API vient de `public/env.js`.

Dans l'image Docker, `apps/web-admin/Dockerfile` regenere ce fichier avec les build args :

- `NG_APP_API_URL`
- `NG_APP_ENV`

## Commandes

```powershell
npm install
npm run build
npm test -- --watch=false
npm start
```

