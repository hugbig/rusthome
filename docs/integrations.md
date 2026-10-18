# rusthome : catalogue des intégrations (annexe de la spec)

> Décision de Hugo (2026-10-17) : **tout reprendre**. Les 68 intégrations configurées dans HA ([inventaire/raw/integrations.json](../inventaire/raw/integrations.json)) ont chacune un traitement dans le clone :
> - **réimplémentation** ;
> - **équivalent natif**, pour les mécanismes propres à HA ;
> - **exclusion motivée** : 2 cas seulement.
>
> Celles qui sont en panne aujourd'hui (WLED, Waze, Epson, SHIELD, DLNA…) sont reprises : elles revivront avec leur appareil.

**Effort** (ordre de grandeur, une personne avec assistance) : **S** ≤ 1 jour · **M** 2 à 4 jours · **L** 1 à 2 semaines · **XL** plus de 2 semaines.
**Jalon** : voir [architecture.md §13](architecture.md).

**Approche** :
- **natif** : Rust pur, en HTTP, WebSocket ou TCP ;
- **crate** : on s'appuie sur une bibliothèque existante ;
- **sidecar** : petit conteneur Python qui réutilise la bibliothèque de référence, lorsque l'API est non officielle et sans équivalent Rust. Il communique avec le clone en HTTP local.

## 1. Cœur domotique (v1)

| Intégration HA | Entités | Approche clone | Effort | Jalon |
|---|---|---|---|---|
| `mqtt` | 261 | Natif (rumqttc), client partagé, déclencheur et service | M | M2 |
| (Z2M via mqtt) | – | Natif, intégration `z2m` dédiée : exposes, commandes, gestion | L | M2 / M5 |
| (Thingino via mqtt) | – | Natif MQTT : flux, mouvement, personne, + réglages en option | M | M2 |
| (Awtrix via mqtt) | – | Natif MQTT + module « apps Awtrix » | S | M3 |
| `esphome` ×7 | 20 | Crate `esphome-native-api`, rôle complet de hub (spec §5.3) | XL | M6 |
| `alarmo` | 1 | **Équivalent natif** : centrale d'alarme (spec §7) | L | M7 |
| `frigate` | 17 | Natif HTTP + MQTT | M | M2 |
| `onvif` ×2 | 16 | Natif SOAP minimal (profils, snapshot, PTZ) + RTSP via go2rtc | M | M2 |
| `mjpeg` (Creality K1C) | 1 | Natif : proxy MJPEG | S | M2 |
| `hue` | 77 | Natif, API v2 du pont (HTTPS + SSE) | L | M2 |
| `homekit_controller` (Netatmo) | 5 | Crate `hap-controller` (spike en M2) | L | M6 |
| `cast` | 5 | Crate `rust_cast` + mDNS | M | M6 |
| `spotify` | 1 | Natif, API Web Spotify (OAuth) ; remplace aussi `spotcast` | M | M6 |
| `alexa_media` + `alexa_devices` | 88 | Sidecar `alexapy` : notify, TTS, media_player, capteurs | L | M6 |
| `mobile_app` ×2 | 10 | **Équivalent natif** : ntfy (notifications) + OwnTracks (position, batterie) | M | M6 |
| `pushbullet` | 10 | Natif, API REST Pushbullet (conservée en canal de notification) | S | M6 |
| `portainer` | 550 | Natif, API Portainer, indexé par nom de conteneur | M | M2 |
| `ping` | 1 | Natif (ICMP, `surge-ping`) | S | M2 |

## 2. Helpers et briques du cœur (v1)

Ce ne sont pas des intégrations dans le clone, mais des **types de helpers natifs**.

| Intégration HA | Usage actuel | Approche clone | Effort | Jalon |
|---|---|---|---|---|
| `utility_meter` ×3 | Linky jour, semaine, mois | Helper natif (cycles, tarifs, reset) | M | M3 |
| `threshold` | 1 capteur | Helper natif (hystérésis, bornes) | S | M3 |
| `switch_as_x` ×3 | Prises exposées en `light` | Helper natif « changer le domaine d'une entité » | S | M3 |
| `template` (YAML) | 2 capteurs | Helper natif « capteur template » (minijinja) | M | M3 |
| `sun` | 7 | Natif (calcul astronomique, crate `sunrise`) + déclencheur et condition `sun` | S | M3 |
| `input_*`, `counter`, `tag`, `person`, `zone` | – | Helpers natifs (spec §4) | M | M1 / M3 |
| `local_calendar` | 1 (« Semaine Nina ») | Calendrier natif (iCalendar stocké, crate `icalendar`) | M | M8 |

## 3. Médias, salon, divertissement

| Intégration HA | Entités | Approche clone | Effort | Jalon |
|---|---|---|---|---|
| `music_assistant` | 12 | Natif, API WebSocket de Music Assistant (le serveur reste en conteneur) | M | M8 |
| `braviatv` | 4 | Natif, API REST Bravia (PSK) + IRCC | M | M8 |
| `androidtv_remote` (SHIELD) | 2 | Natif, protocole Android TV Remote v2 (TLS + protobuf, appairage par code) | L | M8 |
| `dlna_dmr` ×3 | 3 | Crate `rupnp` : AVTransport et RenderingControl | M | M8 |
| `dlna_dms` ×2 | 0 | Crate `rupnp` : ContentDirectory (navigation média Emby / NAS) | M | M8 |
| `emby_upcoming_media` (YAML) | 2 | Natif, API REST Emby | S | M8 |
| (Emby `media_player`, YAML) | – | Natif, API REST et WebSocket Emby | M | M8 |
| `harmony` | 2 | Natif, API WebSocket locale du hub Harmony | M | M8 |
| `unfoldedcircle` (Remote 3) | 24 | Natif, API WebSocket documentée d'Unfolded Circle | M | M8 |
| `huesyncbox` | 13 | Natif, API REST de la Sync Box (appairage par bouton) | M | M8 |
| `nanoleaf` | 3 | Natif, API REST + SSE Nanoleaf | S | M8 |
| `wled` | 0 (hors ligne) | Natif, API JSON et WebSocket WLED | M | M8 |
| `playstation_network` | 18 | Sidecar `psnawp` (API Sony non officielle) | M | M9 |
| `radio_browser` | 0 | Natif, API radio-browser.info (source média) | S | M9 |
| `yamaha_sb_remote` (SR-C20A, désactivée) | 0 | **À investiguer** : protocole de la barre de son (intégration HACS) | M ? | M9 |

## 4. Infos, cloud, maison

| Intégration HA | Entités | Approche clone | Effort | Jalon |
|---|---|---|---|---|
| `meteo_france` | 11 | Natif, API utilisée par `meteofrance-api` (non officielle) : prévisions, pluie 1 h, vigilance | M | M8 |
| `met` | 1 | Natif, API officielle met.no | S | M8 |
| `idfm` | 6 | Natif, API PRIM d'Île-de-France Mobilités (officielle, clé) | M | M8 |
| `google` (Calendar) | 4 | Natif, API Google Calendar (OAuth) | M | M8 |
| `google_cloud` (TTS) | 1 | Natif, API REST Cloud Text-to-Speech | S | M8 |
| `wyoming` (Piper VM + entrées locales) | 1 | Natif, protocole Wyoming (TCP + JSONL) : TTS, et STT et wake word pour les entrées mortes | M | M8 |
| `waze_travel_time` ×3 | 0 (en panne) | Natif, API de routage Waze (non officielle). Alternative officielle : OSRM / OpenRouteService | M | M9 |
| `tile` | 8 | Sidecar `pytile` (API non officielle) | S | M9 |
| `avanza_stock` (YAML) | 1 | Natif, API Avanza (non officielle) : nécessaire à `alarme_concentrix` | S | M8 |
| `github` | 28 | Natif (crate `octocrab`) | S | M9 |
| `oralb` (ignorée) | 0 | Décodage d'annonces BLE, via les proxies Bluetooth ESPHome | S | M9 |
| `ibeacon` (ignorée) | 0 | Décodage iBeacon BLE, via les proxies Bluetooth ESPHome | S | M9 |

## 5. Réseau, NAS, système

| Intégration HA | Entités | Approche clone | Effort | Jalon |
|---|---|---|---|---|
| `synology_dsm` | 30 | Natif, API Web DSM (disques, volumes, Surveillance Station) | M | M8 |
| `sabnzbd` | 12 | Natif, API REST SABnzbd (automatisation vivante sur l'espace disque) | S | M8 |
| `sfr_box` | 6 | Natif, API HTTP de la box | S | M8 |
| `nmap_tracker` | 10 | Natif, scan ARP / ICMP du LAN (`device_tracker`) | M | M8 |
| `systemmonitor` | 8 | Natif (crate `sysinfo`) : hôte du clone | S | M8 |
| `bluetooth` | 0 | Via les **proxies Bluetooth ESPHome** (messages BLE de l'API native), pas de Bluetooth local | L | M9 |
| `ipp` (Epson XP-6100) | 0 (en panne) | Natif, IPP sur HTTP (crate `ipp`) : niveaux d'encre, état | S | M9 |

## 6. Mécanismes propres à HA : équivalent natif

| Intégration HA | Rôle dans HA | Équivalent dans le clone | Effort | Jalon |
|---|---|---|---|---|
| `hacs` | Store de composants tiers | Sans objet : pas de plugins tiers. Le suivi des mises à jour de composants passe par `update` | – | – |
| `hassio` (Supervisor) | Add-ons, Core, OS | Entités `update` des conteneurs (Portainer, registre d'images, à la manière de Diun) + mise à jour du clone lui-même | M | M8 |
| `backup` | Sauvegardes HA | Sauvegarde native (spec §11) + entité d'état | S | M1 |
| `version` | Version disponible | Entité `update.rusthome` (releases du projet) | S | M8 |
| `watchman` | Audit des références cassées | Validateur de références natif (spec §10) + vue Santé | – | M4 |
| `analytics` | Télémétrie HA | Sans objet (aucune télémétrie) | – | – |
| `default_config_disabler` | Retrait de composants par défaut | Sans objet | – | – |
| `adaptive_lighting` (désactivée) | Température de couleur selon l'heure | Helper natif « éclairage adaptatif » (courbe soleil → mired, par groupe de lumières) | M | M9 |
| `go2rtc` | Relais de flux caméra | go2rtc de **Frigate** (déjà en place), ou conteneur go2rtc dédié | S | M2 |
| `raspberry_pi`, `rpi_power` | Matériel du Pi | Remplacés par `systemmonitor` sur l'hôte .25. Les alertes d'alimentation du Pi disparaissent avec HA OS | – | – |

## 7. Exclusions motivées

| Intégration | Raison |
|---|---|
| `zha` (ignorée) | Le ConBee II est utilisé par Zigbee2MQTT ; deux piles Zigbee ne peuvent pas partager le dongle. Le Zigbee passe exclusivement par Z2M |
| `thread` | Aucun appareil Thread ni Matter dans l'installation. À reconsidérer si un appareil Matter arrive |

## 8. Récapitulatif

| Lot | Nombre | Effort cumulé (ordre de grandeur) |
|---|---|---|
| v1, cœur domotique et helpers (M1 à M7) | 27 | Déjà inclus dans les jalons M1 à M7 |
| M8, intégrations étendues | 25 | ≈ 8 à 10 semaines |
| M9, cloud non officiel, BLE et divers | 10 | ≈ 3 à 4 semaines |
| Équivalents natifs ou sans objet | 10 | Inclus |
| Exclusions | 2 | – |

Le total dépasse 68 parce que certaines entrées HA ont été éclatées : Z2M, Thingino et Awtrix sous `mqtt`, les blocs YAML (Emby, template).

**Point de vigilance** : 6 intégrations reposent sur des API **non officielles** (Alexa, Météo-France, PSN, Tile, Waze, Avanza). Elles peuvent casser sans préavis, comme dans HA. Elles sont isolées (sidecar ou module indépendant) et signalées dans la vue Santé.
