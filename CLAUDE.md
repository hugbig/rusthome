# Projet : RustHome, clone de Home Assistant en Rust + Svelte 5

Règles de production du code (tests, clippy, lancement WSL, sécurité) : @AGENTS.md

## Objectif
Recréer un équivalent de Home Assistant (HA) en **Rust** (backend) et **Svelte 5 / SvelteKit** (frontend), ciblé sur l'installation réelle de Hugo, **sans perdre les fonctionnalités clés** :
- création d'automatismes en **visuel ET en YAML** (même modèle de données, synchronisation bidirectionnelle) ;
- création et édition de **dashboards** ;
- **gestion Zigbee via Zigbee2MQTT** (appairage, renommage, OTA, groupes, bindings via l'API MQTT `zigbee2mqtt/bridge/request/...`) ;
- **gestion des users et rôles** ;
- **pièces, étages, labels, registres d'appareils et d'entités**.

## Étape 1 : inventaire, AVANT toute architecture
**Statut (2026-10-17)** : collecte faite, synthèse dans [inventaire/00-synthese.md](inventaire/00-synthese.md). Décisions de Hugo dans la synthèse, au §7. Collecte complète (§6 ; fichiers `auth` non lus, réservés à root). 

## Étape 2 : spec d'architecture
Spec v0.2 **validée** le 2026-10-17 : [docs/architecture.md](docs/architecture.md) (décisions au §12 ; Mosquitto, Z2M et le clone en Docker sur 192.168.1.25 ; **toutes** les intégrations reprises, cf. [docs/integrations.md](docs/integrations.md)). 

## Étape 3 (en cours) : jalon M1 (cœur)
Fait le 2026-10-17 : workspace Cargo (`crates/rusthome-*`), modèle, cœur, enregistreur SQLite, templates (sous-ensemble), importeur de registres, pont HA en lecture seule, API REST + WebSocket, binaire, UI SvelteKit minimale, `justfile`, données de démo. `just check` passe. Pont testé contre le vrai HA (1 353 entités). Ajoutés ensuite : UI de consultation (vue d'ensemble, entités + fiche avec historique, pièces, appareils), OIDC Authelia (docs/authelia.md, test réel avec Hugo à faire), types TS générés (`just gen-types`), image Docker (`just docker`). 

## Étape 4 : jalon M2 (intégrations natives), terminé
Fait le 2026-10-17 : registre de services routé par propriétaire d'entité, client MQTT (`rusthome-mqtt`, **lecture seule imposée** par `RUSTHOME_MQTT_READ_ONLY=true`), intégration Zigbee2MQTT (`rusthome-z2m`) qui reprend les `entity_id` HA par `unique_id` (132/142 repris sur le vrai broker), pont HA qui n'écrase plus les entités reprises, API `/services`, boutons Allumer/Éteindre. Portainer fait (220 entités reprises, client GET uniquement, conteneurs indexés par nom). Hue fait (pont 192.168.1.247, clé dans .env, 75/75 entités reprises, flux SSE, **lecture seule par défaut** `RUSTHOME_HUE_READ_ONLY=true`). Frigate fait (caméras via `/api/config` sans conserver les secrets, temps réel MQTT, comptages initiaux via les événements en cours, 10/10 entités reprises). **M2 clos le 2026-10-17** ; l'essai HomeKit est reporté à P4 par décision de Hugo (constats dans docs/architecture.md §5.4) ; entités `update.*` et sirène (composite) Z2M non reprises.

Le périmètre du clone sera fixé par l'analyse de l'existant. Accès au HA via le serveur MCP (add-on communautaire). **Rester strictement en lecture seule : aucune modification de HA.**

L'inventaire doit produire :
1. **Intégrations actives** : usage réel (nombre d'entités), protocole (MQTT, API native, cloud).
2. **Automatismes et scripts** : types de triggers, conditions, actions, templates Jinja, services appelés → définit le moteur minimal.
3. **Dashboards** : types de cartes utilisés, y compris cartes custom (HACS) → définit le catalogue de cartes.
4. **Add-ons et dépendances externes**.
5. **Le « mort »** : entités indisponibles, automatismes désactivés, intégrations sans usage (à ne pas migrer).

Livrable : un tableau **à reproduire / à simplifier / à abandonner**, puis seulement ensuite une spec d'architecture.

## Environnement connu (à confirmer par l'inventaire)
- HA OS sur Raspberry Pi 4 ; Zigbee2MQTT ; Mosquitto MQTT.
- Frigate (Docker) et caméra Sonoff PT2 sous Thingino, intégrées via MQTT.
- Alarmo (alarme), avec lecteur de badges NFC DIY (ESP32 / ESPHome).
- Boîtier IR DIY (ESP32 / ESPHome).
- Téléinfo Linky, skill Alexa custom « Simon », Piper (Wyoming), monitoring Portainer/ping.
- Reverse proxy SWAG + SSO Authelia (OIDC) déjà en place.

## Pistes d'architecture évoquées (consolidées dans docs/architecture.md)
- Backend : tokio + axum, bus d'événements, state machine, historique SQLite (sqlx), client MQTT (rumqttc).
- Import des automatismes au format YAML HA ; templates via minijinja.
- Auth déléguée à Authelia (OIDC), le clone ne gère que rôles et permissions.
- ESP32 : **décidé (2026-10-17)**, on reste sur l'API native ESPHome. Le clone embarque un client API native (pas de bascule MQTT).
- Netatmo (thermostat via HomeKit) : **décidé**, le clone intègre un contrôleur HomeKit. Alexa (TTS/annonces) conservée. Skill « Simon » abandonné.
- Comptes : admin, hugo, sokhna.
- Remplacer les notifications de l'app compagnon (ex. ntfy).
- **Migration progressive** : le clone tourne en parallèle de HA (miroir en lecture), puis reprend les automatismes un par un. Ne jamais couper HA tant que l'alarme en dépend.
- Ordre : cœur et modèle de données solides d'abord, éditeurs visuels ensuite.

## Étape 5 : jalon M3 (moteur d'automatismes), terminé le 2026-10-17 (branche `feat/m3-automatismes`)
Crate `rusthome-automation` : normalisation du format HA (anciennes clés, `device_id` → entity_id / topic MQTT, blueprints développés), compilation typée avec erreurs localisées, moteur (state, numeric_state, time, time_pattern, event, tag, mqtt, boutons Hue, startup ; modes single/restart/queued/parallel ; `for` ; traces). Import réel : 43 automatismes, 41 valides, **tous désactivés** ; 2 référencent une entité supprimée de HA (`presence_bureau`, `test_cube`). Templates : tout le corpus est couvert (`(now() - x).total_seconds()` réécrit en `seconds_since(x)`, `dict.get` via pycompat, filtre `random`). Pas encore : `sun`/`zone` (pas de localisation), `template` trigger, éditeur visuel (M5).
