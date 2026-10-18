# Projet : RustHome, clone de Home Assistant en Rust + Svelte 5

Règles de production du code (tests, clippy, lancement WSL, sécurité) : @AGENTS.md

## Objectif
Recréer un équivalent de Home Assistant (HA) en **Rust** (backend) et **Svelte 5 / SvelteKit** (frontend), ciblé sur l'installation réelle de Hugo, **sans perdre les fonctionnalités clés** :
- création d'automatisations en **visuel ET en YAML** (même modèle de données, synchronisation bidirectionnelle) ;
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
2. **Automatisations et scripts** : types de triggers, conditions, actions, templates Jinja, services appelés → définit le moteur minimal.
3. **Dashboards** : types de cartes utilisés, y compris cartes custom (HACS) → définit le catalogue de cartes.
4. **Add-ons et dépendances externes**.
5. **Le « mort »** : entités indisponibles, automatisations désactivées, intégrations sans usage (à ne pas migrer).

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
- Import des automatisations au format YAML HA ; templates via minijinja.
- Auth déléguée à Authelia (OIDC), le clone ne gère que rôles et permissions.
- ESP32 : **décidé (2026-10-17)**, on reste sur l'API native ESPHome. Le clone embarque un client API native (pas de bascule MQTT).
- Netatmo (thermostat via HomeKit) : **décidé**, le clone intègre un contrôleur HomeKit. Alexa (TTS/annonces) conservée. Skill « Simon » abandonné.
- Comptes : admin, hugo, sokhna.
- Remplacer les notifications de l'app compagnon (ex. ntfy).
- **Migration progressive** : le clone tourne en parallèle de HA (miroir en lecture), puis reprend les automatisations une par une. Ne jamais couper HA tant que l'alarme en dépend.
- Ordre : cœur et modèle de données solides d'abord, éditeurs visuels ensuite.

## Étape 5 : jalon M3 (moteur d'automatisations), terminé le 2026-10-17 (branche `feat/m3-automatismes`)
Crate `rusthome-automation` : normalisation du format HA (anciennes clés, `device_id` → entity_id / topic MQTT, blueprints développés), compilation typée avec erreurs localisées, moteur (state, numeric_state, time, time_pattern, event, tag, mqtt, boutons Hue, startup ; modes single/restart/queued/parallel ; `for` ; traces). Import réel : 43 automatisations, 41 valides, **tous désactivés** ; 2 référencent une entité supprimée de HA (`presence_bureau`, `test_cube`). Templates : tout le corpus est couvert (`(now() - x).total_seconds()` réécrit en `seconds_since(x)`, `dict.get` via pycompat, filtre `random`). Pas encore : `sun`/`zone` (pas de localisation), `template` trigger, éditeur visuel (M5).

## Étape 6 : jalon M4 (dashboards), branche `feat/m4-dashboards`
Crate `rusthome-dashboard` : document au format Lovelace, cartes natives uniquement (`lovelace::NATIVE_CARDS`). L'import convertit Mushroom, Bubble (pop-ups par `#hash`), tempometer, mini-graph, Météo-France… ; une carte inconnue devient `unsupported` (config d'origine conservée). Le normaliseur est idempotent et nettoie aussi le YAML saisi. Le validateur de références signale les entités absentes. Import HA en lecture seule (WS `lovelace/dashboards/list` + `lovelace/config`). Les images de caméra passent par `/api/v1/cameras/<id>/snapshot` : Frigate s'il possède la caméra, sinon `GET /api/camera_proxy` de HA. Frontend : `/tableaux`, vues sections / masonry / panel, mode kiosque (NsPanel, `?kiosk`, balayage entre vues), éditeur visuel (déplacer, ajouter, supprimer, YAML par carte) et YAML complet. Import réel : 9 dashboards ; Sécurité importé sans aucun point à revoir. Ajouts du 2026-10-17 (retours de Hugo) : cartes P2 natives (`media-list` pour Emby avec proxy d'images limité aux URL du capteur, `calendar` et `todo-list` lus dans HA en GET / WS `todo/item/list`, `map` sur tuiles OSM, caméras multiples, Spotify) : plus aucune carte non supportée sur les tableaux réels ; `grid_options` et `visibility` conservés. Éditeur façon HA : barre « Modification du tableau de bord », menus ⋮, vues (titre, icône, disposition), sections (grille 12 colonnes, largeur), glisser-déposer, sélecteur de cartes, formulaire par carte avec aperçu, sous-cartes en onglets, enregistrement immédiat ; lien direct `?modifier`. Menu latéral façon HA (tableaux, Carte, Journal, Historique, Média, Alarme, ESPHome, Zigbee2MQTT, Paramètres, profil) ; API `/logbook` et `/system` (sans secret). Icônes : jeu complet `static/icons/mdi.json` généré au build, chargé à la demande. Libellés d'état selon le domaine et la classe (« OK », « À jour »… au lieu de « Éteint »). Vocabulaire : « automatisations ». **Éditeur visuel des automatisations (M5) fait en avance** : blocs déclencheur / condition / action, imbrication (ET/OU/NON, choisir, si/alors/sinon, répéter, parallèle), YAML et visuel sur le même document. Ajouts du 2026-10-18 : thème refondu (`frontend/src/lib/theme.css` : jetons clair/sombre, ombres, barres de défilement fines, interrupteurs et curseurs façon HA) ; tuiles colorées par la couleur des lumières et mesures en grand avec courbe 24 h ; jauges à anneau de seuils ; météo illustrée avec prévisions (WS `weather/subscribe_forecast`, cache 10 min) ; fiche lumière (luminosité, roue chromatique, blanc) ; intégration Hue complétée (modes et couleurs comme HA, commandes `hs_color`/`rgb_color`) ; tableau **Énergie** (WS `energy/get_prefs` et `recorder/statistics_during_period`, lecture seule) ; le miroir HA amorce l'état des entités natives encore muettes (capteurs Zigbee après redémarrage). Liens directs : `?fiche=<entité>`, `/energie?periode=semaine&decalage=-1`. Énergie refaite comme HA (heure en cours par les statistiques 5 min, répartition, sources et coût, utilisation détaillée par appareil en barres empilées, comparaison à la période précédente). Navigateur **Média** comme HA : caméras en direct, Frigate natif en trois onglets Clips / Instantanés / Enregistrements (heures dans le fuseau du navigateur, `?timezone=` ; l'enregistrement continu est désactivé dans le Frigate de Hugo, seules les heures autour des événements existent ; noms et ids validés, GET seuls ; vidéos lues en HLS via `/vod/`, car les `clip.mp4` fragmentés de Frigate s'arrêtent à ≈ 4 s dans un navigateur), « Vidéos enregistrées » (médias locaux de HA, noms datés rendus lisibles, plus récent d'abord), sources HA (WS `media_source/browse_media` et `resolve_media`, fichiers relayés par RustHome en GET limité aux chemins média), lecteurs. **Tableau par défaut** (comme HA) : ouvert à `/` pour tous les comptes qui le voient, choisi par un admin (menu ⋮ du tableau, table `settings`, `PUT /api/v1/settings/default-dashboard`) ; l'aperçu est à `/apercu`. Option de tableau `tabs_icon_only` (onglets réduits à l'icône). **Compte `display`** : `RUSTHOME_DISPLAY_DASHBOARD` impose un tableau en kiosque, l'API ne lui montre que celui-ci.

## Étape 7 : jalon M5 (éditeur visuel + gestion Zigbee), fait le 2026-10-18
Gestion Zigbee2MQTT (`rusthome-z2m::bridge`, API `/api/v1/zigbee/*`, page `/zigbee`) : état du pont (`bridge/info`, `groups`, `event`), requêtes `bridge/request/<type>` par transaction (liste fermée), appairage avec compte à rebours, renommage avec **aperçu puis propagation des entity_id** (registre, état, historique, automatisations, tableaux ; `rusthome-model::refs`), OTA (vérifier, installer), retrait, groupes, liaisons. **MQTT en lecture seule : toute requête est refusée avant envoi** (vérifié sur le vrai broker : Zigbee2MQTT 2.14.2, 18 appareils). Pas encore : carte du réseau (v1.1). Conseil machine : le `justfile` limite Cargo à 2 tâches (`CARGO_BUILD_JOBS`, WSL 10 Go partagé avec VS Code).
