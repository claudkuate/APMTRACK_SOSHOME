# PRD — APMTRACK v3.0  
## Plateforme de Gestion des Activités de la Police Municipale

**Produit :** APMTRACK  
**Version du PRD :** v3.0  
**Date :** 02 juin 2026  
**Organisation porteuse :** Flysoft Engineering  
**Contexte cible :** Communes / Police Municipale / NASLA / Collectivités Territoriales Décentralisées  
**Architecture cible :** Monorepo — Backend Rust, Frontend Angular, Mobile Flutter, Docker & Docker Compose  
**Statut :** Document de cadrage produit et technique pour MVP, pilote et industrialisation  

---

# 1. Résumé exécutif

APMTRACK est une plateforme numérique destinée à structurer, tracer et sécuriser les activités de la Police Municipale au Cameroun. Elle permet aux communes de gérer leurs agents, leurs référentiels d’interventions, leurs procès-verbaux, leurs paiements, leurs signalements citoyens, leurs patrouilles et leurs opérations de terrain.

La version précédente d’APMTRACK fonctionnait comme une PWA avec une base de données en mémoire destinée aux tests. Cette nouvelle version vise une architecture plus robuste, persistante, sécurisée et industrialisable.

La solution sera développée avec :

- **Rust / Axum** pour le backend API ;
- **Angular** pour le back-office web ;
- **Flutter** pour les fonctionnalités mobiles terrain ;
- **PostgreSQL** comme base de données principale ;
- **Docker et Docker Compose** pour l’environnement local, les tests, la préproduction et la production ;
- **Monorepo** pour centraliser le code, les packages partagés, les scripts, la documentation et les configurations.

L’objectif n’est pas seulement de créer une application fonctionnelle. L’objectif est de construire une base logicielle sérieuse, maintenable, auditable et capable d’être présentée à des institutions publiques, communes, partenaires techniques ou investisseurs.

---

# 2. Contexte et justification

## 2.1 Problème actuel

Les activités de police municipale sont souvent gérées de manière fragmentée :

- référentiels d’infractions non centralisés ;
- difficultés de traçabilité des interventions ;
- risques de verbalisation non conforme ;
- manque de transparence sur les paiements ;
- difficulté à vérifier l’identité ou le statut d’un agent ;
- faible capacité de reporting ;
- absence d’historique fiable ;
- faible intégration entre terrain, administration et caisse municipale.

Une commune peut disposer de ses propres délibérations, de ses propres infractions et de ses propres sanctions. Il est donc dangereux de construire une application qui impose un référentiel unique national sans tenir compte de la réalité locale.

APMTRACK doit résoudre ce problème en donnant à chaque commune un espace de paramétrage autonome, tout en conservant une supervision globale.

## 2.2 Besoin métier central

Chaque intervention doit être :

- autorisée par une base légale ou communale ;
- rattachée à une commune ;
- rattachée à une catégorie, un type et une intervention précise ;
- traçable ;
- éventuellement soumise à paiement ;
- liée à un délai de paiement et à des pénalités éventuelles ;
- consultable par les profils autorisés ;
- imprimable ou exportable ;
- vérifiable par QR code.

Le système doit empêcher les agents de modifier arbitrairement les montants. Les montants doivent provenir du référentiel validé en backend.

## 2.3 Objectif institutionnel

APMTRACK doit pouvoir devenir :

- une solution de démonstration professionnelle ;
- un MVP exploitable dans une commune pilote ;
- une base de travail pour une solution intercommunale ;
- un produit institutionnel porté par Flysoft Engineering.

---

# 3. Objectifs produit

## 3.1 Objectifs principaux

1. Digitaliser les activités de police municipale.
2. Centraliser les référentiels d’interventions par commune.
3. Sécuriser la création des procès-verbaux.
4. Tracer les interventions des agents.
5. Faciliter la validation des paiements par le receveur municipal.
6. Permettre aux citoyens de vérifier un agent ou un PV.
7. Améliorer le reporting communal et administratif.
8. Préparer une architecture extensible vers le paiement mobile, le suivi GPS avancé et l’analyse statistique.

## 3.2 Objectifs techniques

1. Mettre en place un backend Rust robuste, performant et typé.
2. Développer un back-office Angular modulaire.
3. Développer une application Flutter mobile pour les agents terrain.
4. Utiliser PostgreSQL pour la persistance des données.
5. Utiliser Docker Compose pour standardiser l’environnement.
6. Organiser le projet en monorepo.
7. Documenter les APIs avec OpenAPI/Swagger.
8. Prévoir des tests unitaires, tests d’intégration et tests end-to-end.
9. Prévoir une stratégie de migration progressive.
10. Préparer le déploiement cloud ou VPS.

---

# 4. Périmètre du produit

## 4.1 Inclus dans le MVP

Le MVP doit inclure :

- authentification ;
- gestion des rôles ;
- gestion des communes ;
- gestion des zones géographiques ;
- gestion des agents ;
- gestion des utilisateurs système ;
- référentiel des interventions ;
- création de PV ;
- génération de numéro PV ;
- génération de QR code ;
- consultation et impression de PV ;
- validation de paiement par receveur ;
- calcul basique des pénalités ;
- signalement citoyen ;
- vérification publique d’un agent ;
- vérification publique d’un PV ;
- dashboard minimal ;
- journal d’audit ;
- documentation API ;
- déploiement Docker local.

## 4.2 Hors périmètre du MVP

À ne pas mettre dans la première version sauf contrainte client explicite :

- paiement mobile complet ;
- offline mobile complet ;
- intelligence artificielle ;
- reconnaissance faciale ;
- signature électronique avancée ;
- intégration nationale inter-administrations ;
- impression Bluetooth ;
- cartographie temps réel avancée ;
- analytics prédictif ;
- application citoyenne native complète.

## 4.3 Modules post-MVP

- paiement Mobile Money / Orange Money / MTN MoMo ;
- synchronisation offline mobile ;
- suivi GPS des patrouilles ;
- génération automatique de rapports mensuels ;
- module de missions mobiles ;
- affectation dynamique des agents ;
- notifications SMS / WhatsApp / email ;
- stockage cloud des pièces jointes ;
- portail citoyen avancé ;
- module juridique et délibérations ;
- module statistiques avancées.

---

# 5. Utilisateurs et rôles

## 5.1 Administrateur système

L’administrateur possède un accès global.

Fonctions :

- gérer les communes ;
- gérer les utilisateurs ;
- gérer les agents ;
- consulter tous les PV ;
- consulter tous les paiements ;
- gérer les référentiels ;
- consulter les signalements ;
- consulter les audits ;
- exporter les données ;
- superviser l’activité globale.

## 5.2 Administrateur communal

Profil recommandé pour une version avancée.

Fonctions :

- gérer les paramètres de sa commune ;
- gérer les agents de sa commune ;
- gérer le référentiel local ;
- consulter les PV de sa commune ;
- consulter les signalements de sa commune ;
- consulter les statistiques communales.

## 5.3 Agent APM

Profil terrain.

Fonctions :

- se connecter sur mobile ou web ;
- consulter les interventions autorisées ;
- créer un PV ;
- capturer la localisation GPS ;
- consulter ses PV ;
- démarrer ou clôturer une patrouille ;
- scanner un QR code ;
- voir les missions assignées.

Restrictions :

- ne peut pas créer de montant libre ;
- ne peut pas modifier un PV validé ;
- ne peut pas accéder aux données d’une autre commune ;
- ne peut pas valider un paiement ;
- ne peut pas modifier le référentiel.

## 5.4 Superviseur

Profil lecture et contrôle.

Fonctions :

- consulter toutes les communes ;
- rechercher des agents ;
- consulter les PV ;
- consulter les statistiques ;
- consulter les signalements ;
- vérifier les activités terrain.

Restrictions :

- pas de création de PV ;
- pas de validation de paiement ;
- pas de modification des référentiels.

## 5.5 Receveur municipal / Financier

Profil caisse.

Fonctions :

- consulter les PV en attente de paiement ;
- rechercher un PV par numéro ou QR code ;
- vérifier le montant ;
- voir les pénalités ;
- valider un paiement ;
- imprimer ou télécharger un reçu ;
- consulter l’historique des encaissements.

Restrictions :

- ne peut pas créer un PV ;
- ne peut pas modifier une infraction ;
- ne peut pas modifier un montant ;
- ne peut pas gérer les agents.

## 5.6 Citoyen

Utilisateur public sans connexion.

Fonctions :

- vérifier un agent par matricule ;
- vérifier un PV par QR code ou numéro ;
- déposer un signalement ;
- suivre un signalement avec un numéro de référence.

---

# 6. Parcours utilisateurs clés

## 6.1 Parcours administrateur — configuration d’une commune

1. L’administrateur se connecte.
2. Il crée ou sélectionne une commune.
3. Il configure les informations de la commune.
4. Il configure les zones géographiques.
5. Il crée les catégories d’intervention.
6. Il crée les types d’intervention.
7. Il crée les interventions précises.
8. Il associe les montants, délais, pénalités et références légales.
9. Il active le référentiel.
10. Les agents de la commune peuvent désormais verbaliser selon ce référentiel.

## 6.2 Parcours agent — création d’un PV

1. L’agent se connecte.
2. Le système vérifie son statut.
3. L’agent accède au module de création de PV.
4. Il sélectionne une catégorie.
5. Il sélectionne un type.
6. Il sélectionne une intervention.
7. Le système affiche automatiquement le montant, le délai et les pénalités.
8. L’agent saisit les informations du verbalisé.
9. Il renseigne la localisation.
10. Il capture la position GPS si disponible.
11. Il soumet le PV.
12. Le backend génère le numéro du PV.
13. Le backend génère le QR code.
14. Le système affiche ou imprime le PV.
15. Le PV passe au statut `EN_ATTENTE_PAIEMENT`, sauf s’il s’agit d’un avertissement ou d’une intervention non payante.

## 6.3 Parcours receveur — validation d’un paiement

1. Le receveur se connecte.
2. Il ouvre le module caisse.
3. Il recherche le PV par numéro ou scan QR code.
4. Le système affiche le montant initial.
5. Le système calcule les pénalités éventuelles.
6. Le receveur saisit le montant encaissé.
7. Le système valide ou refuse selon les règles.
8. Le PV passe au statut `PAYE`.
9. Un reçu est généré.
10. L’action est enregistrée dans l’audit log.

## 6.4 Parcours citoyen — vérification d’un agent

1. Le citoyen ouvre le portail public.
2. Il saisit le matricule de l’agent.
3. Le système affiche le nom, la commune, le grade et le statut.
4. Si l’agent est suspendu, le système l’indique clairement.

## 6.5 Parcours citoyen — signalement

1. Le citoyen ouvre le portail public.
2. Il choisit la commune concernée.
3. Il saisit le type d’incident.
4. Il renseigne le lieu et la description.
5. Il peut ajouter son contact ou rester anonyme.
6. Le système génère un numéro de signalement.
7. L’administration peut traiter le signalement.

---

# 7. Exigences fonctionnelles détaillées

## 7.1 Authentification

### Description

Le système doit permettre aux utilisateurs autorisés de se connecter de manière sécurisée.

### Fonctionnalités

- connexion par email et mot de passe ;
- hash des mots de passe avec Argon2 ;
- access token JWT ;
- refresh token ;
- déconnexion ;
- révocation de session ;
- expiration de token ;
- blocage compte inactif ou suspendu.

### Critères d’acceptation

- un utilisateur inactif ne peut pas se connecter ;
- un mot de passe incorrect est refusé ;
- un token expiré est rejeté ;
- un rôle non autorisé ne peut pas accéder à un module interdit.

---

## 7.2 Gestion des rôles et permissions

### Rôles minimum

- `SUPER_ADMIN`
- `ADMIN_COMMUNE`
- `APM_AGENT`
- `SUPERVISEUR`
- `RECEVEUR`
- `CITOYEN_PUBLIC`

### Règles

- les permissions doivent être vérifiées côté backend ;
- Angular et Flutter ne doivent jamais être considérés comme sources de vérité ;
- chaque requête sensible doit être auditée ;
- les agents doivent être limités à leur commune.

---

## 7.3 Gestion des communes

### Fonctionnalités

- créer une commune ;
- modifier une commune ;
- désactiver une commune ;
- gérer le logo ;
- gérer la couleur de thème ;
- gérer les contacts ;
- gérer les zones ;
- gérer le référentiel local.

### Champs

- `id`
- `code`
- `nom`
- `region`
- `departement`
- `adresse`
- `telephone`
- `email`
- `site_web`
- `logo_url`
- `theme_color`
- `active`

---

## 7.4 Gestion des zones

### Types de zones

- quartier ;
- bloc ;
- secteur ;
- lieu-dit ;
- marché ;
- axe routier ;
- zone commerciale ;
- zone sensible.

### Règles

- une zone appartient à une commune ;
- une zone peut avoir une zone parente ;
- une zone peut être active ou inactive ;
- une zone inactive ne doit pas être proposée dans les nouveaux PV.

---

## 7.5 Gestion des agents

### Fonctionnalités

- créer un agent ;
- modifier un agent ;
- suspendre un agent ;
- réactiver un agent ;
- mettre un agent à la retraite ;
- importer des agents par CSV ;
- associer une photo ;
- rechercher par matricule ;
- vérifier publiquement un agent.

### Champs

- matricule ;
- nom complet ;
- commune ;
- grade ;
- statut ;
- date de prise de fonction ;
- formation NASLA ;
- photo ;
- téléphone ;
- email optionnel.

### Règles critiques

- un agent suspendu ne peut pas créer de PV ;
- un agent retraité ne peut pas se connecter comme agent terrain ;
- le matricule doit être unique ;
- chaque action sensible sur un agent doit être auditée.

---

## 7.6 Référentiel des interventions

### Hiérarchie

```text
Commune
 └── Catégorie d’intervention
      └── Type d’intervention
           └── Intervention
```

### Catégorie

Exemples :

- Verbalisation ;
- Saisie ;
- Scellés ;
- Fermeture ;
- Signalement ;
- Contrôle ;
- Avertissement.

### Type

Exemples :

- Espace public ;
- Insalubrité ;
- Marchandises ;
- Magasin ;
- Citoyen ;
- Nuisances sonores ;
- Commerce ambulant.

### Intervention

Exemples :

- stationnement illicite ;
- dépôt sauvage d’ordures ;
- occupation illicite du trottoir ;
- nuisance sonore nocturne ;
- vente ambulante illicite ;
- fermeture boutique sans autorisation ;
- avertissement espace public.

### Champs intervention

- nom ;
- description ;
- commune ;
- catégorie ;
- type ;
- sujet à paiement ;
- montant ;
- délai de paiement ;
- taux de pénalité ;
- référence de délibération ;
- pièce justificative ;
- statut actif/inactif.

### Règles critiques

- une intervention payante doit avoir une référence de délibération ;
- une intervention payante doit avoir un montant ;
- l’agent ne peut pas modifier le montant ;
- une intervention inactive ne peut pas être utilisée dans un nouveau PV ;
- le référentiel d’une commune ne doit pas apparaître dans une autre commune.

---

## 7.7 Création des PV

### Fonctionnalités

- création de PV par agent ;
- sélection intervention en cascade ;
- saisie du verbalisé ;
- localisation ;
- capture GPS ;
- génération numéro PV ;
- génération QR code ;
- génération PDF ;
- impression ;
- suivi statut paiement.

### Statuts PV

- `BROUILLON`
- `EMIS`
- `EN_ATTENTE_PAIEMENT`
- `PAYE`
- `EN_RETARD`
- `ANNULE`
- `CONTESTE`
- `NON_PAYANT`

### Règles

- seul un agent actif peut créer un PV ;
- le montant vient du référentiel ;
- le numéro PV est généré côté serveur ;
- le QR code est généré côté serveur ;
- une intervention non payante ne doit pas créer de paiement attendu ;
- toute modification de statut doit être historisée.

---

## 7.8 Prévention de double verbalisation

### Problème

Un citoyen ne doit pas être verbalisé deux fois pour la même infraction pendant une période de validité définie.

### Règles MVP

Le système doit alerter si une verbalisation similaire existe déjà selon :

- même commune ;
- même type d’intervention ;
- même intervention ;
- même identifiant verbalisé si disponible ;
- même plaque véhicule si disponible ;
- période de validité active.

### Décision MVP

Le système affiche une alerte bloquante ou non bloquante selon la configuration de la commune.

### Version stricte

Pour la V2, le système peut empêcher totalement la création du PV en doublon.

---

## 7.9 Paiements et caisse

### Fonctionnalités

- liste des PV en attente ;
- recherche par numéro PV ;
- scan QR code ;
- calcul pénalité ;
- validation paiement ;
- génération reçu ;
- historique de paiements ;
- export journalier.

### Règles

- seul le receveur peut valider un paiement ;
- un paiement validé ne peut pas être supprimé ;
- une correction doit passer par une annulation contrôlée ;
- le montant encaissé doit être cohérent avec le montant dû ;
- les pénalités doivent être calculées côté backend.

---

## 7.10 Signalements citoyens

### Fonctionnalités

- création sans compte ;
- signalement anonyme possible ;
- numéro de suivi ;
- rattachement commune ;
- statut de traitement ;
- note administrative ;
- consultation par admin/superviseur.

### Statuts

- `RECU`
- `EN_COURS`
- `TRAITE`
- `CLASSE`
- `REJETE`

---

## 7.11 Patrouilles

### MVP

- créer une patrouille ;
- affecter des agents ;
- définir une zone ;
- démarrer ;
- clôturer ;
- consulter l’historique.

### Post-MVP

- tracking GPS ;
- carte temps réel ;
- mission mobile avec trajet ;
- preuve de passage ;
- incident pendant patrouille.

---

## 7.12 QR code

### Règles

Le QR code ne doit pas dépendre d’un service externe gratuit.

Le backend doit générer le QR code localement.

Le QR code peut pointer vers :

```text
https://domain.cm/public/pv/{pv_number}/verify?token={signed_token}
```

### Données affichées publiquement

- numéro PV ;
- commune ;
- date ;
- statut ;
- montant ;
- statut paiement ;
- agent partiellement masqué ou vérifiable ;
- message de validité.

### Données à ne pas exposer publiquement

- adresse complète du verbalisé ;
- numéro de pièce ;
- téléphone ;
- notes internes ;
- coordonnées GPS exactes ;
- historique administratif complet.

---

## 7.13 PDF

### Documents à générer

- PV ;
- reçu de paiement ;
- récapitulatif intervention ;
- fiche agent ;
- rapport communal ;
- journal caisse.

### Règles

- les PDF doivent être générés côté backend ;
- les PDF doivent avoir un identifiant unique ;
- les PDF importants doivent être historisés ;
- les PDF ne doivent pas dépendre du rendu Angular.

---

# 8. Exigences mobiles Flutter

## 8.1 Objectif mobile

L’application Flutter est destinée prioritairement aux agents terrain.

## 8.2 Fonctionnalités MVP mobile

- connexion agent ;
- profil agent ;
- liste des interventions disponibles ;
- création de PV ;
- capture GPS ;
- consultation des PV créés ;
- affichage QR code ;
- scan QR code ;
- synchronisation online ;
- consultation patrouille active.

## 8.3 Fonctionnalités post-MVP mobile

- offline-first ;
- cache local SQLite ;
- file d’attente de synchronisation ;
- impression Bluetooth ;
- notifications push ;
- géolocalisation de patrouille ;
- upload photo preuve ;
- signature du verbalisé ;
- mode faible connexion.

## 8.4 Position critique sur l’offline

L’offline complet ne doit pas être livré dans le MVP sans stratégie sérieuse de conflits.

Risques :

- création de doublons ;
- agent suspendu pendant qu’il est hors ligne ;
- référentiel modifié pendant qu’il est hors ligne ;
- montant obsolète ;
- GPS falsifiable ;
- numérotation PV incohérente ;
- paiement déjà effectué non visible localement.

Décision MVP :

- mobile online-first ;
- cache de lecture autorisé ;
- création officielle de PV validée par serveur ;
- offline complet reporté en V2.

---

# 9. Exigences frontend Angular

## 9.1 Objectif web

Angular doit servir de back-office administratif et opérationnel.

## 9.2 Modules Angular

- authentification ;
- dashboard ;
- communes ;
- zones ;
- agents ;
- utilisateurs ;
- référentiel ;
- PV ;
- paiements ;
- signalements ;
- patrouilles ;
- recherche avancée ;
- rapports ;
- paramètres ;
- audit logs.

## 9.3 Architecture Angular recommandée

```text
apps/web-admin/
 └── src/app/
      ├── core/
      ├── shared/
      ├── layout/
      ├── features/
      │    ├── auth/
      │    ├── dashboard/
      │    ├── communes/
      │    ├── agents/
      │    ├── referentiel/
      │    ├── pv/
      │    ├── payments/
      │    ├── signalements/
      │    ├── patrouilles/
      │    └── reports/
      └── app.routes.ts
```

## 9.4 Design system

Orientation recommandée :

- interface administrative claire ;
- design sobre et institutionnel ;
- couleurs inspirées du Cameroun mais sans surcharge ;
- tableaux lisibles ;
- filtres puissants ;
- états visuels clairs ;
- responsive desktop/tablette ;
- accessibilité correcte.

---

# 10. Exigences backend Rust

## 10.1 Stack backend

- Rust stable ;
- Axum ;
- Tokio ;
- SQLx ;
- PostgreSQL ;
- tower-http ;
- serde ;
- validator ;
- jsonwebtoken ;
- argon2 ;
- utoipa / Swagger UI ;
- tracing ;
- thiserror / anyhow ;
- uuid ;
- chrono ;
- qrcode ;
- printpdf ou génération via service dédié.

## 10.2 Architecture backend

```text
apps/api/
 ├── src/
 │   ├── main.rs
 │   ├── config/
 │   ├── database/
 │   ├── modules/
 │   │    ├── auth/
 │   │    ├── users/
 │   │    ├── communes/
 │   │    ├── agents/
 │   │    ├── referentiel/
 │   │    ├── pv/
 │   │    ├── payments/
 │   │    ├── signalements/
 │   │    ├── patrouilles/
 │   │    ├── files/
 │   │    └── audit/
 │   ├── middlewares/
 │   ├── errors/
 │   └── shared/
 ├── migrations/
 ├── tests/
 ├── Cargo.toml
 └── Dockerfile
```

## 10.3 Principes backend

- validation stricte des entrées ;
- permissions côté serveur ;
- transactions SQL sur opérations sensibles ;
- audit log systématique ;
- erreurs normalisées ;
- pagination obligatoire ;
- soft delete sur données sensibles ;
- migrations versionnées ;
- pas de logique métier critique dans le frontend.

---

# 11. Monorepo

## 11.1 Objectif du monorepo

Le monorepo permet de centraliser :

- backend Rust ;
- frontend Angular ;
- mobile Flutter ;
- documentation ;
- scripts ;
- docker-compose ;
- contrats API ;
- migrations ;
- configurations d’environnement ;
- CI/CD.

## 11.2 Structure recommandée

```text
apmtrack/
 ├── apps/
 │   ├── api/                  # Backend Rust Axum
 │   ├── web-admin/            # Angular back-office
 │   ├── mobile-agent/         # Flutter mobile
 │   └── public-portal/        # Optionnel : portail public séparé
 │
 ├── packages/
 │   ├── api-contracts/        # OpenAPI, schémas JSON, types générés
 │   ├── shared-config/        # configs lint, format, conventions
 │   └── design-tokens/        # couleurs, typographie, tokens UI
 │
 ├── infra/
 │   ├── docker/
 │   ├── nginx/
 │   ├── postgres/
 │   └── scripts/
 │
 ├── docs/
 │   ├── prd/
 │   ├── architecture/
 │   ├── api/
 │   ├── deployment/
 │   └── user-guides/
 │
 ├── docker-compose.yml
 ├── docker-compose.dev.yml
 ├── docker-compose.prod.yml
 ├── .env.example
 ├── Makefile
 ├── README.md
 └── .github/
      └── workflows/
```

## 11.3 Gestion des branches

Branches recommandées :

- `main` : stable production ;
- `develop` : intégration ;
- `feature/*` : nouvelles fonctionnalités ;
- `fix/*` : corrections ;
- `release/*` : préparation livraison.

## 11.4 Conventions de commits

Format recommandé :

```text
feat(api): add pv creation endpoint
fix(web): correct commune filter
docs(prd): update payment workflow
chore(docker): add postgres healthcheck
```

---

# 12. Docker et Docker Compose

## 12.1 Objectif

Docker doit permettre à n’importe quel développeur de lancer tout l’environnement localement avec une commande.

## 12.2 Services minimum

- `api` : backend Rust ;
- `web-admin` : Angular ;
- `postgres` : base de données ;
- `redis` : cache optionnel ;
- `minio` : stockage fichiers local compatible S3 ;
- `mailhog` : test emails optionnel ;
- `adminer` ou `pgadmin` : administration DB.

## 12.3 Exemple docker-compose cible

```yaml
services:
  postgres:
    image: postgres:16
    container_name: apmtrack_postgres
    environment:
      POSTGRES_DB: apmtrack
      POSTGRES_USER: apmtrack
      POSTGRES_PASSWORD: apmtrack_password
    ports:
      - "5432:5432"
    volumes:
      - apmtrack_pgdata:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U apmtrack"]
      interval: 10s
      timeout: 5s
      retries: 5

  api:
    build:
      context: ./apps/api
      dockerfile: Dockerfile
    container_name: apmtrack_api
    depends_on:
      postgres:
        condition: service_healthy
    environment:
      DATABASE_URL: postgres://apmtrack:apmtrack_password@postgres:5432/apmtrack
      JWT_SECRET: change_me
      APP_ENV: development
    ports:
      - "8080:8080"

  web-admin:
    build:
      context: ./apps/web-admin
      dockerfile: Dockerfile
    container_name: apmtrack_web_admin
    depends_on:
      - api
    ports:
      - "4200:80"

volumes:
  apmtrack_pgdata:
```

## 12.4 Makefile recommandé

```makefile
up:
	docker compose up -d

down:
	docker compose down

logs:
	docker compose logs -f

api-dev:
	cd apps/api && cargo watch -x run

web-dev:
	cd apps/web-admin && npm run start

mobile-dev:
	cd apps/mobile-agent && flutter run

migrate:
	cd apps/api && sqlx migrate run

test:
	cd apps/api && cargo test
```

---

# 13. Base de données

## 13.1 Tables principales

- `users`
- `roles`
- `user_roles`
- `communes`
- `zones`
- `agents`
- `categories`
- `intervention_types`
- `interventions`
- `pvs`
- `pv_status_history`
- `payments`
- `signalements`
- `patrouilles`
- `patrouille_agents`
- `patrouille_positions`
- `attachments`
- `audit_logs`
- `refresh_tokens`

## 13.2 Principes

- identifiants UUID ;
- timestamps `created_at`, `updated_at`;
- soft delete sur entités sensibles ;
- contraintes d’unicité ;
- clés étrangères ;
- index sur recherche ;
- transactions pour PV et paiements ;
- migrations SQLx versionnées.

## 13.3 Données seed

En développement, fournir :

- communes de démonstration ;
- agents de démonstration ;
- utilisateurs de test ;
- référentiel YDE1 ;
- PV de démonstration.

Attention : les comptes de test ne doivent jamais être activés en production.

---

# 14. API REST

## 14.1 Principes

- API REST versionnée ;
- préfixe `/api/v1`;
- JSON ;
- erreurs normalisées ;
- pagination ;
- filtres ;
- tri ;
- OpenAPI généré ;
- CORS contrôlé.

## 14.2 Endpoints indicatifs

### Auth

```text
POST /api/v1/auth/login
POST /api/v1/auth/refresh
POST /api/v1/auth/logout
GET  /api/v1/auth/me
```

### Communes

```text
GET    /api/v1/communes
POST   /api/v1/communes
GET    /api/v1/communes/{id}
PATCH  /api/v1/communes/{id}
DELETE /api/v1/communes/{id}
```

### Agents

```text
GET   /api/v1/agents
POST  /api/v1/agents
GET   /api/v1/agents/{id}
PATCH /api/v1/agents/{id}
POST  /api/v1/agents/{id}/suspend
POST  /api/v1/agents/{id}/reactivate
GET   /api/v1/public/agents/verify/{matricule}
```

### Référentiel

```text
GET  /api/v1/communes/{commune_id}/categories
POST /api/v1/communes/{commune_id}/categories
GET  /api/v1/communes/{commune_id}/intervention-types
POST /api/v1/communes/{commune_id}/intervention-types
GET  /api/v1/communes/{commune_id}/interventions
POST /api/v1/communes/{commune_id}/interventions
```

### PV

```text
GET  /api/v1/pvs
POST /api/v1/pvs
GET  /api/v1/pvs/{id}
GET  /api/v1/pvs/{id}/pdf
GET  /api/v1/pvs/{id}/qr
GET  /api/v1/public/pvs/verify/{pv_number}
```

### Paiements

```text
GET  /api/v1/payments
POST /api/v1/payments
GET  /api/v1/payments/{id}/receipt
```

### Signalements

```text
POST /api/v1/public/signalements
GET  /api/v1/signalements
GET  /api/v1/signalements/{id}
PATCH /api/v1/signalements/{id}/status
```

---

# 15. Sécurité

## 15.1 Exigences minimales

- HTTPS en production ;
- hash Argon2 ;
- JWT court ;
- refresh token stocké et révocable ;
- validation stricte ;
- RBAC serveur ;
- CORS contrôlé ;
- rate limiting ;
- logs d’audit ;
- désactivation comptes test ;
- sauvegardes chiffrées ;
- séparation environnement dev/staging/prod.

## 15.2 Données sensibles

Données à protéger :

- informations des citoyens ;
- numéros de pièces ;
- contacts ;
- coordonnées GPS ;
- PV ;
- paiements ;
- identité des agents ;
- pièces jointes ;
- notes administratives.

## 15.3 Audit log

Chaque action critique doit enregistrer :

- utilisateur ;
- rôle ;
- action ;
- entité concernée ;
- ancienne valeur si nécessaire ;
- nouvelle valeur si nécessaire ;
- adresse IP ;
- user agent ;
- date et heure.

Actions à auditer :

- connexion ;
- création PV ;
- modification référentiel ;
- suspension agent ;
- validation paiement ;
- annulation PV ;
- changement statut signalement ;
- export données.

---

# 16. Performance et disponibilité

## 16.1 Objectifs MVP

- temps de réponse API inférieur à 500 ms pour les requêtes simples ;
- pagination obligatoire au-delà de 50 lignes ;
- dashboard chargé en moins de 3 secondes ;
- application utilisable en faible connexion ;
- backend capable de tourner sur une petite instance VPS.

## 16.2 Optimisations

- index PostgreSQL ;
- compression HTTP ;
- cache pour référentiels ;
- lazy loading Angular ;
- pagination côté serveur ;
- limitation des pièces jointes ;
- génération PDF asynchrone si nécessaire.

---

# 17. Déploiement

## 17.1 Déploiement démo

Possible :

- Angular sur Vercel ;
- Rust API sur Railway/Fly.io/Render ;
- PostgreSQL sur Railway/Supabase/Neon.

## 17.2 Déploiement production recommandé

Option 1 — VPS Dockerisé :

- Nginx ;
- Docker Compose ;
- PostgreSQL ;
- API Rust ;
- Angular build statique ;
- MinIO ou S3 externe ;
- backups ;
- monitoring.

Option 2 — Cloud managé :

- Vercel pour Angular ;
- Railway/Fly.io/Render pour API ;
- PostgreSQL managé ;
- S3 compatible pour fichiers.

## 17.3 Attention critique

Le gratuit peut servir pour une démo. Il ne doit pas être vendu comme une base fiable de production.

---

# 18. CI/CD

## 18.1 Pipeline recommandé

À chaque pull request :

- format Rust ;
- clippy ;
- cargo test ;
- build API ;
- npm install ;
- build Angular ;
- flutter analyze ;
- tests Flutter ;
- vérification docker build.

## 18.2 GitHub Actions

Workflows :

- `ci-api.yml`
- `ci-web.yml`
- `ci-mobile.yml`
- `docker-build.yml`
- `release.yml`

---

# 19. Tests

## 19.1 Tests backend

- tests unitaires services ;
- tests d’intégration API ;
- tests SQL ;
- tests permissions ;
- tests génération PV ;
- tests calcul pénalités ;
- tests validation paiement.

## 19.2 Tests frontend Angular

- tests composants critiques ;
- tests guards ;
- tests services API ;
- tests formulaires ;
- tests navigation par rôle.

## 19.3 Tests Flutter

- tests widgets ;
- tests navigation ;
- tests formulaires PV ;
- tests scan QR ;
- tests couche API.

## 19.4 Scénarios end-to-end

1. Admin crée une commune.
2. Admin crée un référentiel.
3. Admin crée un agent.
4. Agent crée un PV.
5. Receveur valide le paiement.
6. Citoyen vérifie le PV.
7. Superviseur consulte les statistiques.

---

# 20. Roadmap

## Phase 0 — Cadrage technique

Durée indicative : 1 à 2 semaines

Livrables :

- monorepo initial ;
- Docker Compose ;
- PostgreSQL ;
- squelette API Rust ;
- squelette Angular ;
- squelette Flutter ;
- conventions de code ;
- PRD validé.

## Phase 1 — Backend fondation

Durée indicative : 3 à 5 semaines

Livrables :

- auth ;
- RBAC ;
- communes ;
- utilisateurs ;
- agents ;
- migrations ;
- audit logs ;
- OpenAPI.

## Phase 2 — Référentiel et PV

Durée indicative : 4 à 6 semaines

Livrables :

- catégories ;
- types ;
- interventions ;
- création PV ;
- QR code ;
- PDF ;
- statuts PV ;
- pénalités simples.

## Phase 3 — Angular Back-office

Durée indicative : 4 à 6 semaines

Livrables :

- dashboard ;
- gestion communes ;
- gestion agents ;
- gestion référentiel ;
- gestion PV ;
- caisse ;
- signalements ;
- recherche.

## Phase 4 — Flutter Agent

Durée indicative : 4 à 6 semaines

Livrables :

- connexion ;
- profil agent ;
- création PV ;
- capture GPS ;
- scan QR ;
- historique agent.

## Phase 5 — Pilote terrain

Durée indicative : 4 semaines

Livrables :

- tests utilisateurs ;
- corrections ;
- durcissement sécurité ;
- sauvegardes ;
- monitoring ;
- documentation utilisateur.

---

# 21. Critères de réussite

Le produit est considéré comme prêt pour un pilote si :

- les données persistent après redémarrage ;
- les rôles sont correctement isolés ;
- un agent suspendu ne peut pas créer de PV ;
- un agent ne peut pas modifier un montant ;
- une commune ne voit pas le référentiel d’une autre commune ;
- un PV peut être généré avec QR code ;
- un paiement peut être validé par receveur ;
- un citoyen peut vérifier un PV ;
- un citoyen peut vérifier un agent ;
- les actions sensibles sont auditées ;
- l’application fonctionne via Docker Compose ;
- les APIs sont documentées ;
- les tests critiques passent.

---

# 22. Risques et mesures de mitigation

## 22.1 Risque : complexité Rust

### Impact

Ralentissement du développement.

### Mitigation

- architecture simple ;
- modules clairs ;
- conventions strictes ;
- documentation ;
- commencer par API REST classique ;
- éviter microservices au départ.

## 22.2 Risque : scope trop large

### Impact

Retard, dette technique, produit inachevé.

### Mitigation

- MVP strict ;
- offline repoussé ;
- paiement mobile repoussé ;
- reporting avancé repoussé.

## 22.3 Risque : données sensibles

### Impact

Perte de confiance, problème juridique, exposition citoyenne.

### Mitigation

- RBAC ;
- audit logs ;
- chiffrement ;
- sauvegardes ;
- logs contrôlés ;
- masquage données publiques.

## 22.4 Risque : infrastructure gratuite

### Impact

Indisponibilité, perte de crédibilité.

### Mitigation

- gratuit uniquement pour démo ;
- pilote sur plan payant ou VPS ;
- backups ;
- monitoring.

## 22.5 Risque : offline mal conçu

### Impact

Doublons, incohérences, PV invalides.

### Mitigation

- online-first en MVP ;
- offline lecture seulement ;
- offline écriture en V2 avec stratégie de conflits.

---

# 23. Décisions techniques actées

| Sujet | Décision |
|---|---|
| Backend | Rust avec Axum |
| Frontend web | Angular |
| Mobile | Flutter |
| Base de données | PostgreSQL |
| Architecture repo | Monorepo |
| Déploiement local | Docker Compose |
| API | REST v1 |
| Documentation API | OpenAPI / Swagger |
| Auth | JWT + refresh token |
| Mot de passe | Argon2 |
| Stockage fichiers | S3 compatible en post-MVP |
| QR code | Généré côté backend |
| PDF | Généré côté backend |
| Offline | Non complet dans MVP |
| Paiement mobile | Post-MVP |
| Production gratuite | Non recommandée |

---

# 24. Backlog MVP priorisé

## Priorité P0 — Indispensable

- Authentification ;
- rôles ;
- communes ;
- agents ;
- référentiel ;
- PV ;
- paiements ;
- QR code ;
- PDF ;
- Docker Compose ;
- PostgreSQL ;
- audit logs.

## Priorité P1 — Important

- dashboard ;
- signalements ;
- recherche avancée ;
- import CSV ;
- exports ;
- vérification publique agent ;
- vérification publique PV.

## Priorité P2 — Après MVP

- Flutter avancé ;
- patrouilles GPS ;
- paiement mobile ;
- notifications ;
- offline ;
- stockage cloud ;
- rapports avancés.

---

# 25. Annexes

## 25.1 Exemple de statut agent

```text
ACTIF
SUSPENDU
RETRAITE
INACTIF
```

## 25.2 Exemple de statut paiement

```text
NON_APPLICABLE
EN_ATTENTE
PAYE
PARTIEL
EN_RETARD
ANNULE
```

## 25.3 Exemple de statut intervention

```text
ACTIVE
INACTIVE
ARCHIVEE
```

## 25.4 Exemple de statut signalement

```text
RECU
EN_COURS
TRAITE
CLASSE
REJETE
```

---

# 26. Conclusion

APMTRACK v3.0 doit être traité comme un produit institutionnel, pas comme une simple application CRUD.

Le choix Rust + Angular + Flutter est pertinent si l’équipe accepte une discipline technique stricte. Rust donne une base backend performante et fiable. Angular permet un back-office structuré. Flutter permet une vraie expérience mobile terrain. Le monorepo et Docker Compose permettent de professionnaliser le développement, l’intégration et les déploiements.

La priorité absolue est de livrer un MVP stable, persistant, sécurisé et démontrable, avant d’ajouter les fonctionnalités lourdes comme l’offline complet, le paiement mobile et le tracking GPS avancé.
