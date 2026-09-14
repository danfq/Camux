use super::{CameraAvailability, CameraBackend, CameraControl, CameraDevice};

pub struct UnsupportedBackend;

impl UnsupportedBackend {
    pub fn new() -> Self {
        Self
    }
}

impl CameraBackend for UnsupportedBackend {
    fn devices(&self) -> Result<Vec<CameraDevice>, String> {
        Ok(Vec::new())
    }

    fn availability(&self, device_ids: &[String]) -> Result<Vec<CameraAvailability>, String> {
        Ok(device_ids
            .iter()
            .map(|id| CameraAvailability {
                id: id.clone(),
                connected: false,
                in_use: false,
                in_use_by: Vec::new(),
            })
            .collect())
    }

    fn set_control(
        &self,
        _device_id: &str,
        _control_id: u32,
        _value: serde_json::Value,
    ) -> Result<Vec<CameraControl>, String> {
        Err("Camera controls are unsupported on this platform".to_owned())
    }
}
