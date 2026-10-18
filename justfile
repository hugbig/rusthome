# RustHome : commandes de développement (cf. AGENTS.md §4.3)
set dotenv-load := true

# Deux compilations Rust en parallèle au plus : WSL (10 Go) est partagé avec
# VS Code ; au-delà, la mémoire sature. Surchargeable : CARGO_BUILD_JOBS=4 just check
export CARGO_BUILD_JOBS := env_var_or_default("CARGO_BUILD_JOBS", "2")

default:
    @just --list

# Pas de recette shebang : just les écrit dans $XDG_RUNTIME_DIR, absent sous
# WSL sans systemd. Sans watchexec (`cargo install watchexec-cli --locked`),
# le backend est lancé une seule fois.
# Backend + frontend relancés à chaque modification (Vite à chaud, backend via watchexec)
dev:
    bash -c 'trap "kill 0" EXIT; (cd frontend && pnpm dev) & just backend-watch'

# Backend relancé à chaque modification du code Rust ou des migrations
backend-watch:
    bash -c 'if command -v watchexec >/dev/null; then exec watchexec --restart --watch crates --watch Cargo.toml --exts rs,toml,sql -- cargo run -p rusthome; else echo "watchexec absent : backend sans relance automatique (cargo install watchexec-cli --locked)"; exec cargo run -p rusthome; fi'

# Backend seul
backend:
    cargo run -p rusthome

# Dépendances Docker de dev (Mosquitto :1884, ntfy :8090)
dev-deps:
    docker compose -f deploy/dev/compose.yaml up -d

dev-down:
    docker compose -f deploy/dev/compose.yaml down

# Efface la base de dev
reset-dev:
    rm -rf var

# Types TypeScript générés depuis les JSON Schema Rust (ne jamais les éditer à la main)
gen-types:
    cargo run -q -p rusthome-model --example schemas > frontend/src/lib/api-schema.json
    pnpm -C frontend exec json2ts --unreachableDefinitions --additionalProperties false --bannerComment "/* Généré par 'just gen-types' depuis crates/rusthome-model : ne pas éditer. */" -i src/lib/api-schema.json -o src/lib/api-types.ts
    pnpm -C frontend exec prettier --write src/lib/api-types.ts >/dev/null
    rm frontend/src/lib/api-schema.json

# Échoue si les types générés ne correspondent plus au modèle Rust
check-types:
    cp frontend/src/lib/api-types.ts /tmp/rusthome-api-types.ts.bak
    just gen-types
    diff -q /tmp/rusthome-api-types.ts.bak frontend/src/lib/api-types.ts || (echo "api-types.ts obsolète : lancer 'just gen-types'" && exit 1)

# Tous les lints et tous les tests : à faire passer avant de déclarer « terminé »
check: check-rust check-types check-front

check-rust:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    cargo test --workspace

check-front:
    pnpm -C frontend lint
    pnpm -C frontend check
    pnpm -C frontend test

test:
    cargo test --workspace
    pnpm -C frontend test

fmt:
    cargo fmt --all
    pnpm -C frontend format

# Binaire release + frontend construit
build:
    pnpm -C frontend build
    cargo build --release -p rusthome

# Image Docker locale (architecture de la machine)
docker:
    docker build -t rusthome:dev .

# Image multi-architecture (amd64 + arm64) ; arm64 passe par l'émulation QEMU, c'est long
docker-multiarch tag="rusthome:latest":
    docker buildx build --platform linux/amd64,linux/arm64 -t {{tag}} .
