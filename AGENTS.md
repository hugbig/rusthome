# AGENTS.md : règles de production du code de RustHome

> Ce fichier s'adresse à tout agent (ou humain) qui écrit du code dans ce dépôt. Il complète [CLAUDE.md](CLAUDE.md), qui porte le contexte projet, et la spec [docs/architecture.md](docs/architecture.md).
> En cas de conflit : **sécurité de la maison > ce fichier > habitudes de l'agent**.

## 1. Le projet en une phrase

**RustHome** (nom retenu le 2026-10-17 ; identifiant technique `rusthome`) est un clone de Home Assistant en Rust (backend) et Svelte 5 / SvelteKit (frontend), dimensionné pour l'installation de Hugo. Le périmètre est fixé par [inventaire/00-synthese.md](inventaire/00-synthese.md), [docs/architecture.md](docs/architecture.md) et [docs/integrations.md](docs/integrations.md).

- Le nom « Home Assistant » n'apparaît **pas** dans le produit : ni dans l'UI, ni dans les noms de crates ou de binaires, ni dans les logos. On ne le cite que pour parler de l'existant (import, `ha_bridge`).
- Préfixe technique : crates `rusthome-*`, binaire `rusthome`, variables d'environnement `RUSTHOME_*`. Ne pas disperser d’autres variantes (pas de `rust-home`, `rust_home`, `RustHA`).

## 2. Règles d'or (non négociables)

1. **Home Assistant en lecture seule.** Pendant tous les jalons de développement (M1 à M9) :
   - aucune modification de HA, ni par le MCP, ni par SSH, ni par l'API ;
   - le `ha_bridge` n'utilise que des appels de lecture et l'abonnement aux événements. Tout appel de service vers HA est **interdit dans le code** tant que la phase de migration correspondante n'a pas été validée par Hugo.
2. **Jalons (M) ≠ phases de migration (P).**
   - Un jalon M produit du code.
   - Une phase P bascule quelque chose dans la maison, et **seulement avec l'accord explicite de Hugo**.
   - Un automatisme importé est **désactivé par défaut**.
3. **Jamais deux pilotes pour un même appareil ou automatisme.**
   - Les ESP32 de l'alarme (`lecteur-alarme`, `tag-reader`) ne sont **jamais** connectés au clone pendant le développement. Les tests ESPHome se font sur banc (`test-nfc`, ESP de rechange) ou contre un ESP simulé.
4. **L'alarme d'abord en sûreté.** Tout code qui touche `rusthome-alarm`, les automatismes de la catégorie « Alarme » ou les intégrations sirène, badge et caméra demande :
   - des tests couvrant chaque transition d'état ;
   - une mention explicite dans le compte rendu.
5. **Aucun secret dans le dépôt.**
   - Les secrets vont dans `.env` (ignoré par git) ou dans le magasin chiffré du clone.
   - Les fixtures issues de l'inventaire sont **expurgées** (`<REDACTED>`).
   - Ne jamais copier `secrets.yaml`, les jetons ni les fichiers `.storage/auth*`.

## 3. Qualité : tests et lint

### 3.1 Rust

- **Tests unitaires obligatoires.**
  - Toute fonction publique non triviale, toute règle métier et tout parseur ou importeur arrive avec ses tests, dans le même changement.
  - Les tests vivent dans `#[cfg(test)] mod tests` à côté du code. Les tests d'intégration d'un crate vont dans `crates/<crate>/tests/`.
  - **Corpus d'import** : les exports de l'inventaire servent de fixtures, mais `inventaire/` **n’est pas versionné** (données de la maison). On en copie un extrait **expurgé** (sans secret, sans adresse inutile) dans `crates/<crate>/tests/fixtures/`, et aucun code ne lit `inventaire/`. Les 44 automatismes doivent s'importer sans perte : un test le vérifie.
  - **Temps** : pas de `sleep` réel. Utiliser `tokio::time::pause()` / `advance()` et une horloge injectable (`trait Clock`).
  - **Réseau** : pas d'appel réseau réel dans les tests. Utiliser des doubles : Mosquitto embarqué (`rumqttd`) ou des mocks, `wiremock` pour HTTP, un serveur `esphome-native-api` simulé.
  - Tests de propriétés (`proptest`) pour les parseurs YAML et Jinja et pour le round-trip UI ↔ YAML.
- **Lint et format** : un changement n'est terminé que si ces commandes passent sans erreur ni avertissement.
  ```bash
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets --all-features -- -D warnings
  cargo test --workspace
  ```
  - Lints du workspace (`[workspace.lints]` dans `Cargo.toml`) : `clippy::all` et `clippy::pedantic` en *warn* ; `unsafe_code = "forbid"` ; `clippy::unwrap_used` et `clippy::expect_used` interdits hors tests.
  - Un `#[allow(...)]` doit être **local et justifié** par un commentaire.
- **Erreurs** : `thiserror` dans les bibliothèques, `anyhow` uniquement dans le binaire et les tests. Pas de `panic!` sur une donnée externe (MQTT, HTTP, YAML).
- **Couverture** : `cargo llvm-cov` est disponible. Objectif ≥ 80 % sur `rusthome-model`, `rusthome-core`, `rusthome-automation`, `rusthome-alarm` et `rusthome-template`.

### 3.2 Frontend (SvelteKit, Svelte 5)

- TypeScript strict. **Runes** Svelte 5 (`$state`, `$derived`, `$effect`), pas l'ancienne syntaxe des stores sauf nécessité.
- Commandes à faire passer :
  ```bash
  pnpm -C frontend lint          # eslint + prettier --check
  pnpm -C frontend check         # svelte-check
  pnpm -C frontend test          # vitest (unitaires, composants via @testing-library/svelte)
  ```
- Les types d'API sont **générés** depuis les JSON Schema Rust (`just gen-types`). Ne jamais les éditer à la main.

### 3.3 Commande unique

`just check` enchaîne fmt, clippy, tests Rust, lint, check et tests frontend. C'est **la** commande à lancer avant de déclarer un travail terminé. La CI exécutera la même.

## 4. Lancer le projet dans WSL (environnement de Hugo)

Cible : Ubuntu 24.04 sous WSL2, x86_64. Docker est déjà installé.

### 4.1 Prérequis (une fois)

```bash
# Rust stable (installé dans ~/.cargo, sans sudo) + composants
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal -c clippy -c rustfmt
cargo install just --locked
# Node : SvelteKit 3 exige Node ≥ 22.17 → Node 24 via nvm (version épinglée dans .nvmrc ;
# la version par défaut de nvm n'est pas modifiée)
nvm install 24 && nvm use        # à refaire dans chaque nouveau terminal du projet
npm i -g pnpm@10                 # corepack 0.31 ne sait pas lancer pnpm 12 : ne pas l'utiliser
pnpm -C frontend install
```

Déjà présents dans le WSL de Hugo : `build-essential`, `pkg-config`, `libssl-dev`, Docker, git.

### 4.2 Lancement en développement

```bash
nvm use                         # Node 24 (cf. .nvmrc)
cp .env.example .env            # par défaut : données de démo ; pour le miroir HA, renseigner HA_URL / HA_TOKEN
just dev-deps                   # docker compose -f deploy/dev/compose.yaml up -d : Mosquitto de dev (port 1884), ntfy de dev
just dev                        # backend (cargo watch) + frontend (vite) en parallèle
```

- UI : http://localhost:5173 (Vite, proxifié vers l'API). API : http://localhost:8124.
- Données de dev dans `./var/` (SQLite), ignoré par git. `just reset-dev` efface tout.
- **Auth en dev** : le mode `RUSTHOME_AUTH=dev` crée un utilisateur local `hugo` (admin) sans OIDC. Il est **refusé au démarrage** si `RUSTHOME_ENV=production`.
- **Données réelles** : `ha_bridge` lit le HA de production (192.168.1.35) en lecture seule. Sans `HA_TOKEN`, le clone démarre avec les données de démo `deploy/dev/fixtures.json` (`RUSTHOME_FIXTURES`).
- Le Mosquitto de dev est **isolé** (port 1884, jamais le broker de production) : aucune commande n'atteint un appareil réel par accident.

### 4.3 Commandes `just` attendues

| Commande | Effet |
|---|---|
| `just dev` | Backend + frontend (Vite en rechargement à chaud ; backend relancé à la main) |
| `just dev-deps` / `just dev-down` | Démarre ou arrête les dépendances Docker de dev |
| `just check` | Tous les lints et tous les tests (§3.3) |
| `just test` | Tests Rust + frontend |
| `just gen-types` | Régénère `frontend/src/lib/api-types.ts` depuis les schémas Rust (`just check` échoue s'ils divergent) |
| `just import-fixtures` | *(à venir, M3)* Importe le corpus d’automatismes de l’inventaire dans la base de dev |
| `just build` | Binaire release + frontend embarqué |
| `just docker` / `just docker-multiarch` | Image Docker locale / multi-architecture (amd64 + arm64) |

Le `justfile` est créé dès le jalon M1 et tenu à jour avec ce tableau.

## 5. Structure et conventions

- **Arborescence** : `crates/rusthome-*` (workspace Cargo), `frontend/`, `deploy/` (Compose dev et prod), `docs/`, `inventaire/` (local uniquement, ignoré par git : il décrit la maison et ses faiblesses).
- **Langue** :
  - code, identifiants et commentaires techniques en **anglais** ;
  - documentation (`docs/`), UI (i18n, `fr` par défaut) et messages de commit en **français**.
- **Commentaires** : expliquer le *pourquoi*, pas le *quoi*. Toute règle issue de l'inventaire cite sa source, par exemple `// cf. inventaire/02 §9.3 : le désarmement ne tue pas les runs`.
- **Dépendances** : préférer les crates maintenues et répandues (tokio, axum, sqlx, serde, minijinja, rumqttc, tracing). Toute nouvelle dépendance « exotique » (`hap-controller`, `esphome-native-api`…) est justifiée dans la PR.
- **Spec vivante** : un écart avec [docs/architecture.md](docs/architecture.md) se **propose** comme une modification de la spec, jamais en silence.

## 6. Git

- Branches `feat/…`, `fix/…`, `chore/…` ; jamais de commit direct sur `main` une fois le dépôt initialisé.
- Messages de commit en français, à l'impératif, préfixés (`feat(core): …`, `fix(alarm): …`, `test(import): …`).
- Un commit ne casse pas `just check`.
- **Ne pas commiter ni pousser sans demande de Hugo.**

## 7. Définition de « terminé »

Un changement est terminé quand :
1. `just check` passe (fmt, clippy `-D warnings`, tests Rust et frontend, svelte-check) ;
2. le comportement nouveau est couvert par des tests ;
3. il est lançable en local (`just dev`) et a été essayé, ou le compte rendu dit clairement ce qui n'a pas pu l'être ;
4. la doc est à jour (spec, `docs/integrations.md`, tableau `just` si besoin) ;
5. aucune règle du §2 n'a été enfreinte.
