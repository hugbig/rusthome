//! `RustHome` entry point.

mod config;

use std::sync::Arc;

use anyhow::Context as _;
use config::{AuthConfig, Config, Env};
use rusthome_api::oidc::{OidcAuth, OpenIdProvider};
use rusthome_api::{ApiState, AuthMode, router};
use rusthome_core::{Hub, SystemClock};
use rusthome_ha_bridge::BridgeConfig;
use rusthome_import::{HaSnapshot, import_registries};
use rusthome_recorder::Recorder;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    let cfg = Config::from_vars(&std::env::vars().collect()).context("invalid configuration")?;
    // `rusthome healthcheck`: container probe (the runtime image has no curl).
    if std::env::args().nth(1).as_deref() == Some("healthcheck") {
        return healthcheck(cfg.bind).await;
    }
    init_tracing(cfg.env);
    tracing::info!(env = ?cfg.env, bind = %cfg.bind, tz = %cfg.tz, "starting RustHome {}", env!("CARGO_PKG_VERSION"));

    if let Some(dir) = cfg
        .database_url
        .strip_prefix("sqlite://")
        .and_then(|p| std::path::Path::new(p).parent())
    {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let recorder = Recorder::open(&cfg.database_url)
        .await
        .context("opening database")?;
    let hub = Hub::new(Arc::new(SystemClock));
    let loaded = recorder.load_registries(&hub.registries).await?;
    tracing::info!(loaded, "registries loaded");
    tokio::spawn(recorder.clone().run(hub.clone()));
    spawn_purge(recorder.clone(), hub.clone(), cfg.retention_days);

    // Ready signal of the HA mirror: native integrations wait for its first snapshot.
    let mut mirror_ready: Option<tokio::sync::watch::Receiver<bool>> = None;
    match (&cfg.ha, &cfg.fixtures) {
        (Some((url, token)), _) => {
            tracing::info!(%url, "mirroring Home Assistant (read-only)");
            let (_bridge, ready) = rusthome_ha_bridge::spawn(
                &hub,
                BridgeConfig {
                    url: url.clone(),
                    token: token.clone(),
                },
            );
            mirror_ready = Some(ready);
        }
        (None, Some(path)) => {
            let raw = std::fs::read_to_string(path)
                .with_context(|| format!("reading {}", path.display()))?;
            let snapshot: HaSnapshot = serde_json::from_str(&raw).context("parsing fixtures")?;
            let report =
                rusthome_ha_bridge::apply(&hub, &import_registries(&snapshot), snapshot.states);
            tracing::info!(?report, "fixtures loaded");
        }
        (None, None) => tracing::warn!("no HA_URL/HA_TOKEN nor RUSTHOME_FIXTURES: starting empty"),
    }

    start_integrations(&cfg, &hub, mirror_ready.as_ref())?;

    let auth = match &cfg.auth {
        AuthConfig::Dev => AuthMode::Dev,
        AuthConfig::Disabled => AuthMode::Disabled,
        AuthConfig::Oidc(o) => {
            let provider = OpenIdProvider::discover(
                &o.issuer,
                &o.client_id,
                &o.client_secret,
                &o.redirect_url(),
            )
            .await
            .context("OIDC discovery (is RUSTHOME_OIDC_ISSUER reachable?)")?;
            tracing::info!(issuer = %o.issuer, redirect = %o.redirect_url(), "OIDC login enabled");
            let secure = o.public_url.scheme() == "https";
            AuthMode::Oidc(Arc::new(OidcAuth::new(
                Arc::new(provider),
                o.roles.clone(),
                secure,
            )))
        }
    };

    let app = router(
        ApiState {
            hub,
            recorder: Some(recorder),
            auth,
        },
        cfg.static_dir.clone(),
    );
    let listener = tokio::net::TcpListener::bind(cfg.bind)
        .await
        .with_context(|| format!("binding {}", cfg.bind))?;
    tracing::info!("API on http://{}/api/v1", cfg.bind);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
}

/// Native integrations (M2). Each waits for the first HA snapshot before
/// taking over HA entities.
fn start_integrations(
    cfg: &Config,
    hub: &Hub,
    mirror_ready: Option<&tokio::sync::watch::Receiver<bool>>,
) -> anyhow::Result<()> {
    let hub = hub.clone();
    if let Some(m) = &cfg.mqtt {
        start_mqtt(&hub, m, mirror_ready.cloned())?;
    }
    if let Some(h) = &cfg.hue {
        let hue = rusthome_hue::Hue::new(
            hub.clone(),
            rusthome_hue::HueConfig {
                url: h.url.clone(),
                key: h.key.clone(),
                read_only: h.read_only,
            },
        )
        .context("Hue client")?;
        hue.register_services();
        if h.read_only {
            tracing::warn!(url = %h.url, "Hue in READ-ONLY mode: light commands are refused");
        }
        let ready = mirror_ready.cloned();
        tokio::spawn(async move {
            wait_for_mirror(ready).await;
            hue.run().await;
        });
    }
    if let Some(p) = &cfg.portainer {
        let (hub, mirror_ready) = (hub.clone(), mirror_ready.cloned());
        let pc = rusthome_portainer::PortainerConfig {
            url: p.url.clone(),
            token: p.token.clone(),
            accept_invalid_certs: p.accept_invalid_certs,
            poll_every: std::time::Duration::from_secs(30),
        };
        tracing::info!(url = %p.url, "Portainer monitoring enabled (read-only)");
        tokio::spawn(async move {
            wait_for_mirror(mirror_ready).await;
            rusthome_portainer::run(hub, pc).await;
        });
    }
    Ok(())
}

/// MQTT client + Zigbee2MQTT integration (M2).
fn start_mqtt(
    hub: &Hub,
    m: &config::MqttSettings,
    mirror_ready: Option<tokio::sync::watch::Receiver<bool>>,
) -> anyhow::Result<()> {
    let (mqtt, eventloop) = rusthome_mqtt::Mqtt::new(&rusthome_mqtt::MqttConfig {
        url: m.url.clone(),
        username: m.username.clone(),
        password: m.password.clone(),
        read_only: m.read_only,
        client_id: format!("rusthome-{}", std::process::id()),
    })
    .context("MQTT configuration")?;
    if m.read_only {
        tracing::warn!(url = %m.url, "MQTT in READ-ONLY mode: commands to devices are refused");
    }
    tokio::spawn(rusthome_mqtt::run(mqtt.clone(), eventloop));
    let z2m = rusthome_z2m::Z2m::new(hub.clone(), &m.z2m_base_topic, Arc::new(mqtt.clone()));
    z2m.register_services();
    if let Some(url) = &m.frigate_url {
        let frigate = rusthome_frigate::Frigate::new(
            hub.clone(),
            url.clone(),
            &m.frigate_prefix,
            Arc::new(mqtt.clone()),
        )
        .context("Frigate client")?;
        frigate.register_services();
        tracing::info!(%url, "Frigate enabled");
        let (mqtt, ready) = (mqtt.clone(), mirror_ready.clone());
        tokio::spawn(async move {
            wait_for_mirror(ready).await;
            frigate.run(mqtt).await;
        });
    }
    tokio::spawn(async move {
        wait_for_mirror(mirror_ready).await;
        z2m.run(mqtt).await;
    });
    Ok(())
}

/// Native integrations wait for the first HA snapshot to take over the HA
/// entity ids; bounded so that they still start without HA.
async fn wait_for_mirror(ready: Option<tokio::sync::watch::Receiver<bool>>) {
    if let Some(mut ready) = ready {
        let wait = ready.wait_for(|r| *r);
        if tokio::time::timeout(std::time::Duration::from_secs(60), wait)
            .await
            .is_err()
        {
            tracing::warn!("HA mirror not ready after 60 s: starting native integrations anyway");
        }
    }
}

fn init_tracing(env: Env) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn,tower_http=info"));
    let builder = tracing_subscriber::fmt().with_env_filter(filter);
    if env == Env::Production {
        builder.json().init();
    } else {
        builder.init();
    }
}

fn spawn_purge(recorder: Recorder, hub: Hub, days: i64) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            tick.tick().await;
            match recorder
                .purge(hub.clock.now(), chrono::Duration::days(days))
                .await
            {
                Ok(n) if n > 0 => tracing::info!(removed = n, "history purged"),
                Ok(_) => {}
                Err(e) => tracing::error!(error = %e, "history purge failed"),
            }
        }
    });
}

/// Plain HTTP/1.0 GET on `/api/v1/health`; exit code 0 only on `200`.
async fn healthcheck(bind: std::net::SocketAddr) -> anyhow::Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    // A 0.0.0.0 bind is reached through loopback.
    let addr = if bind.ip().is_unspecified() {
        std::net::SocketAddr::from(([127, 0, 0, 1], bind.port()))
    } else {
        bind
    };
    let mut stream = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        tokio::net::TcpStream::connect(addr),
    )
    .await
    .context("healthcheck timeout")??;
    stream
        .write_all(b"GET /api/v1/health HTTP/1.0\r\nHost: localhost\r\n\r\n")
        .await?;
    let mut head = [0u8; 12];
    stream.read_exact(&mut head).await?;
    anyhow::ensure!(
        head.ends_with(b"200"),
        "unhealthy: {}",
        String::from_utf8_lossy(&head)
    );
    Ok(())
}

async fn shutdown() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutting down");
}
