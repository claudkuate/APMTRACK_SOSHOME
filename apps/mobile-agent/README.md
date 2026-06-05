# APMTRACK — Agent mobile (Flutter)

Application terrain pour les **agents de Police Municipale (APM)**. Elle se connecte à l'API
APMTRACK (Rust/Axum) pour émettre des procès-verbaux, consulter les interventions et la patrouille
active, et vérifier des PV par QR code.

> **Online-first.** Le serveur reste la source de vérité : numérotation, montant et QR d'un PV
> sont **toujours** produits par le backend, y compris lors de la synchronisation d'un brouillon
> hors-ligne. Voir la section « Mode hors-ligne » plus bas.

## Prérequis

- Flutter SDK `^3.12` (Dart `^3.12`)
- L'API APMTRACK accessible (voir `docker-compose.dev.yml` à la racine du monorepo)

## Configuration

L'app lit deux variables à la compilation via `--dart-define` (défauts dans
`lib/core/config.dart`) :

| Variable  | Défaut                      | Rôle                          |
|-----------|-----------------------------|-------------------------------|
| `API_URL` | `http://192.168.1.113:8080` | URL de base de l'API          |
| `APP_ENV` | `development`               | Étiquette d'environnement     |

Valeurs `API_URL` usuelles :

- **Émulateur Android** → `http://10.0.2.2:8080`
- **Appareil physique** → l'IP LAN de la machine qui héberge l'API (ex. `http://192.168.1.113:8080`)
- **iOS simulateur / desktop** → `http://localhost:8080`

## Démarrer

```bash
flutter pub get

# Émulateur Android contre l'API Docker locale
flutter run \
  --dart-define=API_URL=http://10.0.2.2:8080 \
  --dart-define=APP_ENV=development

# Appareil Android physique → IP LAN de la machine qui héberge l'API
flutter run \
  --dart-define=API_URL=http://192.168.1.113:8080 \
  --dart-define=APP_ENV=development
```

## Dépannage — appareil physique : « API indisponible: delai depasse »

Un **timeout** au login depuis un téléphone réel n'est **pas** une erreur CORS (CORS ne
s'applique qu'aux navigateurs, jamais à une app Flutter native). C'est presque toujours un
problème de **joignabilité réseau** entre le téléphone et le PC :

1. **Même réseau WiFi.** Le téléphone et le PC doivent être sur le même sous-réseau
   (`192.168.1.x`), pas en données mobiles. Pas d'« isolation client / AP isolation » sur le
   routeur.
2. **IP correcte.** Vérifier l'IP LAN courante du PC (`ipconfig` → adresse IPv4 du WiFi) et la
   passer via `--dart-define=API_URL=http://<IP>:8080`. Le DHCP peut la changer.
3. **Pare-feu Windows.** Par défaut, Windows bloque les connexions entrantes vers le port 8080.
   Autoriser le port pour le réseau local, dans une **PowerShell en mode administrateur** :

   ```powershell
   New-NetFirewallRule -DisplayName "APMTRACK API 8080 (LAN)" `
     -Direction Inbound -Protocol TCP -LocalPort 8080 `
     -Action Allow -Profile Private -RemoteAddress LocalSubnet
   ```

   > Si le WiFi est classé « Public » sous Windows, repasser le réseau en « Privé » ou ajouter
   > `Public` au paramètre `-Profile`. Pour retirer la règle :
   > `Remove-NetFirewallRule -DisplayName "APMTRACK API 8080 (LAN)"`.

4. **Tester depuis le téléphone.** Ouvrir `http://<IP>:8080/health` dans le navigateur du
   téléphone : une réponse JSON confirme que le réseau est OK et que l'app doit fonctionner.

## Qualité

```bash
flutter analyze   # lint (gate CI)
flutter test      # tests unitaires + widgets
```

## Écrans

| Écran            | Fichier                                   | Rôle                                            |
|------------------|-------------------------------------------|-------------------------------------------------|
| Connexion        | `lib/features/auth/login_page.dart`       | Authentification agent                          |
| Accueil          | `lib/features/home/home_page.dart`        | Profil, patrouille active, derniers PV          |
| Nouveau PV       | `lib/features/pvs/create_pv_page.dart`    | Création de PV + capture GPS                     |
| Liste PV         | `lib/features/pvs/pv_list_page.dart`      | PV émis (serveur)                                |
| Détail PV / QR   | `lib/features/pvs/pv_detail_page.dart`    | Détail, QR de vérification + **photos preuve**   |
| Scan QR          | `lib/features/scan/scan_page.dart`        | Vérification publique d'un PV                    |
| Patrouille       | `lib/features/patrouille/patrouille_page.dart` | Patrouille active, envoi manuel + **suivi GPS automatique** |
| Profil           | `lib/features/profile/profile_page.dart`  | Infos agent, diagnostic, déconnexion            |

## Architecture

- `lib/core/api/api_client.dart` — contrat `ApmtrackApi` + implémentation HTTP
- `lib/core/auth/session_controller.dart` — état de session, **refresh de token automatique**
  (replay transparent sur 401) et résilience hors-ligne au démarrage
- `lib/core/auth/session_store.dart` — persistance chiffrée (`flutter_secure_storage`)
- `lib/core/models.dart` — modèles et (dé)sérialisation JSON
- `lib/features/patrouille/patrouille_tracker.dart` — suivi GPS automatique (avant-plan)
  pendant une patrouille, avec **file d'attente hors-ligne** rejouée au retour réseau.
  `location_source.dart` isole `geolocator` pour permettre les tests.
- `lib/core/offline/` — cache lecture persistant (`OfflineSnapshot`) et file de création PV
  hors-ligne (`PvDraft`), via `OfflineCacheStore` (secure storage chiffré, injectable pour tests).

## Mode hors-ligne

- **Lecture** : la dernière synchro réussie (profil, interventions, PV récents, patrouille) est
  mise en cache chiffré et réaffichée au démarrage même sans réseau. Une bannière « Mode
  hors-ligne » signale que les données proviennent du cache.
- **Création de PV** : en cas de coupure réseau, le PV est enregistré comme **brouillon local**
  (non officiel) et mis en file. À la reconnexion (à chaque synchro réussie, ou via le bouton
  « Synchroniser »), chaque brouillon est envoyé au serveur qui attribue numéro, montant et QR.
- **Stratégie de conflits** (PRD §8.4) : seules les **erreurs réseau** sont mises en file ; un
  **rejet métier** (agent suspendu, référentiel modifié, double-verbalisation, montant invalide)
  marque le brouillon en **Échec** avec le message serveur — il est conservé pour revue
  (réessayer / supprimer), jamais rejoué en boucle ni perdu silencieusement.
- La file de brouillons **survit à une déconnexion** ; le cache lecture (PII) est lui effacé.

> **Limite connue.** Le suivi automatique fonctionne tant que l'app est au premier plan
> (permission « while-in-use »). Le suivi en arrière-plan (app fermée / écran éteint) nécessite
> un *foreground service* natif — increment séparé.
