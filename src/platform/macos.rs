use super::{CameraBackend, CameraDevice};

pub struct MacOSBackend;

impl MacOSBackend {
    pub fn new() -> Self {
        Self
    }
}

impl CameraBackend for MacOSBackend {
    fn devices(&self) -> Result<Vec<CameraDevice>, String> {
        Ok(vec![])
    }
}
