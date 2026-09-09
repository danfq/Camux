use super::{CameraBackend, CameraDevice};
use std::fs;
use std::path::Path;

pub struct LinuxBackend;

impl LinuxBackend {
    pub fn new() -> Self {
        Self
    }
}

impl CameraBackend for LinuxBackend {
    fn devices(&self) -> Result<Vec<CameraDevice>, String> {
        let mut devices = fs::read_dir(Path::new("/dev"))
            .map_err(|error| error.to_string())?
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name();
                let name = name.to_str()?;

                if name.starts_with("video") {
                    Some(entry.path().display().to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        devices.sort();

        Ok(devices)
    }
}
