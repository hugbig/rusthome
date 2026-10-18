//! ESPHome projects (spec §5.3 bis): list, YAML editing, creation,
//! deletion, validation, compilation, over-the-air installation, logs and
//! firmware downloads for USB flashing from the browser. Everything goes
//! through the ESPHome Builder container. Admins only.
//!
//! The policy is applied here, before the builder is asked anything:
//! locked projects (the alarm nodes) are read-only and never reach a
//! device; only flashable projects are installed over the air.

use std::sync::Arc;

use axum::Json;
use axum::body::Body;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State as AxState};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use rusthome_esphome::policy::{valid_configuration, valid_secret_key, yaml_device_name};
use rusthome_esphome::{Builder, BuilderError, Frame, Policy};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{ApiError, ApiState, Role, User};

pub struct EsphomeCtx {
    pub builder: Builder,
    pub policy: Policy,
}

impl From<BuilderError> for ApiError {
    fn from(e: BuilderError) -> Self {
        match e.code() {
            Some("not_found") => ApiError::NotFound(e.to_string()),
            Some("invalid_args" | "invalid_message") => ApiError::BadRequest(e.to_string()),
            Some("already_exists" | "precondition_failed") => ApiError::Conflict(e.to_string()),
            _ => ApiError::Internal(e.to_string()),
        }
    }
}

fn ctx<'a>(s: &'a ApiState, user: &User) -> Result<&'a EsphomeCtx, ApiError> {
    if user.role != Role::Admin {
        return Err(ApiError::Forbidden(
            "only admins manage ESPHome projects".into(),
        ));
    }
    s.esphome
        .as_deref()
        .ok_or_else(|| ApiError::NotFound("ESPHome Builder is not configured".into()))
}

fn check_name(configuration: &str) -> Result<(), ApiError> {
    if valid_configuration(configuration) {
        Ok(())
    } else {
        Err(ApiError::BadRequest(format!(
            "invalid project file {configuration}"
        )))
    }
}

/// The builder's `esphome.name` of a project, `None` when the project does
/// not exist yet. Fails closed: if the builder cannot list its projects, the
/// lock cannot be checked and the caller refuses (code review n° 4).
async fn builder_name(c: &EsphomeCtx, configuration: &str) -> Result<Option<String>, ApiError> {
    let list = c
        .builder
        .call("devices/list", json!({}))
        .await
        .map_err(|e| {
            ApiError::Conflict(format!("cannot check the lock of {configuration}: {e}"))
        })?;
    Ok(list["configured"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|d| d["configuration"] == configuration)
        .and_then(|d| d["name"].as_str())
        .map(str::to_owned))
}

fn locked_error(configuration: &str) -> ApiError {
    ApiError::Conflict(format!(
        "{configuration} is locked (alarm node): read-only until the alarm migration phase"
    ))
}

/// Refuses any change to a locked project, known by its file name, by its
/// current `esphome.name`, or by the `esphome.name` of the new content (a
/// copy of an alarm node under another file name is locked too).
async fn writable(
    c: &EsphomeCtx,
    configuration: &str,
    content: Option<&str>,
) -> Result<(), ApiError> {
    check_name(configuration)?;
    if c.policy.is_locked(configuration, None) {
        return Err(locked_error(configuration));
    }
    let current = builder_name(c, configuration).await?;
    if c.policy.is_locked(configuration, current.as_deref()) {
        return Err(locked_error(configuration));
    }
    if let Some(text) = content {
        let Some(name) = yaml_device_name(text) else {
            return Err(ApiError::BadRequest(
                "esphome.name must be readable (plain value or substitution)".into(),
            ));
        };
        if c.policy.is_locked(configuration, Some(&name)) {
            return Err(locked_error(configuration));
        }
    }
    Ok(())
}

/// What the projects page needs from a builder device, plus the policy.
#[must_use]
pub(crate) fn summary(d: &Value, state: Option<&str>, policy: &Policy) -> Value {
    let configuration = d["configuration"].as_str().unwrap_or_default();
    let name = d["name"].as_str();
    let rt = &d["runtime_state"];
    json!({
        "configuration": configuration,
        "name": name,
        "friendly_name": d["friendly_name"],
        "comment": d["comment"],
        "platform": d["target_platform"],
        "board": d["board_id"],
        "address": d["address"],
        "ip": d["ip"],
        "state": state.or_else(|| rt["state"].as_str()).unwrap_or("unknown"),
        "built_version": d["current_version"],
        "deployed_version": rt["deployed_version"],
        "update_available": d["update_available"] == true,
        "pending_changes": d["has_pending_changes"] == true,
        "locked": policy.is_locked(configuration, name),
        "flashable": policy.can_flash(configuration, name),
        "target": policy.address(configuration, name).map(|ip| ip.to_string()),
    })
}

pub(crate) async fn overview(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
) -> Result<Json<Value>, ApiError> {
    let c = ctx(&s, &user)?;
    let version = c.builder.call("config/version", json!({})).await?;
    let list = c.builder.call("devices/list", json!({})).await?;
    let states = c
        .builder
        .call("devices/get_states", json!({}))
        .await
        .unwrap_or(Value::Null);
    let devices: Vec<Value> = list["configured"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|d| {
            let conf = d["configuration"].as_str().unwrap_or_default();
            summary(d, states[conf].as_str(), &c.policy)
        })
        .collect();
    Ok(Json(json!({
        "version": version["esphome_version"],
        "devices": devices,
    })))
}

pub(crate) async fn get_config(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
    Path(configuration): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let c = ctx(&s, &user)?;
    check_name(&configuration)?;
    let content = c
        .builder
        .call(
            "devices/get_config",
            json!({ "configuration": configuration }),
        )
        .await?;
    // Read-only view: an unreadable builder list counts as locked.
    let current = builder_name(c, &configuration).await;
    let from_yaml = content.as_str().and_then(yaml_device_name);
    let locked = current.is_err()
        || c.policy.is_locked(
            &configuration,
            current.as_ref().ok().and_then(Option::as_deref),
        )
        || c.policy.is_locked(&configuration, from_yaml.as_deref());
    let name = current.ok().flatten();
    Ok(Json(json!({
        "content": content,
        "locked": locked,
        "flashable": !locked && c.policy.can_flash(&configuration, name.as_deref()),
        "target": c.policy.address(&configuration, name.as_deref()).map(|ip| ip.to_string()),
    })))
}

#[derive(Deserialize)]
pub(crate) struct ContentBody {
    content: String,
}

pub(crate) async fn put_config(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
    Path(configuration): Path<String>,
    Json(body): Json<ContentBody>,
) -> Result<Json<Value>, ApiError> {
    let c = ctx(&s, &user)?;
    writable(c, &configuration, Some(&body.content)).await?;
    if body.content.trim().is_empty() {
        return Err(ApiError::BadRequest("empty YAML".into()));
    }
    c.builder
        .call(
            "devices/update_config",
            json!({ "configuration": configuration, "content": body.content }),
        )
        .await?;
    Ok(Json(json!({ "saved": configuration })))
}

/// Checks a YAML in memory (nothing is written).
pub(crate) async fn check_config(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
    Path(configuration): Path<String>,
    Json(body): Json<ContentBody>,
) -> Result<Json<Value>, ApiError> {
    let c = ctx(&s, &user)?;
    check_name(&configuration)?;
    let r = c
        .builder
        .call(
            "editor/validate_yaml",
            json!({ "configuration": configuration, "content": body.content }),
        )
        .await?;
    Ok(Json(r))
}

#[derive(Deserialize)]
pub(crate) struct CreateBody {
    /// Host name (`salon-ir`).
    name: String,
    friendly_name: Option<String>,
    board_id: Option<String>,
    /// Imported YAML, written as is.
    content: Option<String>,
}

/// File name the builder gives a new project.
fn new_configuration(name: &str) -> String {
    format!("{}.yaml", name.trim())
}

pub(crate) async fn create(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
    Json(body): Json<CreateBody>,
) -> Result<Json<Value>, ApiError> {
    let c = ctx(&s, &user)?;
    let content = body.content.as_deref().filter(|t| !t.trim().is_empty());
    writable(c, &new_configuration(&body.name), content).await?;
    if c.policy.is_locked(&body.name, None) {
        return Err(locked_error(&body.name));
    }
    let mut args = json!({ "name": body.name.trim() });
    if let Some(f) = body.friendly_name.filter(|f| !f.trim().is_empty()) {
        args["friendly_name"] = f.trim().into();
    }
    if let Some(b) = body.board_id.filter(|b| !b.is_empty()) {
        args["board_id"] = b.into();
    }
    if let Some(content) = body.content.filter(|c| !c.trim().is_empty()) {
        args["file_content"] = content.into();
    }
    let r = c.builder.call("devices/create", args).await?;
    Ok(Json(r))
}

pub(crate) async fn delete(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
    Path(configuration): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let c = ctx(&s, &user)?;
    writable(c, &configuration, None).await?;
    c.builder
        .call("devices/delete", json!({ "configuration": configuration }))
        .await?;
    Ok(Json(json!({ "deleted": configuration })))
}

#[derive(Deserialize)]
pub(crate) struct BoardQuery {
    q: Option<String>,
}

/// Board catalogue of the builder, searched by name (first 50 matches).
pub(crate) async fn boards(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
    Query(q): Query<BoardQuery>,
) -> Result<Json<Value>, ApiError> {
    let c = ctx(&s, &user)?;
    let mut args = json!({ "limit": 50 });
    if let Some(query) = q.q.filter(|s| !s.trim().is_empty()) {
        args["query"] = query.trim().into();
    }
    Ok(Json(c.builder.call("boards/get_boards", args).await?))
}

/// Secret names only: values never leave the builder.
pub(crate) async fn secrets(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
) -> Result<Json<Value>, ApiError> {
    let c = ctx(&s, &user)?;
    let keys = c.builder.call("config/get_secrets", json!({})).await?;
    // Only well-formed key names leave, whatever the builder answers.
    let names: Vec<&str> = keys
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|k| valid_secret_key(k))
        .collect();
    Ok(Json(json!(names)))
}

#[derive(Deserialize)]
pub(crate) struct SecretBody {
    value: String,
}

pub(crate) async fn set_secret(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
    Path(key): Path<String>,
    Json(body): Json<SecretBody>,
) -> Result<Json<Value>, ApiError> {
    let c = ctx(&s, &user)?;
    if !valid_secret_key(&key) {
        return Err(ApiError::BadRequest(format!("invalid secret name {key}")));
    }
    let r = c
        .builder
        .call(
            "config/set_secret",
            json!({ "key": key, "value": body.value, "overwrite": true }),
        )
        .await?;
    Ok(Json(json!({ "created": r["created"] })))
}

pub(crate) async fn binaries(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
    Path(configuration): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let c = ctx(&s, &user)?;
    check_name(&configuration)?;
    Ok(Json(
        c.builder
            .call(
                "firmware/get_binaries",
                json!({ "configuration": configuration }),
            )
            .await?,
    ))
}

/// A firmware file of the last build, for download or USB flashing. Not for
/// locked projects.
pub(crate) async fn firmware(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
    Path((configuration, file)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let c = ctx(&s, &user)?;
    writable(c, &configuration, None).await?;
    if !valid_firmware_file(&file) {
        return Err(ApiError::BadRequest(format!(
            "invalid firmware file {file}"
        )));
    }
    let res = c.builder.download(&configuration, &file).await?;
    let stem = configuration.trim_end_matches(".yaml");
    Response::builder()
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{stem}-{file}\""),
        )
        .body(Body::from_stream(res.bytes_stream()))
        .map_err(|e| ApiError::Internal(e.to_string()))
}

/// `firmware.factory.bin`, `firmware.ota.bin`, `firmware.bin`, `firmware.uf2`.
fn valid_firmware_file(file: &str) -> bool {
    matches!(
        file,
        "firmware.factory.bin" | "firmware.ota.bin" | "firmware.bin" | "firmware.uf2"
    )
}

/// esp-web-tools chip family of an ESP32 factory image, read from its image
/// header (magic `0xE9`, chip id at byte 12), found at offset 0 (S3, C3…)
/// or 0x1000 (ESP32, S2). The builder's `config/get_info` cannot serialize
/// its answer in 2026.9.1, and the header is the ground truth anyway.
#[must_use]
pub(crate) fn chip_family(image: &[u8]) -> Option<&'static str> {
    [0usize, 0x1000].into_iter().find_map(|off| {
        let h = image.get(off..off + 14)?;
        if h[0] != 0xE9 {
            return None;
        }
        match u16::from_le_bytes([h[12], h[13]]) {
            0 => Some("ESP32"),
            2 => Some("ESP32-S2"),
            5 => Some("ESP32-C3"),
            9 => Some("ESP32-S3"),
            12 => Some("ESP32-C2"),
            13 => Some("ESP32-C6"),
            16 => Some("ESP32-H2"),
            _ => None,
        }
    })
}

/// esp-web-tools manifest of the last build (factory image at offset 0).
pub(crate) async fn manifest(
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
    Path(configuration): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let c = ctx(&s, &user)?;
    writable(c, &configuration, None).await?;
    let list = c.builder.call("devices/list", json!({})).await?;
    let device = list["configured"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|d| d["configuration"] == configuration.as_str())
        .ok_or_else(|| ApiError::NotFound(format!("unknown project {configuration}")))?;
    let no_build = || ApiError::Conflict("no build for this project yet: compile it first".into());
    let (family, file) = if device["target_platform"] == "esp8266" {
        ("ESP8266", "firmware.bin")
    } else {
        let image = c
            .builder
            .download(&configuration, "firmware.factory.bin")
            .await
            .map_err(|_| no_build())?
            .bytes()
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        (
            chip_family(&image).ok_or_else(no_build)?,
            "firmware.factory.bin",
        )
    };
    Ok(Json(json!({
        "name": device["friendly_name"].as_str().or(device["name"].as_str()).unwrap_or(&configuration),
        "version": device["current_version"],
        "new_install_prompt_erase": true,
        "builds": [{
            "chipFamily": family,
            "parts": [{ "path": format!("firmware/{file}"), "offset": 0 }],
        }],
    })))
}

// ---------- operations streamed over a WebSocket ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    Validate,
    Compile,
    Install,
    Logs,
}

#[derive(Deserialize)]
pub(crate) struct RunQuery {
    action: String,
}

/// Checks an operation against the policy; returns the builder `port` for
/// over-the-air operations: only the address configured for the project
/// (`RUSTHOME_ESPHOME_ADDRESSES`), never a typed address, a `.local` name or
/// a YAML `use_address`, any of which could reach an alarm node (code review
/// n° 2). `known` is false when the project is absent from the builder.
pub(crate) fn plan(
    policy: &Policy,
    configuration: &str,
    name: Option<&str>,
    known: bool,
    action: &str,
) -> Result<(Action, Option<String>), String> {
    let action = match action {
        "validate" => Action::Validate,
        "compile" => Action::Compile,
        "install" => Action::Install,
        "logs" => Action::Logs,
        other => return Err(format!("unknown action {other}")),
    };
    if !valid_configuration(configuration) {
        return Err(format!("invalid project file {configuration}"));
    }
    if action == Action::Validate {
        return Ok((action, None));
    }
    if !known {
        return Err(format!("unknown project {configuration}"));
    }
    if policy.is_locked(configuration, name) {
        return Err(format!(
            "{configuration} is locked (alarm node): no compilation, installation or logs"
        ));
    }
    if action == Action::Install && !policy.can_flash(configuration, name) {
        return Err(format!(
            "{configuration} is not allowed to be flashed over the air (RUSTHOME_ESPHOME_FLASHABLE)"
        ));
    }
    let port = match action {
        Action::Install | Action::Logs => Some(
            policy
                .address(configuration, name)
                .ok_or_else(|| {
                    format!(
                        "no address configured for {configuration} (RUSTHOME_ESPHOME_ADDRESSES=project@ip)"
                    )
                })?
                .to_string(),
        ),
        _ => None,
    };
    Ok((action, port))
}

pub(crate) async fn run(
    ws: WebSocketUpgrade,
    AxState(s): AxState<ApiState>,
    axum::Extension(user): axum::Extension<User>,
    Path(configuration): Path<String>,
    Query(q): Query<RunQuery>,
    headers: axum::http::HeaderMap,
) -> Result<Response, ApiError> {
    ctx(&s, &user)?;
    // A flash must never be started by another site open in the browser.
    if !crate::same_origin(&headers) {
        return Err(ApiError::Forbidden("cross-site WebSocket refused".into()));
    }
    let Some(c) = s.esphome.clone() else {
        return Err(ApiError::NotFound(
            "ESPHome Builder is not configured".into(),
        ));
    };
    // Fails closed: without the builder's list, only validation runs.
    let planned = match builder_name(&c, &configuration).await {
        Ok(name) => plan(
            &c.policy,
            &configuration,
            name.as_deref(),
            name.is_some(),
            &q.action,
        ),
        Err(_) if q.action == "validate" => {
            plan(&c.policy, &configuration, None, false, "validate")
        }
        Err(ApiError::Conflict(why)) => Err(why),
        Err(_) => Err(format!("cannot check the lock of {configuration}")),
    };
    match &planned {
        Ok((action, port)) => {
            tracing::info!(%configuration, ?action, ?port, user = %user.name, "ESPHome operation");
        }
        Err(why) => {
            tracing::warn!(%configuration, %why, user = %user.name, "ESPHome operation refused");
        }
    }
    // A refusal is reported on the socket: a browser cannot read the body
    // of a failed WebSocket handshake.
    Ok(ws
        .on_upgrade(move |socket| async move {
            let mut out = Out(socket);
            let done = match planned {
                Err(why) => json!({ "type": "done", "ok": false, "error": why }),
                Ok((action, port)) => {
                    let port = port.unwrap_or_default();
                    match operate(&c, &mut out, &configuration, action, &port).await {
                        Ok(Outcome::Done(ok)) => json!({ "type": "done", "ok": ok }),
                        Ok(Outcome::Closed) => return,
                        Err(e) => json!({ "type": "done", "ok": false, "error": e.to_string() }),
                    }
                }
            };
            let _ = out.send(done).await;
            let _ = out.0.send(Message::Close(None)).await;
        })
        .into_response())
}

struct Out(WebSocket);

impl Out {
    async fn send(&mut self, v: Value) -> bool {
        self.0.send(Message::text(v.to_string())).await.is_ok()
    }
    async fn line(&mut self, text: &str) -> bool {
        let text = text.trim_end_matches(['\n', '\r']);
        self.send(json!({ "type": "line", "text": text })).await
    }
    async fn step(&mut self, text: &str) -> bool {
        self.send(json!({ "type": "step", "text": text })).await
    }
}

enum Outcome {
    Done(bool),
    /// The browser closed the socket: the operation was cancelled.
    Closed,
}

async fn operate(
    c: &Arc<EsphomeCtx>,
    out: &mut Out,
    configuration: &str,
    action: Action,
    port: &str,
) -> Result<Outcome, BuilderError> {
    let conf = json!({ "configuration": configuration });
    match action {
        Action::Validate => {
            out.step("Validation de la configuration").await;
            let s = c.builder.stream("devices/validate", conf).await?;
            pipe(out, s, None, c).await
        }
        Action::Logs => {
            out.step("Connexion aux logs (Wi-Fi)").await;
            let s = c
                .builder
                .stream(
                    "devices/logs",
                    json!({ "configuration": configuration, "port": port }),
                )
                .await?;
            pipe(out, s, None, c).await
        }
        Action::Compile => {
            out.step("Compilation").await;
            job(c, out, "firmware/compile", conf).await
        }
        Action::Install => {
            // Compile, then upload: never `firmware/install`, which queues
            // an automatic flash for when an offline device comes back.
            out.step("Compilation").await;
            match job(c, out, "firmware/compile", conf).await? {
                Outcome::Done(true) => {}
                other => return Ok(other),
            }
            out.step("Installation sans fil (OTA)").await;
            job(
                c,
                out,
                "firmware/upload",
                json!({ "configuration": configuration, "port": port }),
            )
            .await
        }
    }
}

/// Submits a firmware job and follows it to the end.
async fn job(
    c: &Arc<EsphomeCtx>,
    out: &mut Out,
    command: &str,
    args: Value,
) -> Result<Outcome, BuilderError> {
    let job = c.builder.call(command, args).await?;
    let id = job["job_id"].as_str().unwrap_or_default().to_owned();
    let s = c
        .builder
        .stream("firmware/follow_job", json!({ "job_id": id }))
        .await?;
    pipe(out, s, Some(&id), c).await
}

/// Relays a builder stream to the browser. Closing the browser side cancels
/// the stream and, for a firmware job, the job itself.
async fn pipe(
    out: &mut Out,
    mut stream: rusthome_esphome::Stream<'_>,
    job_id: Option<&str>,
    c: &Arc<EsphomeCtx>,
) -> Result<Outcome, BuilderError> {
    let mut ok = None;
    loop {
        tokio::select! {
            frame = stream.next() => match frame {
                Some(Frame::Event { event, data }) => match event.as_str() {
                    "output" => {
                        let text = data.as_str().unwrap_or_default();
                        if !out.line(text).await {
                            break;
                        }
                    }
                    "result" => ok = Some(result_ok(&data)),
                    _ => {}
                },
                Some(Frame::Result(_)) | None => return Ok(Outcome::Done(ok.unwrap_or(false))),
                Some(Frame::Error { code, details }) => {
                    return Err(BuilderError::Command { code, details });
                }
            },
            incoming = out.0.recv() => match incoming {
                Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
        }
    }
    stream.cancel().await;
    if let Some(id) = job_id {
        let _ = c
            .builder
            .call("firmware/cancel", json!({ "job_id": id }))
            .await;
    }
    Ok(Outcome::Closed)
}

/// `{success, code}` (validation, logs) or `{status, exit_code}` (jobs).
fn result_ok(data: &Value) -> bool {
    data["success"] == true || data["status"] == "completed"
}

pub(crate) fn routes() -> axum::Router<ApiState> {
    use axum::routing::{get, post, put};
    axum::Router::new()
        .route("/esphome", get(overview))
        .route("/esphome/boards", get(boards))
        .route("/esphome/secrets", get(secrets))
        .route("/esphome/secrets/{key}", put(set_secret))
        .route("/esphome/configs", post(create))
        .route(
            "/esphome/configs/{configuration}",
            get(get_config).put(put_config).delete(delete),
        )
        .route("/esphome/configs/{configuration}/check", post(check_config))
        .route("/esphome/configs/{configuration}/run", get(run))
        .route("/esphome/configs/{configuration}/binaries", get(binaries))
        .route(
            "/esphome/configs/{configuration}/manifest.json",
            get(manifest),
        )
        .route(
            "/esphome/configs/{configuration}/firmware/{file}",
            get(firmware),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_policy_gates_every_operation() {
        let p = Policy::default()
            .with_addresses(["test-nfc@192.168.1.80".into(), "ir-box@192.168.1.81".into()])
            .unwrap();
        // alarm node: only validation
        assert!(plan(&p, "lecteur-alarme.yaml", None, true, "validate").is_ok());
        for a in ["compile", "install", "logs"] {
            assert!(
                plan(&p, "lecteur-alarme.yaml", None, true, a).is_err(),
                "{a}"
            );
            assert!(
                plan(&p, "x.yaml", Some("tag-reader"), true, a).is_err(),
                "{a}"
            );
            // unknown to the builder: refused (fail closed)
            assert!(plan(&p, "test-nfc.yaml", None, false, a).is_err(), "{a}");
        }
        // other nodes: logs and compile, but OTA only for the bench
        assert_eq!(
            plan(&p, "ir-box.yaml", Some("ir-box"), true, "logs"),
            Ok((Action::Logs, Some("192.168.1.81".to_owned())))
        );
        assert_eq!(
            plan(&p, "ir-box.yaml", Some("ir-box"), true, "compile"),
            Ok((Action::Compile, None))
        );
        assert!(plan(&p, "ir-box.yaml", Some("ir-box"), true, "install").is_err());
        // the target is always the configured address
        assert_eq!(
            plan(&p, "test-nfc.yaml", Some("test-nfc"), true, "install"),
            Ok((Action::Install, Some("192.168.1.80".to_owned())))
        );
        // no configured address: no over-the-air operation at all
        let bare = Policy::default();
        assert!(plan(&bare, "test-nfc.yaml", Some("test-nfc"), true, "install").is_err());
        assert!(plan(&bare, "test-nfc.yaml", Some("test-nfc"), true, "logs").is_err());
        assert!(plan(&p, "test-nfc.yaml", None, true, "rename").is_err());
        assert!(plan(&p, "secrets.yaml", None, true, "validate").is_err());
    }

    #[test]
    fn summaries_carry_the_policy() {
        let d = json!({
            "configuration": "tag-reader.yaml", "name": "tag-reader",
            "current_version": "2026.9.1", "runtime_state": { "state": "offline" }
        });
        let s = summary(&d, Some("online"), &Policy::default());
        assert_eq!(s["state"], "online");
        assert_eq!(s["locked"], true);
        assert_eq!(s["flashable"], false);
        assert_eq!(summary(&d, None, &Policy::default())["state"], "offline");
    }

    #[test]
    fn chip_families_from_image_headers() {
        let image = |off: usize, chip: u16| {
            let mut v = vec![0xFFu8; off + 64];
            v[off] = 0xE9;
            v[off + 12..off + 14].copy_from_slice(&chip.to_le_bytes());
            v
        };
        assert_eq!(chip_family(&image(0, 9)), Some("ESP32-S3"));
        assert_eq!(chip_family(&image(0x1000, 2)), Some("ESP32-S2"));
        assert_eq!(chip_family(&image(0x1000, 0)), Some("ESP32"));
        assert_eq!(chip_family(&image(0, 77)), None);
        assert_eq!(chip_family(&[0u8; 10]), None);
        assert!(valid_firmware_file("firmware.factory.bin"));
        assert!(!valid_firmware_file("../secrets.yaml"));
    }

    #[test]
    fn job_and_stream_results() {
        assert!(result_ok(&json!({ "success": true, "code": 0 })));
        assert!(result_ok(&json!({ "status": "completed", "exit_code": 0 })));
        assert!(!result_ok(&json!({ "status": "failed", "exit_code": 1 })));
        assert_eq!(new_configuration(" salon "), "salon.yaml");
    }
}
