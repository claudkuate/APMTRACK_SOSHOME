# Securite Phase 0

## Principes retenus

- Les secrets reels restent hors Git.
- `.env.example` documente les variables, mais `.env` est ignore.
- `JWT_SECRET` est deja exige par la configuration API pour eviter de repousser cette discipline.
- CORS est configure explicitement par environnement.
- Les endpoints publics initiaux ne renvoient aucune donnee sensible.

## Points a corriger avant pilote

- Retirer ou conditionner `android:usesCleartextTraffic="true"`.
- Remplacer tous les secrets de developpement.
- Ajouter HTTPS en staging et production.
- Ajouter rate limiting, audit log, RBAC serveur et tests de permissions.
- Valider la politique de retention et de sauvegarde PostgreSQL.

## Roles de reference

- `SUPER_ADMIN`
- `ADMIN_COMMUNE`
- `APM_AGENT`
- `SUPERVISEUR`
- `RECEVEUR`
- `CITOYEN_PUBLIC`

