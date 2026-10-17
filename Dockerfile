# syntax=docker/dockerfile:1
# Image de production RustHome : binaire Rust + interface SvelteKit embarquée.
# Construction : `just docker` (cf. AGENTS.md §4.3).

# --- 1. Interface ---
FROM node:24-trixie-slim AS web
WORKDIR /src/frontend
RUN npm install -g pnpm@10
COPY frontend/package.json frontend/pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile
COPY frontend/ ./
RUN pnpm build

# --- 2. Backend ---
FROM rust:1-trixie AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock clippy.toml ./
COPY crates crates
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p rusthome \
    && cp target/release/rusthome /rusthome

# --- 3. Exécution ---
FROM debian:trixie-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates tzdata \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home /data rusthome \
    && mkdir -p /data && chown rusthome /data
COPY --from=build /rusthome /usr/local/bin/rusthome
COPY --from=web /src/frontend/build /app/web
ENV RUSTHOME_ENV=production \
    RUSTHOME_BIND=0.0.0.0:8124 \
    RUSTHOME_STATIC_DIR=/app/web \
    RUSTHOME_DB=sqlite:///data/rusthome.db \
    RUST_LOG=info,sqlx=warn
USER rusthome
VOLUME /data
EXPOSE 8124
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s CMD ["rusthome", "healthcheck"]
ENTRYPOINT ["rusthome"]
