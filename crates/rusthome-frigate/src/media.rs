//! Frigate media for the media browser: events (clips, snapshots), recording
//! summaries and the files themselves. GET only; camera names and event ids
//! from the browser are validated before any request.

use serde_json::Value;

use crate::Frigate;

#[derive(Debug, thiserror::Error)]
pub enum MediaError {
    #[error("unknown camera {0}")]
    UnknownCamera(String),
    #[error("invalid identifier {0}")]
    Invalid(String),
    #[error("Frigate: {0}")]
    Http(#[from] reqwest::Error),
}

/// Frigate event ids look like `1729212345.123456-abc123`.
#[must_use]
pub fn valid_event_id(id: &str) -> bool {
    // Starting with an alphanumeric character rules out `.` and `..`
    // (code review n° 42).
    id.len() <= 80
        && id.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

/// A file of an HLS VOD stream (`index.m3u8`, `init-v1.mp4`, `seg-3-v1.m4s`):
/// a plain file name, never a path.
#[must_use]
pub fn valid_vod_file(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        && [".m3u8", ".mp4", ".m4s"].iter().any(|x| name.ends_with(x))
}

/// `Europe/Paris`, `UTC`, `America/Argentina/Buenos_Aires`…
#[must_use]
pub fn valid_timezone(tz: &str) -> bool {
    !tz.is_empty()
        && tz.len() <= 64
        && tz
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '_' | '-' | '+'))
        && !tz.contains("..")
}

/// Which events: all, with a clip, or with a snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EventKind {
    #[default]
    All,
    Clips,
    Snapshots,
}

/// Filters of an events query.
#[derive(Debug, Default)]
pub struct EventFilter<'a> {
    pub camera: Option<&'a str>,
    pub label: Option<&'a str>,
    pub before: Option<f64>,
    pub kind: EventKind,
    pub limit: u32,
}

/// One media file of Frigate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaFile {
    EventSnapshot(String),
    EventThumbnail(String),
    EventClip(String),
    /// Camera, start and end (Unix seconds): an MP4 cut from the recordings.
    Recording(String, i64, i64),
    Latest(String),
    /// HLS VOD of an event (id, file). Frigate's `clip.mp4` files are
    /// fragmented MP4 without a total duration: browsers only play the first
    /// fragment (~4 s), so videos are played through HLS like Frigate's UI.
    EventVod(String, String),
    /// HLS VOD of recordings (camera, start, end, file).
    RecordingVod(String, i64, i64, String),
}

impl MediaFile {
    /// Path on the Frigate API.
    #[must_use]
    pub fn path(&self) -> String {
        match self {
            Self::EventSnapshot(id) => format!("/api/events/{id}/snapshot.jpg?bbox=1"),
            Self::EventThumbnail(id) => format!("/api/events/{id}/thumbnail.jpg"),
            Self::EventClip(id) => format!("/api/events/{id}/clip.mp4"),
            Self::Recording(cam, s, e) => format!("/api/{cam}/start/{s}/end/{e}/clip.mp4"),
            Self::Latest(cam) => format!("/api/{cam}/latest.jpg?h=720"),
            Self::EventVod(id, f) => format!("/vod/event/{id}/{f}"),
            Self::RecordingVod(cam, s, e, f) => format!("/vod/{cam}/start/{s}/end/{e}/{f}"),
        }
    }
}

impl Frigate {
    /// Names of the cameras Frigate knows.
    pub fn camera_names(&self) -> Vec<String> {
        self.cameras
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .map(|c| c.name.clone())
            .collect()
    }

    fn known(&self, camera: &str) -> Result<(), MediaError> {
        if self.camera_names().iter().any(|c| c == camera) {
            Ok(())
        } else {
            Err(MediaError::UnknownCamera(camera.to_owned()))
        }
    }

    /// A recording window: ordered, at most 4 hours.
    fn check_range(camera: Result<(), MediaError>, s: i64, e: i64) -> Result<(), MediaError> {
        camera?;
        if e <= s || e - s > 4 * 3600 {
            return Err(MediaError::Invalid(format!("{s}-{e}")));
        }
        Ok(())
    }

    /// Checks a file request before it reaches Frigate.
    pub fn check(&self, f: &MediaFile) -> Result<(), MediaError> {
        match f {
            MediaFile::EventSnapshot(id)
            | MediaFile::EventThumbnail(id)
            | MediaFile::EventClip(id) => {
                if valid_event_id(id) {
                    Ok(())
                } else {
                    Err(MediaError::Invalid(id.clone()))
                }
            }
            MediaFile::Recording(cam, s, e) => Self::check_range(self.known(cam), *s, *e),
            MediaFile::Latest(cam) => self.known(cam),
            MediaFile::EventVod(id, f) => {
                if valid_event_id(id) && valid_vod_file(f) {
                    Ok(())
                } else {
                    Err(MediaError::Invalid(format!("{id}/{f}")))
                }
            }
            MediaFile::RecordingVod(cam, s, e, f) => {
                if !valid_vod_file(f) {
                    return Err(MediaError::Invalid(f.clone()));
                }
                Self::check_range(self.known(cam), *s, *e)
            }
        }
    }

    /// Latest events (newest first), optionally for one camera and label.
    pub async fn events(&self, q: &EventFilter<'_>) -> Result<Value, MediaError> {
        let mut parts = vec![
            format!("limit={}", q.limit.clamp(1, 200)),
            "include_thumbnails=0".into(),
        ];
        if let Some(c) = q.camera {
            self.known(c)?;
            parts.push(format!("cameras={c}"));
        }
        if let Some(l) = q.label {
            if !l.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                return Err(MediaError::Invalid(l.to_owned()));
            }
            parts.push(format!("labels={l}"));
        }
        if let Some(b) = q.before {
            parts.push(format!("before={b}"));
        }
        match q.kind {
            EventKind::All => {}
            EventKind::Clips => parts.push("has_clip=1".into()),
            EventKind::Snapshots => parts.push("has_snapshot=1".into()),
        }
        Ok(self
            .get(&format!("/api/events?{}", parts.join("&")))
            .await?)
    }

    /// Recorded hours per day of a camera (`/api/<camera>/recordings/summary`),
    /// days and hours in `timezone` (Frigate answers in UTC otherwise).
    pub async fn recordings_summary(
        &self,
        camera: &str,
        timezone: Option<&str>,
    ) -> Result<Value, MediaError> {
        self.known(camera)?;
        let tz = match timezone {
            Some(t) if valid_timezone(t) => format!("?timezone={t}"),
            Some(t) => return Err(MediaError::Invalid(t.to_owned())),
            None => String::new(),
        };
        Ok(self
            .get(&format!("/api/{camera}/recordings/summary{tz}"))
            .await?)
    }

    /// Starts downloading a file; `range` is forwarded for video seeking.
    pub async fn open(
        &self,
        f: &MediaFile,
        range: Option<&str>,
    ) -> Result<reqwest::Response, MediaError> {
        self.check(f)?;
        let url = format!("{}{}", self.api.as_str().trim_end_matches('/'), f.path());
        // no overall timeout: a clip may take a while to stream
        let mut req = self.media_http.get(url);
        if let Some(r) = range {
            req = req.header(reqwest::header::RANGE, r);
        }
        Ok(req.send().await?.error_for_status()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timezones() {
        assert!(valid_timezone("Europe/Paris"));
        assert!(valid_timezone("Etc/GMT+1"));
        assert!(!valid_timezone("Europe/../x"));
        assert!(!valid_timezone("Paris&limit=1"));
        assert!(!valid_timezone(""));
    }

    #[test]
    fn event_ids_and_paths() {
        assert!(valid_event_id("1729212345.123456-abc123"));
        assert!(!valid_event_id("../config"));
        assert!(!valid_event_id(".."));
        assert!(!valid_event_id("."));
        assert!(!valid_event_id("a/b"));
        assert!(!valid_event_id(""));
        assert_eq!(
            MediaFile::Recording("salon".into(), 10, 20).path(),
            "/api/salon/start/10/end/20/clip.mp4"
        );
        assert_eq!(
            MediaFile::EventClip("e1".into()).path(),
            "/api/events/e1/clip.mp4"
        );
        assert_eq!(
            MediaFile::RecordingVod("salon".into(), 10, 20, "index.m3u8".into()).path(),
            "/vod/salon/start/10/end/20/index.m3u8"
        );
        assert_eq!(
            MediaFile::EventVod("e1".into(), "seg-1-v1.m4s".into()).path(),
            "/vod/event/e1/seg-1-v1.m4s"
        );
    }

    #[test]
    fn vod_files() {
        for ok in ["index.m3u8", "master.m3u8", "init-v1.mp4", "seg-12-v1.m4s"] {
            assert!(valid_vod_file(ok), "{ok}");
        }
        for bad in [
            "",
            "../index.m3u8",
            "a/b.m4s",
            ".m3u8",
            "x.jpg",
            "index.m3u8?x=1",
        ] {
            assert!(!valid_vod_file(bad), "{bad}");
        }
    }
}
