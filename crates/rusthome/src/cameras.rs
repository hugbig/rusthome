//! Camera snapshots for dashboards: Frigate when it owns the camera, else
//! HA's camera proxy (read-only GET) while the camera is still mirrored.

use std::sync::Arc;

use rusthome_api::dashboards::{CameraSource, ImageFuture};
use rusthome_ha_bridge::BridgeConfig;
use rusthome_model::EntityId;

pub struct Cameras {
    pub frigate: Option<Arc<rusthome_frigate::Frigate>>,
    pub ha: Option<BridgeConfig>,
}

impl CameraSource for Cameras {
    fn snapshot<'a>(&'a self, entity_id: &'a EntityId) -> ImageFuture<'a> {
        Box::pin(async move {
            if let Some(f) = &self.frigate
                && let Some(cam) = f.camera_for(entity_id)
            {
                return f
                    .snapshot(&cam)
                    .await
                    .map(|b| ("image/jpeg".to_owned(), b))
                    .map_err(|e| format!("Frigate {cam}: {e}"));
            }
            match &self.ha {
                Some(ha) => rusthome_ha_bridge::rest::camera_snapshot(ha, entity_id.as_str())
                    .await
                    .map_err(|e| e.to_string()),
                None => Err(format!("no source for {entity_id}")),
            }
        })
    }
}
