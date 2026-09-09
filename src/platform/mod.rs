#[derive(Debug)]
pub struct CameraDevice {
    pub id: String,
    pub name: String,
}

pub trait CameraBackend {
    fn devices(&self) -> Result<Vec<CameraDevice>, String>;
}

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "linux")]
pub use linux::LinuxBackend as PlatformBackend;

#[cfg(target_os = "macos")]
pub use macos::MacOSBackend as PlatformBackend;
