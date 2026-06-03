# Plan de Déploiement — APMTRACK v3

**Projet :** APMTRACK — Système de Gestion des Activités de la Police Municipale  
**Architecture cible :** Rust Backend + Angular Web + Flutter Mobile + PostgreSQL  
**Organisation :** Monorepo + Docker + Docker Compose  
**Version du document :** v1.0  
**Date :** 02 juin 2026  
**Auteur :** Flysoft Engineering / Claude Kuaté  

---

## 1. Objectif du plan de déploiement

Ce document définit la stratégie de déploiement technique de la plateforme APMTRACK v3.

L’objectif est de permettre à l’équipe technique de déployer progressivement l’application dans trois contextes :

1. **Développement local**
2. **Préproduction / démonstration**
3. **Production pilote**

Le plan prend en compte :

- le backend Rust ;
- le frontend web Angular ;
- l’application mobile Flutter ;
- l’utilisation d’un monorepo ;
- Docker et Docker Compose ;
- PostgreSQL ;
- la sécurité des secrets ;
- les sauvegardes ;
- le monitoring ;
- la stratégie de rollback ;
- les contraintes terrain camerounaises : connectivité instable, coûts maîtrisés, démonstrations institutionnelles, besoin de crédibilité.

---

## 2. Architecture générale de déploiement

### 2.1 Vue logique

```txt
[Angular Web - Admin/Superviseur/Receveur]
                |
                | HTTPS
                v
        [Rust API - Axum]
                |
                | SQL
                v
          [PostgreSQL]
                |
                v
       [Stockage fichiers/PDF/QR]
```

```txt
[Flutter Mobile - Agent APM]
                |
                | HTTPS REST API
                v
        [Rust API - Axum]
                |
                v
          [PostgreSQL]
```

### 2.2 Composants principaux

| Composant | Technologie | Rôle |
|---|---|---|
| Backend API | Rust + Axum | Cœur métier, sécurité, rôles, PV, paiements |
| Web Frontend | Angular | Administration, supervision, caisse, reporting |
| Mobile | Flutter | Usage terrain des agents APM |
| Base de données | PostgreSQL | Stockage persistant des données métier |
| Reverse proxy | Nginx / Caddy / Traefik | HTTPS, routage, compression |
| Conteneurisation | Docker | Standardisation des environnements |
| Orchestration locale | Docker Compose | Lancement multi-services |
| CI/CD | GitHub Actions | Tests, build, déploiement |
| Stockage fichiers | S3 compatible / Supabase Storage / MinIO | Pièces jointes, PDF, logos, photos |
| Monitoring | UptimeRobot / Grafana / Sentry | Surveillance technique |

---

## 3. Organisation du monorepo

### 3.1 Structure recommandée

```txt
apmtrack/
├── apps/
│   ├── api/                    # Backend Rust Axum
│   ├── web/                    # Angular Web App
│   └── mobile/                 # Flutter App
│
├── packages/
│   ├── contracts/              # OpenAPI, DTO partagés, schémas
│   ├── ui/                     # Design system optionnel
│   └── shared/                 # Constantes, types, règles transversales
│
├── infra/
│   ├── docker/
│   ├── nginx/
│   ├── postgres/
│   ├── scripts/
│   └── environments/
│
├── docs/
│   ├── prd/
│   ├── architecture/
│   ├── api/
│   ├── deployment/
│   └── security/
│
├── .github/
│   └── workflows/
│
├── docker-compose.yml
├── docker-compose.dev.yml
├── docker-compose.prod.yml
├── .env.example
├── Makefile
└── README.md
```

### 3.2 Pourquoi un monorepo ?

Le monorepo est pertinent pour APMTRACK parce que les trois applications partagent le même domaine métier :

- rôles ;
- communes ;
- agents ;
- PV ;
- interventions ;
- paiements ;
- signalements ;
- référentiel ;
- statuts ;
- validations.

Il permet de garder une cohérence entre :

- les routes backend ;
- les écrans Angular ;
- les écrans Flutter ;
- les contrats API ;
- la documentation ;
- les scripts de déploiement.

### 3.3 Règle critique

Le monorepo ne doit pas devenir un désordre.

Chaque application doit rester autonome :

- `apps/api` ne dépend pas directement de `apps/web` ;
- `apps/mobile` ne contient aucune logique métier sensible ;
- les règles critiques restent côté backend ;
- les contrats API sont centralisés dans `packages/contracts`.

---

## 4. Environnements de déploiement

### 4.1 Environnement local

Objectif : développement quotidien.

Services :

- API Rust ;
- Angular ;
- PostgreSQL ;
- Redis optionnel ;
- MinIO optionnel ;
- Adminer ou PgAdmin optionnel.

Utilisation :

```bash
docker compose -f docker-compose.dev.yml up --build
```

### 4.2 Environnement de démonstration

Objectif : présenter le produit à NASLA, mairie, partenaires ou clients.

Services possibles :

- Angular sur Vercel ;
- API Rust sur Railway, Render, Fly.io ou VPS ;
- PostgreSQL managé ;
- stockage fichiers simplifié ;
- données de démonstration.

Caractéristiques :

- URL publique ;
- HTTPS ;
- comptes de test ;
- base persistante ;
- seed de démonstration ;
- monitoring basique.

### 4.3 Environnement de préproduction

Objectif : tester une version quasi production.

Caractéristiques :

- données proches de la réalité ;
- rôles configurés ;
- référentiel par commune ;
- sauvegardes automatiques ;
- logs d’audit ;
- tests de charge simples ;
- domaine de préproduction.

Exemple :

```txt
https://staging.apmtrack.cm
https://api-staging.apmtrack.cm
```

### 4.4 Environnement de production pilote

Objectif : exploitation réelle avec une commune ou une institution pilote.

Caractéristiques obligatoires :

- PostgreSQL persistant ;
- sauvegardes automatiques ;
- HTTPS ;
- logs ;
- monitoring ;
- stratégie de rollback ;
- désactivation des comptes de test ;
- gestion stricte des secrets ;
- politique de conservation des données.

Exemple :

```txt
https://app.apmtrack.cm
https://api.apmtrack.cm
```

---

## 5. Dockerisation

## 5.1 Backend Rust — Dockerfile

```dockerfile
# apps/api/Dockerfile

FROM rust:1.78 as builder

WORKDIR /app

COPY apps/api/Cargo.toml apps/api/Cargo.lock ./
COPY apps/api/src ./src
COPY apps/api/migrations ./migrations

RUN cargo build --release

FROM debian:bookworm-slim

WORKDIR /app

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/apmtrack-api /app/apmtrack-api
COPY --from=builder /app/migrations /app/migrations

EXPOSE 8080

CMD ["/app/apmtrack-api"]
```

### Points critiques

- Ne jamais embarquer `.env` dans l’image Docker.
- Les secrets doivent venir de l’environnement.
- L’image doit rester légère.
- Les migrations doivent être exécutées de manière contrôlée.

---

## 5.2 Frontend Angular — Dockerfile

```dockerfile
# apps/web/Dockerfile

FROM node:22-alpine as builder

WORKDIR /app

COPY apps/web/package*.json ./
RUN npm ci

COPY apps/web ./
RUN npm run build

FROM nginx:alpine

COPY --from=builder /app/dist/web/browser /usr/share/nginx/html
COPY infra/nginx/web.conf /etc/nginx/conf.d/default.conf

EXPOSE 80
```

### Note

Pour Vercel, Docker n’est pas obligatoire.  
Mais garder un Dockerfile Angular est utile pour :

- les tests locaux ;
- le déploiement VPS ;
- la préproduction ;
- la portabilité.

---

## 5.3 Docker Compose local

```yaml
services:
  postgres:
    image: postgres:16
    container_name: apmtrack-postgres
    environment:
      POSTGRES_DB: apmtrack
      POSTGRES_USER: apmtrack
      POSTGRES_PASSWORD: apmtrack_dev_password
    ports:
      - "5432:5432"
    volumes:
      - apmtrack_postgres_data:/var/lib/postgresql/data

  api:
    build:
      context: .
      dockerfile: apps/api/Dockerfile
    container_name: apmtrack-api
    environment:
      APP_ENV: development
      APP_PORT: 8080
      DATABASE_URL: postgres://apmtrack:apmtrack_dev_password@postgres:5432/apmtrack
      JWT_SECRET: dev_change_me
      CORS_ALLOWED_ORIGINS: http://localhost:4200
    depends_on:
      - postgres
    ports:
      - "8080:8080"

  web:
    build:
      context: .
      dockerfile: apps/web/Dockerfile
    container_name: apmtrack-web
    depends_on:
      - api
    ports:
      - "4200:80"

volumes:
  apmtrack_postgres_data:
```

---

## 6. Gestion des variables d’environnement

### 6.1 Fichier `.env.example`

```env
APP_ENV=development
APP_NAME=APMTRACK
APP_PORT=8080

DATABASE_URL=postgres://user:password@localhost:5432/apmtrack

JWT_SECRET=change_me
JWT_ACCESS_TOKEN_TTL_MINUTES=15
JWT_REFRESH_TOKEN_TTL_DAYS=7

CORS_ALLOWED_ORIGINS=http://localhost:4200

STORAGE_DRIVER=local
STORAGE_BUCKET=apmtrack

PUBLIC_APP_URL=http://localhost:4200
PUBLIC_API_URL=http://localhost:8080
```

### 6.2 Règles de sécurité

- Ne jamais commiter un vrai `.env`.
- Utiliser `.env.example` uniquement comme modèle.
- Utiliser les secrets GitHub Actions pour CI/CD.
- Utiliser les variables d’environnement Railway/Vercel/VPS en production.
- Renouveler les secrets JWT avant production.
- Isoler les secrets de staging et de production.

---

## 7. Déploiement du backend Rust

### 7.1 Option A — Railway / Render / Fly.io

Avantages :

- rapide ;
- peu d’administration serveur ;
- adapté aux démos ;
- déploiement Git simple.

Inconvénients :

- coût variable ;
- limites de ressources ;
- dépendance plateforme ;
- moins de contrôle système.

Procédure générale :

1. Créer un service backend.
2. Connecter le repo GitHub.
3. Définir le dossier racine ou Dockerfile.
4. Ajouter les variables d’environnement.
5. Connecter PostgreSQL.
6. Lancer les migrations.
7. Tester `/health`.
8. Connecter le domaine API.

Endpoint minimal attendu :

```txt
GET /health
```

Réponse :

```json
{
  "status": "ok",
  "service": "apmtrack-api",
  "environment": "production"
}
```

---

### 7.2 Option B — VPS avec Docker Compose

Avantages :

- contrôle total ;
- coût prévisible ;
- bon choix pour pilote local ;
- possibilité d’héberger API + DB + fichiers.

Inconvénients :

- responsabilité technique plus forte ;
- besoin de maintenance serveur ;
- sauvegardes à gérer sérieusement.

Architecture VPS :

```txt
VPS Ubuntu
├── Docker
├── Docker Compose
├── Nginx ou Caddy
├── API Rust
├── PostgreSQL
├── MinIO optionnel
└── scripts backup
```

Déploiement :

```bash
git clone https://github.com/flysoft/apmtrack.git
cd apmtrack
cp .env.example .env
nano .env
docker compose -f docker-compose.prod.yml up -d --build
```

Vérification :

```bash
docker ps
docker logs apmtrack-api
curl https://api.apmtrack.cm/health
```

---

## 8. Déploiement Angular Web

### 8.1 Option recommandée pour démo : Vercel

Procédure :

1. Connecter le repo GitHub à Vercel.
2. Sélectionner l’application Angular dans `apps/web`.
3. Configurer les commandes :

```txt
Install Command: npm ci
Build Command: npm run build
Output Directory: dist/web/browser
```

4. Ajouter les variables d’environnement frontend :

```env
NG_APP_API_URL=https://api-staging.apmtrack.cm
NG_APP_ENV=staging
```

5. Déployer.
6. Vérifier la connexion à l’API.

### 8.2 Option production VPS

Utiliser Docker + Nginx.

Commandes :

```bash
docker compose -f docker-compose.prod.yml up -d web
```

---

## 9. Déploiement Flutter Mobile

### 9.1 En développement

```bash
cd apps/mobile
flutter pub get
flutter run
```

### 9.2 Build Android APK

```bash
flutter build apk --release   --dart-define=API_URL=https://api-staging.apmtrack.cm   --dart-define=APP_ENV=staging
```

### 9.3 Build Android App Bundle

```bash
flutter build appbundle --release   --dart-define=API_URL=https://api.apmtrack.cm   --dart-define=APP_ENV=production
```

### 9.4 Distribution recommandée

Pour une phase pilote :

- APK signé distribué directement aux agents ;
- ou Firebase App Distribution ;
- ou Play Console en test fermé.

Pour production :

- Play Store privé ou public ;
- signature officielle ;
- gestion des versions ;
- journal des changements.

---

## 10. Base de données PostgreSQL

### 10.1 En développement

PostgreSQL via Docker Compose.

### 10.2 En staging

PostgreSQL managé ou conteneur persistant sur VPS.

### 10.3 En production

PostgreSQL managé recommandé.

Exigences :

- sauvegarde quotidienne ;
- restauration testée ;
- monitoring espace disque ;
- accès réseau restreint ;
- utilisateurs DB séparés ;
- migration contrôlée.

### 10.4 Migrations

Utiliser SQLx migrations ou outil équivalent.

Commandes possibles :

```bash
sqlx migrate run
sqlx migrate revert
```

Règle stricte :

- aucune modification manuelle de structure en production ;
- toute modification passe par migration versionnée ;
- migration testée en staging avant production.

---

## 11. Stratégie CI/CD

### 11.1 Branches

```txt
main        -> production
develop     -> staging
feature/*   -> développement
hotfix/*    -> correction urgente
release/*   -> préparation version
```

### 11.2 Pipeline recommandé

À chaque Pull Request :

1. Vérification format Rust.
2. Tests Rust.
3. Build API.
4. Tests Angular.
5. Build Angular.
6. Analyse statique.
7. Vérification Docker build.
8. Pas de déploiement automatique en production.

À chaque merge sur `develop` :

- déploiement staging.

À chaque tag `vX.Y.Z` :

- déploiement production après validation.

---

## 12. Sécurité de déploiement

## 12.1 Authentification

- JWT court pour access token.
- Refresh token stocké et révocable.
- Mot de passe hashé avec Argon2.
- Politique de mot de passe minimale.
- Désactivation compte possible.

## 12.2 Autorisation

RBAC obligatoire :

- `ADMIN`
- `APM`
- `SUPERVISEUR`
- `RECEVEUR`

Chaque action critique doit être vérifiée côté backend.

## 12.3 Isolation par commune

Un agent APM ne doit voir que les données de sa commune.

Un receveur municipal ne doit clôturer que les PV de sa commune.

Un superviseur peut lire globalement mais ne doit pas modifier.

Un administrateur peut gérer l’ensemble.

## 12.4 Protection des données

- HTTPS obligatoire.
- CORS strict.
- Audit logs.
- Pas de secrets dans le frontend.
- Pas de montant calculé côté frontend.
- Pas de validation paiement côté frontend.
- Logs sans mots de passe.
- Sauvegardes chiffrées si possible.

---

## 13. Monitoring et observabilité

### 13.1 Minimum obligatoire

- endpoint `/health` ;
- logs API ;
- surveillance uptime ;
- alertes indisponibilité ;
- surveillance CPU/RAM/disque.

### 13.2 Recommandé

- Sentry pour erreurs frontend et mobile ;
- Prometheus + Grafana pour backend ;
- Loki pour logs ;
- alertes Telegram/Email ;
- audit métier séparé des logs techniques.

### 13.3 Événements métier à auditer

- connexion utilisateur ;
- échec connexion ;
- création PV ;
- annulation PV ;
- validation paiement ;
- modification référentiel ;
- création agent ;
- suspension agent ;
- changement rôle utilisateur ;
- génération PDF ;
- consultation PV sensible.

---

## 14. Sauvegarde et restauration

### 14.1 Sauvegardes PostgreSQL

Fréquence minimale :

- staging : quotidienne ;
- production : quotidienne + hebdomadaire longue durée.

Exemple script :

```bash
#!/bin/bash
DATE=$(date +"%Y-%m-%d_%H-%M-%S")
pg_dump "$DATABASE_URL" > "backup_apmtrack_$DATE.sql"
```

### 14.2 Règle critique

Une sauvegarde non testée est une illusion.

Il faut tester la restauration au moins une fois par mois :

```bash
psql "$RESTORE_DATABASE_URL" < backup_apmtrack_YYYY-MM-DD.sql
```

### 14.3 Conservation

- backups journaliers : 7 jours ;
- backups hebdomadaires : 1 mois ;
- backups mensuels : 6 mois, selon politique institutionnelle.

---

## 15. Stratégie de rollback

### 15.1 Rollback application

Chaque version production doit être taguée :

```txt
v1.0.0
v1.0.1
v1.1.0
```

En cas d’incident :

1. Identifier la version stable précédente.
2. Redéployer l’image Docker précédente.
3. Vérifier `/health`.
4. Vérifier les flux critiques.
5. Documenter l’incident.

### 15.2 Rollback base de données

Plus délicat.

Règle :

- éviter les migrations destructives ;
- préférer les migrations compatibles arrière ;
- ne jamais supprimer une colonne critique sans période de transition ;
- sauvegarder avant migration ;
- tester en staging.

---

## 16. Plan de déploiement progressif

## Phase 0 — Préparation technique

Durée indicative : 2 à 5 jours.

Objectifs :

- créer le monorepo ;
- configurer Docker ;
- configurer PostgreSQL ;
- mettre en place le backend minimal ;
- mettre en place Angular minimal ;
- préparer Flutter minimal ;
- créer `.env.example` ;
- créer endpoint `/health`.

Livrables :

- repo initial ;
- Docker Compose local ;
- API démarrable ;
- web démarrable ;
- mobile démarrable ;
- documentation README.

Critères de validation :

- `docker compose up` fonctionne ;
- l’API répond sur `/health` ;
- Angular appelle l’API ;
- Flutter appelle l’API.

---

## Phase 1 — Déploiement local complet

Objectif :

- permettre aux développeurs de travailler sur la même base.

Services :

- API Rust ;
- Angular ;
- PostgreSQL ;
- migrations ;
- seed de démonstration.

Critères de validation :

- création utilisateur ;
- connexion ;
- création commune ;
- création agent ;
- création référentiel ;
- création PV ;
- validation paiement.

---

## Phase 2 — Staging public

Objectif :

- obtenir une version démontrable publiquement.

Déploiement :

- Angular sur Vercel ;
- API sur Railway/Render/Fly/VPS ;
- PostgreSQL persistant ;
- domaine staging ;
- HTTPS.

Critères de validation :

- accès web public ;
- API sécurisée ;
- base persistante ;
- comptes de test ;
- QR code fonctionnel ;
- PDF fonctionnel.

---

## Phase 3 — Préproduction institutionnelle

Objectif :

- simuler un usage réel.

Ajouts :

- sauvegardes ;
- monitoring ;
- audit logs ;
- sécurité CORS ;
- séparation des rôles ;
- données de test réalistes ;
- documentation utilisateur ;
- procédure de support.

Critères de validation :

- test complet du cycle PV ;
- test receveur ;
- test superviseur ;
- test admin ;
- test signalement citoyen ;
- test sauvegarde/restauration.

---

## Phase 4 — Production pilote

Objectif :

- déployer pour une commune pilote.

Préconditions obligatoires :

- validation juridique du référentiel ;
- données de commune réelles ;
- agents réels ;
- formation utilisateurs ;
- sauvegarde activée ;
- monitoring activé ;
- comptes de test supprimés ;
- domaine officiel ;
- support technique désigné.

Critères de mise en production :

- disponibilité API ;
- certificats HTTPS ;
- base sauvegardée ;
- rôles validés ;
- PV généré correctement ;
- paiement validable ;
- PDF imprimable ;
- QR code vérifiable ;
- logs d’audit actifs.

---

## 17. Checklist avant mise en production

### Technique

- [ ] Backend compilé en release.
- [ ] Frontend buildé en production.
- [ ] Mobile signé.
- [ ] PostgreSQL persistant.
- [ ] Migrations appliquées.
- [ ] Endpoint `/health` actif.
- [ ] HTTPS activé.
- [ ] CORS restreint.
- [ ] Variables d’environnement configurées.
- [ ] Logs activés.
- [ ] Monitoring actif.
- [ ] Sauvegarde testée.
- [ ] Rollback documenté.

### Sécurité

- [ ] Comptes de test supprimés.
- [ ] Mots de passe initiaux changés.
- [ ] JWT secret renouvelé.
- [ ] Accès DB non public.
- [ ] Rôles testés.
- [ ] Permissions par commune testées.
- [ ] Audit logs actifs.
- [ ] Fichiers protégés.

### Métier

- [ ] Communes configurées.
- [ ] Zones configurées.
- [ ] Référentiel validé.
- [ ] Délibérations renseignées.
- [ ] Agents enregistrés.
- [ ] Receveurs configurés.
- [ ] Superviseurs configurés.
- [ ] Cycle PV testé.
- [ ] Cycle paiement testé.
- [ ] Signalement citoyen testé.

---

## 18. Risques majeurs

| Risque | Gravité | Mesure de mitigation |
|---|---:|---|
| Perte de données | Très élevée | PostgreSQL persistant + sauvegardes |
| Mauvaise gestion des rôles | Très élevée | RBAC backend + tests |
| Montant modifié côté frontend | Très élevée | Montants calculés uniquement côté backend |
| Doublons de PV | Élevée | Contrôles métier serveur |
| Offline mobile mal maîtrisé | Élevée | Reporter offline complet après MVP |
| Hébergement gratuit instable | Moyenne/Élevée | Prévoir staging gratuit, production payante |
| QR code dépendant d’un service externe | Moyenne | Génération interne backend |
| Absence de monitoring | Élevée | Healthcheck + alertes |
| Migration DB cassante | Très élevée | Staging + backups + migrations non destructives |

---

## 19. Recommandation finale

Pour APMTRACK, le déploiement doit être progressif.

La meilleure stratégie est :

1. **Docker Compose local** pour stabiliser l’équipe.
2. **Vercel + Railway/Render/Fly.io** pour les démonstrations.
3. **VPS ou services managés payants** pour un pilote sérieux.
4. **PostgreSQL persistant avec sauvegardes** dès que l’application quitte la simple démo.
5. **Flutter mobile online-first au MVP**, puis offline contrôlé plus tard.
6. **Aucune logique métier critique côté frontend.**

Le déploiement gratuit peut servir à démontrer le produit, mais il ne doit pas être vendu comme une solution de production fiable.

APMTRACK doit être présenté comme une plateforme institutionnelle sérieuse : stable, traçable, auditable, sauvegardée et sécurisée.
