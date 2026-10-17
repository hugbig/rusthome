# rusthome : spécification d'architecture (v0.1)

> Statut : **v0.2, validée par Hugo le 2026-10-17** (décisions §12). Rédigée le 2026-10-17 à partir de l'inventaire ([inventaire/00-synthese.md](../inventaire/00-synthese.md)) et des décisions du §7 de la synthèse.
> Périmètre : un clone de Home Assistant en Rust + Svelte 5, dimensionné pour **cette** installation et non pour l'écosystème HA entier.

## 1. Principes directeurs

1. **Le périmètre vient de l'inventaire.**
   - Le moteur couvre les 27 automatismes vivants et les dormants que Hugo veut garder.
   - Le catalogue de cartes couvre les 4 dashboards vivants.
   - **Toutes les intégrations sont reprises** (décision du 2026-10-17) : le cœur en v1 (M1 à M7), le reste en M8 et M9. Voir [integrations.md](integrations.md).
2. **Un seul modèle de données pour l'UI et le YAML.** Un automatisme ou un dashboard est un document typé (Rust `serde` + JSON Schema). L'éditeur visuel et l'éditeur YAML sont deux vues du même document, sans état propre à l'une ou à l'autre.
3. **Compatibilité d'import, pas d'émulation.**
   - On importe les formats HA (automatismes, Lovelace, Alarmo, registres) dans le modèle du clone.
   - On ne reproduit ni l'API Python des intégrations ni HACS.
   - Les références par `device_id` ou UUID sont résolues à l'import et interdites ensuite.
4. **Dépendances externes conservées** : Mosquitto, Zigbee2MQTT, Frigate, Piper, ESPHome Builder, SWAG + Authelia. Le clone **se branche** sur ces services, il ne les remplace pas.
5. **Migration progressive et réversible.**
   - Le clone tourne à côté de HA.
   - Chaque automatisme, et chaque appareil qui parle directement au hub, n'est piloté **que par un seul système** à un instant donné.
   - L'alarme bascule en dernier.
6. **Sobriété** : un seul binaire, SQLite, pas de broker interne supplémentaire. Le système doit tourner confortablement sur un Raspberry Pi 4 ou dans un conteneur sur l'hôte Docker.

## 2. Vue d'ensemble

```
                    ┌──────────────── SWAG (TLS) + Authelia (OIDC) ────────────────┐
                    │                                                               │
             navigateur / NsPanel                                         ntfy (notifications)
                    │  HTTPS + WebSocket                                            ▲
┌───────────────────▼───────────────────────────────────────────────────────────────┼──┐
│ rusthome (binaire unique, tokio)                                                    │  │
│                                                                                   │  │
│  api (axum : REST + WS)  ──  auth (OIDC, rôles)  ──  frontend statique (SvelteKit)  │  │
│        │                                                                          │  │
│  ┌─────▼──────────────── core ─────────────────────────────────────────────┐      │  │
│  │ registres (étages, pièces, labels, catégories, appareils, entités)      │      │  │
│  │ state machine  ◄──►  bus d'événements (tokio::broadcast)                 │      │  │
│  │ service registry (domaine.service → handler)                             │      │  │
│  └──────┬───────────────┬─────────────────┬─────────────────┬───────────────┘      │  │
│   automation engine   alarm (centrale)   recorder (SQLite)   template (minijinja)   │  │
│         │                                                                          │  │
│  ┌──────▼──────────────────── intégrations (trait Integration) ─────────────────────┴┐ │
│  │ mqtt · z2m · esphome(API native) · frigate · hue · homekit · cast/spotify ·        │ │
│  │ onvif/camera · notify(ntfy, mobile) · alexa(sidecar) · portainer · ha_bridge ·     │ │
│  │ weather/idfm/calendar (v2) · awtrix                                               │ │
│  └────────────────────────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────────────────┘
     │MQTT           │Noise/TCP 6053      │HTTP          │HAP (IP)        │HA WebSocket
 Mosquitto ◄─ Z2M  ESP32 (lecteurs,    Frigate, Hue,   Netatmo          HA (miroir pendant
 Thingino, Awtrix   IR, e-ink…)        Portainer, Cast  thermostat       la migration)
```

## 3. Organisation du code

Le workspace Cargo vit à la racine du dépôt, à côté de `inventaire/` et `docs/`.

| Crate | Rôle | Dépendances clés |
|---|---|---|
| `rusthome-model` | Types du domaine : `EntityId`, `State`, `Device`, `Area`… ; documents automatisme, dashboard et alarme ; dérivation JSON Schema (`schemars`) | serde, schemars, chrono |
| `rusthome-core` | Registres, state machine, bus d'événements, service registry, contexte (`Context { id, user_id, parent_id }`) | tokio |
| `rusthome-template` | Environnement minijinja et extensions HA (§6.4) | minijinja |
| `rusthome-automation` | Moteur : triggers, conditions, actions, modes, traces | core, template |
| `rusthome-alarm` | Centrale d'alarme native (§7) | core |
| `rusthome-recorder` | Historique, statistiques, persistance de la configuration | sqlx (SQLite) |
| `rusthome-integrations/*` | Un crate par intégration (`mqtt`, `z2m`, `esphome`, `homekit`, …) | rumqttc, esphome-native-api, hap-controller, reqwest |
| `rusthome-import` | Importeurs HA : registres, automatismes, Lovelace, `alarmo.storage`, helpers | serde_yaml |
| `rusthome-api` | axum : REST, WebSocket, OIDC, permissions | axum, openidconnect |
| `rusthome` (bin) | Assemblage, configuration, démarrage | – |
| `frontend/` | SvelteKit (adapter-static) servi par le binaire | Svelte 5, TypeScript |

Les types TypeScript du frontend sont **générés** depuis les JSON Schema de `rusthome-model` (`json-schema-to-typescript`), pour garder une seule source de vérité.

## 4. Cœur

### 4.1 Modèle d'entités et d'états

- **Identifiants** : on conserve le format HA `domaine.objet` (`light.salon`) pour la familiarité et l'import.
  - Chaque entité possède aussi un `uid` interne stable, qui sert aux renommages.
  - Les documents (automatismes, dashboards) stockent l'`entity_id`. Un renommage les **réécrit** de façon transactionnelle, avec un aperçu des impacts : c'est ce que HA ne fait pas.
- **État** : `State { entity_id, state: String, attributes: Map, last_changed, last_updated, last_reported, context }`. On garde la sémantique HA (`unavailable`, `unknown`), car les templates importés en dépendent.
- **Distinction bruit / panne** (cf. 05-mort) :
  - les entités « sans état » (bouton, scène, événement) exposent `last_triggered` au lieu de `unknown` ;
  - `unavailable` porte un horodatage et une cause (`device_offline`, `integration_error`, `orphan`).
- **Domaines v1** : `sensor`, `binary_sensor`, `switch`, `light`, `button`, `event`, `select`, `number`, `climate`, `media_player`, `camera`, `siren`, `alarm_control_panel`, `update`, `device_tracker`, `person`, `zone`, `tag`, `input_boolean`, `input_text`, `input_datetime`, `counter`, `automation`, `scene` (proxy Hue), `remote`, `weather`, `calendar`, `notify`.

### 4.2 Registres

Ce sont les tables SQLite chargées en mémoire au démarrage. Chaque écriture passe par le core, qui émet un événement `registry_updated`.

| Registre | Champs principaux |
|---|---|
| Étage | id, nom, niveau, icône |
| Pièce | id, nom, étage, icône, alias |
| Label | id, nom, couleur, icône |
| Catégorie | id, portée (`automation`, `helper`…), nom |
| Appareil | id, intégration, identifiants externes (ieee Z2M, MAC ESPHome, id HAP…), constructeur, modèle, version firmware, pièce, labels, `disabled` |
| Entité | uid, entity_id, appareil, intégration, unique_id externe, nom, pièce (ou héritée de l'appareil), labels, icône, `hidden`, `disabled`, catégorie d'entité (config / diagnostic) |
| Badge (**nouveau**) | tag_id, personne, autorisé (oui / non), plage de validité, dernier passage. Remplace la triple liste blanche (cf. 02 §9.3) |
| Personne | id, nom, user OIDC lié, trackers |

**Cycle de vie des orphelins** : une entité dont l'intégration ne fournit plus d'`unique_id` passe `orphan` et apparaît dans une vue « à purger ». Cela concerne par exemple les conteneurs Docker identifiés par leur **nom**, et non plus par leur ID.

### 4.3 Bus d'événements et state machine

- Un `tokio::sync::broadcast` par flux : `state_changed`, `event` générique, `call_service`, `registry_updated`. Les abonnés lents sont détectés grâce au lag du broadcast, et journalisés.
- La state machine est une `DashMap<EntityId, Arc<State>>`. `set_state` n'émet un événement que s'il y a un changement d'état ou d'attribut, comme HA.
- Le **service registry** associe `domaine.service` à un handler async, avec un schéma de données validé. Les intégrations y enregistrent leurs services : `mqtt.publish`, `esphome.lecteur_alarme_badge_ok`, `alarm.arm`…
- Chaque appel porte un `Context`, qui sert aux traces, au journal (logbook) et aux permissions.

### 4.4 Persistance

**SQLite** (sqlx, mode WAL), avec un fichier de configuration et un fichier d'historique séparés :
- **config.db** : registres, documents (automatismes, dashboards, alarme, helpers), utilisateurs et rôles, paramètres des intégrations. Les secrets sont **chiffrés** avec une clé fournie par l'environnement.
- **history.db** :
  - `states` : rétention de 10 jours, avec exclusions par domaine et par intégration. Portainer et les réglages Thingino sont exclus par défaut.
  - `statistics` : agrégats horaires et journaliers (min, max, moyenne, somme) pour les entités `state_class`. Rétention illimitée, nécessaire pour les compteurs Linky et les graphes.
  - `logbook`.
- **Export et import YAML** de toute la configuration, pour les sauvegardes et le versionnage git.

## 5. Intégrations

### 5.1 Contrat

```rust
#[async_trait]
trait Integration: Send + Sync {
    fn domain(&self) -> &'static str;
    async fn setup(&self, ctx: IntegrationCtx, entry: ConfigEntry) -> Result<()>; // déclare appareils, entités, services
    async fn unload(&self) -> Result<()>;
    fn config_schema(&self) -> schemars::Schema;  // formulaire UI généré
}
```

- Chaque intégration tourne dans ses propres tâches tokio et parle au core via `IntegrationCtx` : `set_state`, `register_service`, `fire_event`, `registry`.
- Une panne d'intégration ne fait jamais tomber le cœur : l'intégration est supervisée, et redémarrée avec backoff.
- État de santé exposé : `loaded`, `retrying`, `error` et dernière erreur. Il alimente la vue « Santé », avec en plus les batteries, les orphelins et les capteurs d'alarme indisponibles.

### 5.2 Catalogue v1

| Intégration | Transport | Ce qu'elle fait | Notes |
|---|---|---|---|
| `mqtt` | rumqttc → Mosquitto | Client partagé, `mqtt.publish`, déclencheur `mqtt` | Utilisateur MQTT dédié `rusthome` |
| `z2m` | MQTT `zigbee2mqtt/*` | Entités depuis `bridge/devices` (exposes), commandes via `<nom>/set`, actions de boutons, disponibilité. **Gestion** : permit_join, renommage, suppression, OTA (check / update), interview ; groupes et bindings dans le modèle et l'UI, en v1.1 | Pas de discovery HA. Lecture directe des exposes Z2M. Activer `availability` dans Z2M |
| `esphome` | API native (Noise), crate `esphome-native-api` | Voir §5.3 | Rôle complet de « hub » |
| `frigate` | HTTP + MQTT `frigate/*` | Caméra, occupation, événements, `camera.record` (clip via l'API Frigate) | |
| `camera` / `onvif` | RTSP / ONVIF / snapshot HTTP | Flux live (relayé par go2rtc de Frigate) et snapshots | Thingino : on n'expose que flux + mouvement + personne ; les 117 réglages restent dans l'UI Thingino |
| `hue` | API v2 du pont (HTTPS + SSE) | Lumières, scènes du pont (activation), boutons Hue | Scènes = proxy, pas de moteur de scènes |
| `homekit` | HAP IP, crate `hap-controller` | Appairage du thermostat Netatmo, `climate` + capteurs, événements | Voir §5.4 |
| `cast` / `spotify` | mDNS + Cast ; API Web Spotify (Connect) | `media_player` Nest et TV, lecture de `alarm.mp3`, transfert Spotify | Remplace spotcast |
| `alexa` | **sidecar** HTTP local (Python, lib `alexapy`) | `notify.alexa_*` (annonce, TTS) | API non officielle : isolée dans un conteneur jetable. Le TTS passe par le skill Amazon « Simon Says », donc le « skill Simon » de CLAUDE.md n'a rien à recréer |
| `notify` | ntfy (HTTP) | `notify.hugo`, `notify.sokhna`, `notify.nspanel` : priorité, tags, pièce jointe (snapshot) | Remplace mobile_app et Pushbullet |
| `presence` | ntfy ou OwnTracks (HTTP) | `device_tracker` GPS pour `person` | Remplace la position de l'app compagnon : **à choisir** (§12) |
| `awtrix` | MQTT | Entités et applications custom | Les 5 automatismes « horloge » sont fusionnés en un module paramétré |
| `portainer` | API HTTPS | Statut par **nom** de conteneur, redémarrage | Pas d'historique |
| `ha_bridge` | WebSocket HA (jeton longue durée) | Miroir en lecture de toutes les entités HA, et appel de services HA | **Temporaire** : migration (§9) |
| M8 / M9 | – | **Toutes les autres intégrations** (décision « tout reprendre ») | Voir [integrations.md](integrations.md) : 68 intégrations, approche, effort, jalon |

### 5.3 ESPHome : le clone tient le rôle de HA

L'inventaire (08 §3) montre que les ESP32 **attendent le comportement complet d'un hub**. Le client ESPHome du clone implémente donc :

| Direction | Message ESPHome | Usage |
|---|---|---|
| hub → ESP | connexion Noise (clé `api_key_*` par nœud), `ListEntities`, `SubscribeStates` | Entités et états |
| hub → ESP | **`ExecuteService`** (actions utilisateur) | `badge_ok`, `badge_ko`, `reset_lecteur`, exposées en services `esphome.<nœud>_<action>` |
| ESP → hub | **`HomeassistantServiceCall`** | `alarmo.arm` / `alarmo.disarm` depuis les touches du lecteur. Une table d'alias les traduit vers `alarm.arm` / `alarm.disarm` |
| ESP → hub | **`tag_scanned`** (via service call `esphome.tag_scanned` / événement) | Alimente le registre de badges et les déclencheurs `tag` |
| hub → ESP | **`SubscribeHomeAssistantStates`** puis push d'états | Alarme, `input_text.lecteur_dernier_badge`, météo, bus : il faut **conserver les mêmes entity_id**, ou reflasher |
| ESP → hub (HTTP) | `GET /local/sounds/*.wav` | Le clone sert `/local/` en statique. La substitution `ha_url` du firmware pointe sur le clone au moment de la bascule |
| hub → ESP | commandes d'entités (light, switch…) | Standard |

**Contrainte de bascule** : un ESP accepte plusieurs clients API, mais ses `HomeassistantServiceCall` et `tag_scanned` partent vers **tous** les clients connectés. Le risque est un armement double. Un nœud donné est donc relié **soit à HA, soit au clone**, jamais aux deux. Pour en changer, on supprime le nœud de HA et on l'ajoute au clone.

### 5.4 HomeKit (Netatmo)

- Appairage HAP sur IP avec le code d'installation. Le thermostat accepte plusieurs contrôleurs, mais l'ajout d'un appairage passe par un contrôleur admin.
  - **Option A** : ajouter le clone comme contrôleur supplémentaire depuis HA ou depuis l'app Maison, et garder HA en parallèle.
  - **Option B** : désappairer HA, puis appairer le clone.
- Mapping des services HAP : `Thermostat` → `climate` (consigne, mode, température courante) ; capteurs → `sensor`.
- Les événements HAP (notifications de caractéristiques) mettent à jour l'état sans polling.
- **Risque** : maturité du crate `hap-controller`. Il faut un spike technique dès le jalon M2, avec un repli possible : garder le thermostat dans HA et le lire via `ha_bridge`.

- **Constat du 2026-10-17 (exploration M2, reportée à P4 par Hugo)** :
  - **Option A non faisable telle quelle.** Ajouter un contrôleur exige un contrôleur *administrateur*. Ici, c'est HA (appairé seul via le code), et ni HA ni l'app Maison ne proposent d'« ajouter un contrôleur tiers ».
  - **Crate `hap-controller` 3.3.0** : il couvre l'appairage par IP sans découverte (`Discovered::Ip` construit à la main), la lecture, l'écriture et l'abonnement, ainsi que `add_pairing`, `remove_pairing` et `list_pairings`.
  - **Thermostat** : « Netatmo Relay » à `192.168.1.167:5001`, identifiant d'appairage `B3:52:17:73:04:15`. Il n'est utilisé dans aucun automatisme HA, seulement dans des dashboards.
  - **Pas de découverte mDNS depuis WSL2** en mode NAT : il faut passer l'adresse en dur en dev, ou tester depuis l'hôte Docker .25.
  - **Plan P4 retenu** : retirer l'appairage dans HA, puis appairer RustHome avec le code à 8 chiffres. Impact : la carte thermostat des dashboards HA devient vide, mais le chauffage continue via Netatmo. Retour arrière : retirer RustHome, puis réappairer HA avec le même code.

## 6. Moteur d'automatismes

### 6.1 Modèle

Le document `Automation { id, alias, description, category, labels, mode, max, variables, triggers[], conditions[], actions[], enabled }` reprend le format HA moderne (`triggers` / `conditions` / `actions`, `trigger:` / `action:`). L'importeur accepte aussi les anciennes clés (`platform`, `service`, `trigger` / `condition` / `action` au singulier).

### 6.2 Périmètre v1

Il correspond à 02 §10.1, plus les dormants conservés.

- **Déclencheurs** : `state` (`from`, `to` en liste, `attribute`, `for`), `numeric_state` (front montant, `for`), `time`, `time_pattern`, `event`, `homeassistant` (start / shutdown), `tag`, `mqtt` (topic, payload, `value_template`). Les boutons Z2M et le cube sont **réécrits à l'import** en `mqtt` sur `zigbee2mqtt/<nom>/action`.
- **Conditions** : `state`, `numeric_state`, `template`, `trigger`, `time`, `and` / `or` / `not`.
- **Actions** : appel de service (cible, données templatées récursivement), `choose` / `default`, `if` / `then` / `else`, `delay`, `repeat` (`while`, `until`, `count`), `parallel`, `sequence`, condition inline, `variables`, `wait_template`, `wait_for_trigger`, `stop`, `event`, `continue_on_error`, `enabled`.
- **Modes** : `single`, `restart`, `queued`, `parallel` avec `max`.
- **Variables de contexte** : `trigger.*` (id, platform, entity_id, from_state, to_state, event, tag_id, payload…), `this`.

### 6.3 Exécution

- Chaque déclencheur est un abonné au bus, compilé à l'activation : un index par entity_id évite d'évaluer tous les automatismes à chaque `state_changed`.
- Un run est une tâche tokio, qui porte un `CancellationToken` pour `restart` et `stop`.
- Le désarmement ne tue pas les runs en cours (exigence de l'alarme, 02 §9.3 point 4).
- **Traces** : chaque run enregistre le déclencheur, les variables, le chemin de chaque étape (résultat, durée, erreur) et les changements d'état provoqués. On en garde 10 par automatisme, 50 pour la catégorie « Alarme ». L'UI les affiche sous forme de graphe.
- `last_triggered` est exposé sur l'entité `automation.*`.
- Les **erreurs** d'actions avec `continue_on_error` sont tracées et journalisées, sans interrompre la séquence.

### 6.4 Templates (minijinja)

Extensions à fournir, d'après l'inventaire Jinja (02 §4) :
- **Objet `states`** : `states.domaine.objet.state` et `.attributes`, objet appelable `states('x')`, itération par domaine.
- **Fonctions** : `is_state`, `state_attr`, `is_state_attr`, `has_value`, `now()` et `utcnow()` (Europe/Paris), `as_timestamp`, `as_datetime`, `today_at`, `timedelta`, `iif`, `relative_time`, `distance` (v2).
- **Types date et durée** : `strftime`, soustraction, `.total_seconds()`.
- **Filtres** : `int`, `float`, `round(n)`, `default(v, true)`, `upper`, `lower`, `trim`, `regex_*`, `from_json`, `to_json`, `min`, `max`.
- Méthode `.get()` sur les dictionnaires.
- **Rendu limité** : timeout, nombre max d'itérations, pas d'accès système.
- **Suivi des dépendances** : pour les déclencheurs `template` (v2), on enregistre les entités lues pendant le rendu.

Les templates sont validés à l'import : chaque template de l'inventaire est rendu en mode sec, et toute fonction inconnue fait échouer l'import.

### 6.5 Éditeurs (visuel et YAML)

- Une seule source : le document JSON. Le YAML est sérialisé et parsé côté serveur, qui fait foi pour la validation (JSON Schema + validation sémantique : entités existantes, services connus, templates compilables).
- **Éditeur visuel** : arbre de blocs (déclencheurs, conditions, actions imbriquées : `choose`, `if`, `repeat`, `parallel`). Chaque bloc a un formulaire généré depuis le schéma, avec des sélecteurs d'entités et de services filtrés.
- **Éditeur YAML** : CodeMirror 6, avec complétion des entity_id et services, et diagnostics serveur.
- **Bascule entre les deux sans perte.** Les commentaires YAML ne sont pas conservés : c'est assumé, et le champ `description` sert à documenter.

## 7. Centrale d'alarme native

Elle remplace Alarmo. Les exigences viennent de 07 et de 02 §9.

- **États** : `disarmed` → `arming` (sortie) → `armed_away` | `armed_home` → `pending` (entrée) → `triggered` (durée bornée) → retour `armed_*`, puis `disarmed`.
- **Configuration**, importée depuis `alarmo.storage` :
  - modes (sortie, entrée, déclenchement) ;
  - capteurs (type, modes, délais, `allow_open`, `always_on`, `trigger_unavailable`, `auto_bypass`).
- **Entité** `alarm_control_panel.alarme`. Pendant la transition, l'**alias `alarm_control_panel.alarmo`** est conservé, car les firmwares ESP s'y abonnent.
- **Services** : `alarm.arm` (mode, `skip_delay`), `alarm.disarm`, `alarm.trigger`. Les alias `alarmo.arm` / `alarmo.disarm` servent aux ESP et aux automatismes importés.
- **Événements** : `alarm_state_changed`, `alarm_arm_failed` (capteurs bloquants), `alarm_sensor_triggered`.
- **Identification** : par badge (registre §4.2) ou par personne (présence). Le code PIN reste optionnel.
- **Vue Santé** : capteurs d'alarme `unavailable`, batterie des sirènes, lecteurs hors ligne. L'armement est bloqué, avec un avertissement, si un capteur actif est indisponible (paramétrable).
- Les **réactions** (sirènes, clips, notifications) restent des **automatismes** de la catégorie « Alarme », pas de la logique codée en dur dans la centrale. C'est le même découpage qu'aujourd'hui, lisible et éditable.

## 8. API, authentification, rôles

### 8.1 API

- **REST** `/api/v1/...` : registres, documents (CRUD + validation), services (`POST /services/{domaine}/{service}`), historique, statistiques, logbook, traces, santé, import et export.
- **WebSocket** `/api/v1/ws` : abonnement aux états (filtres par entités ou domaines, diff incrémental), événements, appels de service. Protocole JSON inspiré de celui de HA, sans compatibilité visée.
- **OpenAPI** générée (`utoipa`).

### 8.2 Authentification

- **OIDC via Authelia** (code flow + PKCE), avec une session par cookie `HttpOnly`. Le clone ne stocke aucun mot de passe.
- **Comptes** : `admin`, `hugo`, `sokhna`, rattachés aux identités Authelia.
- **Comptes techniques** :
  - `nspanel` : jeton d'appareil à portée limitée, réservé à la tablette murale, avec un dashboard imposé ;
  - jetons d'API pour les scripts.
- **Pas d'équivalent de `trusted_networks` + `allow_bypass_login`.**

### 8.3 Rôles et permissions

| Rôle | Droits |
|---|---|
| `admin` | Tout : configuration, intégrations, utilisateurs, Zigbee, import |
| `resident` (hugo, sokhna) | Utiliser les dashboards, piloter les entités, armer et désarmer, voir l'historique ; éditer les automatismes et dashboards **non verrouillés** |
| `display` (nspanel) | Lecture et commandes sur une liste blanche d'entités ; pas de désarmement sans badge (paramétrable) |

- Les permissions sont évaluées sur `(rôle, action, ressource)`, avec des ressources filtrables par pièce, label ou domaine.
- Les documents de la catégorie « Alarme » sont **verrouillés** : seul `admin` peut les modifier.

## 9. Migration progressive

| Phase | Contenu | Côté HA | Critère de sortie |
|---|---|---|---|
| **P0 : miroir** | `ha_bridge` en lecture seule, import des registres (sans le mort, cf. 05 §7), dashboards Sécurité et Aperçu dans le clone | Inchangé | Le clone affiche les mêmes états que HA pendant une semaine |
| **P1 : sources natives + déménagement MQTT/Z2M** | Mosquitto et Z2M passent en Docker sur .25 (§11.1), puis `mqtt` + `z2m`, `frigate`, `hue`, `portainer`, `cast` en lecture directe | L'intégration MQTT de HA pointe vers le nouveau Mosquitto ; les add-ons Mosquitto et Z2M sont arrêtés | Parité des états ; les 17 appareils sont réappairés avec la nouvelle clé |
| **P2 : automatismes non critiques** | Import, puis reprise un par un : Awtrix, cube, interrupteurs, tireuse, Spotify, radiateur, congélateur. Désactivation dans HA **au même moment** | Automatisme désactivé | Traces OK sur 1 semaine |
| **P3 : notifications et présence** | ntfy, solution GPS, Alexa (sidecar) | mobile_app conservé en secours | Notifications reçues sur les deux téléphones |
| **P4 : HomeKit** | Appairage Netatmo (option A ou B, §5.4) | – | Contrôle du chauffage |
| **P5 : alarme** | Centrale native importée et **double fonctionnement en observation** : le clone calcule ses états sans agir. Puis bascule des ESP32 (lecteurs) et de la centrale, et reprise des automatismes « Alarme » | Alarmo, puis les lecteurs, retirés | 2 semaines sans écart, avec un test d'intrusion réel |
| **P6 : arrêt de HA** | **Après M9** : plus aucune entité ne passe par le bridge. Suppression de `ha_bridge`. Le Pi est libéré : il peut être réutilisé, par exemple en hôte de secours | HA éteint | – |

**Règle invariante** : à tout moment, une table « propriétaire » (automatisme ou appareil → `ha` | `rusthome`) est tenue dans le clone et affichée dans l'UI. L'import refuse d'activer un automatisme dont l'équivalent est actif dans HA, ce que l'on vérifie via le bridge.

## 10. Frontend (SvelteKit, Svelte 5)

- **Structure** :
  - `/` : dashboards ;
  - `/config/*` : pièces, étages, labels, appareils, entités, badges, automatismes, alarme, Zigbee, intégrations, utilisateurs ;
  - `/history`, `/logbook`, `/health`.
- **État temps réel** : un store rune (`$state`) alimenté par le WebSocket. Les cartes s'abonnent par entity_id, ce qui donne un rendu fin.
- **Dashboards** : un document `Dashboard { views[ { type: sections|masonry|panel, sections[ cards[] ] } ] }`. Il est édité en visuel (drag & drop, formulaires générés depuis le schéma de chaque carte) ou en YAML, sur le même principe que les automatismes.
- **Catalogue de cartes v1** (03 §5) :
  - P0 : `heading`, `tile`, `entities` (slider), `alarm-panel`, `gauge`, `grid` / stacks ;
  - P1 : `camera`, `logbook`, `thermostat`, `media-control`, `graph`, `weather`, `popup` (`#hash`).
  - L'**importeur Lovelace** fait correspondre les cartes Mushroom, Bubble et tempometer aux cartes natives ; une carte inconnue devient une carte « non supportée » visible.
- **Validateur de références** : une carte pointant vers une entité absente est signalée dans l'éditeur.
- **Profils d'affichage** : le mode kiosque par dashboard et par compte (NsPanel) masque l'en-tête et la navigation, et active le swipe entre vues.
- **Gestion Zigbee** :
  - liste des appareils (LQI, batterie, dernier contact, firmware) ;
  - appairage (`permit_join` avec compte à rebours et nouveaux appareils en direct), renommage (qui propage le nouveau nom dans les entity_id après confirmation), OTA, carte du réseau (v1.1) ;
  - groupes et bindings (v1.1).
- **Design** : composants maison (tokens CSS, thème clair et sombre). Aucune dépendance aux cartes HACS. **Identité** : logo maison + engrenage ([docs/brand/](brand/)), couleur principale `#1F5BC4` (contraste 3,4:1 sur noir, 6,3:1 sur blanc).

## 11. Déploiement et exploitation

- **Hôte** : un conteneur Docker sur 192.168.1.25, derrière SWAG + Authelia, dans la même stack Compose que **Mosquitto** et **Zigbee2MQTT** (D4). Les ressources sont plus larges que sur le Pi, et Frigate et Piper sont sur le même hôte.
- L'alarme dépend donc entièrement de l'hôte .25 : onduleur et redémarrage automatique (`restart: unless-stopped`) obligatoires. La résilience est à réexaminer avant P5.
### 11.1 Déménagement de Mosquitto et Zigbee2MQTT (phase P1)

1. **Mosquitto** en conteneur (`eclipse-mosquitto:2`) sur .25, avec un compte par client (`rusthome`, `z2m`, `frigate`, `thingino`, `awtrix`, `ha`) et des ACL par topic. Les mots de passe « compromis » actuels sont remplacés.
2. Repointer chaque client vers .25 : Frigate (déjà sur .25), caméra Thingino, Awtrix, intégration MQTT de HA. Pendant la transition, un **bridge Mosquitto** (ancien vers nouveau) peut éviter une coupure franche.
3. **Zigbee2MQTT** en conteneur (`koenkk/zigbee2mqtt`).
   - Le **ConBee II est déplacé physiquement en USB sur l'hôte .25**, et monté par son chemin `/dev/serial/by-id/...`.
   - Il faut vérifier le **placement radio** : position centrale, rallonge USB pour s'éloigner de l'USB 3 et du Wi-Fi.
   - Le maillage repose sur 7 routeurs secteur, ce qui devrait absorber le déplacement.
4. **Clé réseau** (D5) : nouvelle `network_key` et `pan_id` aléatoires, puis réappairage des 17 appareils. On reprend `devices.yaml` pour conserver les friendly names, donc les topics, donc les entités.
5. On active `availability` dans Z2M, pour la vue Santé.
6. Plan de retour arrière : l'ancien `zigbee2mqtt/` (sauvegarde coordinateur incluse) et le ConBee réinséré dans le Pi.

- **Image** : multi-arch (amd64 + arm64), binaire statique, frontend embarqué (`rust-embed`).
- **Configuration de démarrage** : `rusthome.toml` (chemins, URL OIDC, MQTT) et secrets en variables d'environnement. Le reste se configure dans l'UI ou par import YAML.
- **Sauvegarde** : snapshot SQLite à chaud (`VACUUM INTO`) quotidien, plus un export YAML versionnable (sans secrets).
- **Observabilité** : `tracing` (JSON) et métriques Prometheus (latence du bus, runs d'automatismes, santé des intégrations).
- **Tests** :
  - unitaires par crate ;
  - **corpus d'import** : les 44 automatismes, les dashboards et `alarmo.storage` de l'inventaire servent de fixtures, et l'import doit réussir sans perte ;
  - tests du moteur sur une horloge simulée (`tokio::time::pause`) ;
  - test de bout en bout de la chaîne alarme, avec des ESP simulés en serveur `esphome-native-api`.

## 12. Décisions (validées le 2026-10-17)

| # | Question | Décision |
|---|---|---|
| D1 | Hôte du clone | **Docker sur 192.168.1.25**. La question de la résilience de l'alarme sera réexaminée avant la phase P5 |
| D2 | Présence GPS | **OwnTracks** (HTTP vers le clone) |
| D3 | HomeKit | **Option B en P4** : retirer de HA, puis appairer RustHome. L'option A s'est révélée infaisable (cf. §5.4) |
| D4 | Mosquitto et Z2M | **Conteneurs Docker sur 192.168.1.25, comme le clone**. Le ConBee II doit être **déplacé en USB sur l'hôte .25** (voir §11.1) |
| D5 | Clé réseau Zigbee | **Changée en P1**, au moment du déplacement de Z2M, avec réappairage des 17 appareils |
| D6 | Alexa | **Sidecar `alexapy`**, avec ntfy et Piper/Cast en repli automatique |

## 13. Jalons

| Jalon | Contenu | Démo |
|---|---|---|
| **M1 : cœur** | model, core, recorder, template, API REST + WS, OIDC, `ha_bridge`, import des registres | Liste des entités en direct, depuis HA, dans une UI minimale |
| **M2 : intégrations natives** | mqtt, z2m (lecture et commande), frigate, hue, portainer ; spike HomeKit et ESPHome | Pilotage d'une prise Zigbee depuis le clone |
| **M3 : moteur** | Automatismes v1, traces, import des 44 automatismes, éditeur YAML | Les automatismes Awtrix tournent dans le clone |
| **M4 : dashboards** | Cartes P0 + P1, import Lovelace, éditeur visuel de dashboards, profil NsPanel | Dashboard Sécurité dans le clone |
| **M5 : éditeur visuel d'automatismes, gestion Zigbee** | Éditeur à blocs, appairage, renommage, OTA | – |
| **M6 : ESPHome, HomeKit, notifications** | Client API native complet, HAP, ntfy, Alexa, présence | Lecteur v1 piloté par le clone (banc de test) |
| **M7 : alarme** | Centrale native, observation, puis bascule | Phase P5 |
| **M8 : intégrations étendues** | Médias, salon, infos, réseau, NAS (25 intégrations, cf. [integrations.md](integrations.md)) | Dashboards Aperçu et Monitoring entièrement natifs |
| **M9 : cloud non officiel, BLE, divers** | PSN, Tile, Waze, proxies Bluetooth (Oral-B, iBeacon), Epson, Yamaha, éclairage adaptatif | Arrêt de HA possible sans perte (phase P6) |
