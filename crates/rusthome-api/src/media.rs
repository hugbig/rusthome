//! Media browser: Frigate (events, clips, snapshots, recordings) through the
//! native integration, and HA media sources (local media…) read-only. Files
//! are streamed through RustHome so the page stays HTTPS; `Range` is
//! forwarded for video seeking.

use axum::Json;
use axum::body::Body;
use axum::extract::{Path, Query, State as AxState};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use rusthome_frigate::Frigate;
use rusthome_frigate::media::{EventFilter, EventKind, MediaError, MediaFile};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::dashboards::ha_of;
use crate::{ApiError, ApiState};

fn frigate(s: &ApiState) -> Result<&Frigate, ApiError> {
    s.frigate
        .as_deref()
        .ok_or_else(|| ApiError::NotFound("Frigate is not configured".into()))
}

impl From<MediaError> for ApiError {
    fn from(e: MediaError) -> Self {
        match e {
            MediaError::UnknownCamera(_) | MediaError::Invalid(_) => {
                ApiError::BadRequest(e.to_string())
            }
            MediaError::Http(_) => ApiError::Internal(e.to_string()),
        }
    }
}

/// Streams an upstream response (status, type, length, range) to the browser.
fn relay(res: reqwest::Response) -> Response {
    let status = StatusCode::from_u16(res.status().as_u16()).unwrap_or(StatusCode::OK);
    let mut out = Response::builder().status(status);
    for h in [
        header::CONTENT_TYPE,
        header::CONTENT_LENGTH,
        header::CONTENT_RANGE,
        header::ACCEPT_RANGES,
    ] {
        if let Some(v) = res.headers().get(h.as_str()) {
            out = out.header(h, v.as_bytes());
        }
    }
    out.header(header::CACHE_CONTROL, "private, max-age=300")
        .body(Body::from_stream(res.bytes_stream()))
        .unwrap_or_else(|_| StatusCode::BAD_GATEWAY.into_response())
}

fn range(headers: &HeaderMap) -> Option<&str> {
    headers.get(header::RANGE).and_then(|v| v.to_str().ok())
}

pub(crate) async fn frigate_cameras(
    AxState(s): AxState<ApiState>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({ "cameras": frigate(&s)?.camera_names() })))
}

#[derive(Deserialize)]
pub(crate) struct EventsQuery {
    camera: Option<String>,
    label: Option<String>,
    before: Option<f64>,
    limit: Option<u32>,
    /// `clips`, `snapshots` or nothing (all events).
    kind: Option<String>,
}

pub(crate) async fn frigate_events(
    AxState(s): AxState<ApiState>,
    Query(q): Query<EventsQuery>,
) -> Result<Json<Value>, ApiError> {
    let kind = match q.kind.as_deref() {
        None | Some("" | "all") => EventKind::All,
        Some("clips") => EventKind::Clips,
        Some("snapshots") => EventKind::Snapshots,
        Some(other) => return Err(ApiError::BadRequest(format!("unknown kind {other}"))),
    };
    let v = frigate(&s)?
        .events(&EventFilter {
            camera: q.camera.as_deref().filter(|c| !c.is_empty()),
            label: q.label.as_deref().filter(|l| !l.is_empty()),
            before: q.before,
            kind,
            limit: q.limit.unwrap_or(48),
        })
        .await?;
    Ok(Json(v))
}

#[derive(Deserialize)]
pub(crate) struct TzQuery {
    /// Time zone of the browser, so days and hours match the clock.
    timezone: Option<String>,
}

pub(crate) async fn frigate_recordings(
    AxState(s): AxState<ApiState>,
    Path(camera): Path<String>,
    Query(q): Query<TzQuery>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        frigate(&s)?
            .recordings_summary(&camera, q.timezone.as_deref().filter(|t| !t.is_empty()))
            .await?,
    ))
}

#[derive(Deserialize)]
pub(crate) struct FileQuery {
    id: Option<String>,
    camera: Option<String>,
    start: Option<i64>,
    end: Option<i64>,
}

fn media_file(kind: &str, q: &FileQuery) -> Result<MediaFile, ApiError> {
    let id = || {
        q.id.clone()
            .ok_or_else(|| ApiError::BadRequest("id is required".into()))
    };
    let camera = || {
        q.camera
            .clone()
            .ok_or_else(|| ApiError::BadRequest("camera is required".into()))
    };
    Ok(match kind {
        "snapshot" => MediaFile::EventSnapshot(id()?),
        "thumbnail" => MediaFile::EventThumbnail(id()?),
        "clip" => MediaFile::EventClip(id()?),
        "latest" => MediaFile::Latest(camera()?),
        "recording" => MediaFile::Recording(
            camera()?,
            q.start
                .ok_or_else(|| ApiError::BadRequest("start is required".into()))?,
            q.end
                .ok_or_else(|| ApiError::BadRequest("end is required".into()))?,
        ),
        other => return Err(ApiError::BadRequest(format!("unknown media kind {other}"))),
    })
}

pub(crate) async fn frigate_file(
    AxState(s): AxState<ApiState>,
    Path(kind): Path<String>,
    Query(q): Query<FileQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let f = media_file(&kind, &q)?;
    let res = frigate(&s)?.open(&f, range(&headers)).await?;
    Ok(relay(res))
}

/// HLS VOD of an event: playlist and segments, relayed as they are (their
/// URLs are relative, so they come back through these routes).
pub(crate) async fn frigate_event_vod(
    AxState(s): AxState<ApiState>,
    Path((id, file)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let res = frigate(&s)?
        .open(&MediaFile::EventVod(id, file), None)
        .await?;
    Ok(relay(res))
}

/// HLS VOD of an hour (or less) of recordings.
pub(crate) async fn frigate_recording_vod(
    AxState(s): AxState<ApiState>,
    Path((camera, start, end, file)): Path<(String, i64, i64, String)>,
) -> Result<Response, ApiError> {
    let res = frigate(&s)?
        .open(&MediaFile::RecordingVod(camera, start, end, file), None)
        .await?;
    Ok(relay(res))
}

#[derive(Deserialize)]
pub(crate) struct IdQuery {
    id: Option<String>,
    path: Option<String>,
}

/// One level of the HA media sources (`media-source://…`).
pub(crate) async fn ha_browse(
    AxState(s): AxState<ApiState>,
    Query(q): Query<IdQuery>,
) -> Result<Json<Value>, ApiError> {
    let id = q.id.as_deref().filter(|i| !i.is_empty());
    if let Some(i) = id
        && !i.starts_with("media-source://")
    {
        return Err(ApiError::BadRequest("only media-source:// ids".into()));
    }
    rusthome_ha_bridge::browse_media(ha_of(&s)?, id)
        .await
        .map(Json)
        .map_err(|e| ApiError::Internal(e.to_string()))
}

/// Plays a HA media: resolves its signed URL, then streams it.
pub(crate) async fn ha_play(
    AxState(s): AxState<ApiState>,
    Query(q): Query<IdQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let id =
        q.id.filter(|i| i.starts_with("media-source://"))
            .ok_or_else(|| ApiError::BadRequest("a media-source:// id is required".into()))?;
    let ha = ha_of(&s)?;
    let resolved = rusthome_ha_bridge::resolve_media(ha, &id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let url = resolved["url"].as_str().unwrap_or_default();
    let res = rusthome_ha_bridge::rest::open_media(ha, url, range(&headers))
        .await
        .map_err(|e| ApiError::NotFound(e.to_string()))?;
    Ok(relay(res))
}

/// A HA media file by path (thumbnails of the media browser), restricted to
/// media paths.
pub(crate) async fn ha_file(
    AxState(s): AxState<ApiState>,
    Query(q): Query<IdQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let path = q
        .path
        .filter(|p| rusthome_ha_bridge::rest::is_media_path(p))
        .ok_or_else(|| ApiError::BadRequest("not a media path".into()))?;
    let res = rusthome_ha_bridge::rest::open_media(ha_of(&s)?, &path, range(&headers))
        .await
        .map_err(|e| ApiError::NotFound(e.to_string()))?;
    Ok(relay(res))
}

/// Routes of the media browser.
pub(crate) fn routes() -> axum::Router<ApiState> {
    axum::Router::new()
        .route("/media/frigate", axum::routing::get(frigate_cameras))
        .route("/media/frigate/events", axum::routing::get(frigate_events))
        .route(
            "/media/frigate/{camera}/recordings",
            axum::routing::get(frigate_recordings),
        )
        .route(
            "/media/frigate/file/{kind}",
            axum::routing::get(frigate_file),
        )
        .route(
            "/media/frigate/vod/event/{id}/{file}",
            axum::routing::get(frigate_event_vod),
        )
        .route(
            "/media/frigate/vod/recording/{camera}/{start}/{end}/{file}",
            axum::routing::get(frigate_recording_vod),
        )
        .route("/media/ha/browse", axum::routing::get(ha_browse))
        .route("/media/ha/play", axum::routing::get(ha_play))
        .route("/media/ha/file", axum::routing::get(ha_file))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_kinds_need_their_parameters() {
        let q = |id: Option<&str>, cam: Option<&str>| FileQuery {
            id: id.map(str::to_owned),
            camera: cam.map(str::to_owned),
            start: Some(10),
            end: Some(20),
        };
        assert_eq!(
            media_file("clip", &q(Some("e1"), None)).ok(),
            Some(MediaFile::EventClip("e1".into()))
        );
        assert!(media_file("clip", &q(None, None)).is_err());
        assert_eq!(
            media_file("recording", &q(None, Some("salon"))).ok(),
            Some(MediaFile::Recording("salon".into(), 10, 20))
        );
        assert!(media_file("config", &q(Some("x"), None)).is_err());
    }
}
