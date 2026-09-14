#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraDevice {
    pub id: String,
    pub name: String,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub serial_number: Option<String>,
    pub transport: Option<String>,
    pub device_type: Option<String>,
    pub position: CameraPosition,
    pub connected: bool,
    pub in_use: Option<bool>,
    pub in_use_by: Vec<String>,
    pub suspended: Option<bool>,
    pub driver: Option<CameraDriver>,
    pub capability_bits: Option<u32>,
    pub capabilities: Vec<String>,
    pub formats: Vec<CameraFormat>,
    pub active_format: Option<CameraActiveFormat>,
    pub controls: Vec<CameraControl>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraAvailability {
    pub id: String,
    pub connected: bool,
    pub in_use: bool,
    pub in_use_by: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraDriver {
    pub name: String,
    pub version: String,
    pub bus_info: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(target_os = "linux", allow(dead_code))]
pub enum CameraPosition {
    Front,
    Back,
    Unspecified,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraFormat {
    pub pixel_format: String,
    pub description: Option<String>,
    pub compressed: bool,
    pub emulated: bool,
    pub flag_bits: Option<u32>,
    pub frame_sizes: Vec<CameraFrameSize>,
    pub field_of_view: Option<f32>,
    pub binned: Option<bool>,
    pub hdr: Option<bool>,
    pub min_iso: Option<f32>,
    pub max_iso: Option<f32>,
}

#[derive(Debug, serde::Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CameraFrameSize {
    Discrete {
        width: u32,
        height: u32,
        frame_rates: Vec<CameraFrameRateRange>,
    },
    Stepwise {
        min_width: u32,
        max_width: u32,
        width_step: u32,
        min_height: u32,
        max_height: u32,
        height_step: u32,
        frame_rates: Vec<CameraFrameRateRange>,
    },
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraFrameRateRange {
    pub min: f64,
    pub max: f64,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraActiveFormat {
    pub pixel_format: String,
    pub width: u32,
    pub height: u32,
    pub frame_rate: Option<f64>,
    pub stride: Option<u32>,
    pub image_size: Option<u32>,
    pub field_order: Option<String>,
    pub color_space: Option<String>,
    pub quantization: Option<String>,
    pub transfer_function: Option<String>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraControl {
    pub id: u32,
    pub name: String,
    pub control_type: String,
    pub minimum: i64,
    pub maximum: i64,
    pub step: u64,
    pub default: i64,
    pub value: Option<serde_json::Value>,
    pub flag_bits: u32,
    pub flags: Vec<String>,
    pub menu_items: Vec<CameraControlMenuItem>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraControlMenuItem {
    pub index: u32,
    pub name: Option<String>,
    pub value: Option<i64>,
}

pub trait CameraBackend {
    fn devices(&self) -> Result<Vec<CameraDevice>, String>;

    fn availability(&self, device_ids: &[String]) -> Result<Vec<CameraAvailability>, String>;

    fn set_control(
        &self,
        device_id: &str,
        control_id: u32,
        value: serde_json::Value,
    ) -> Result<Vec<CameraControl>, String>;
}

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "linux")]
pub use linux::LinuxBackend as PlatformBackend;

#[cfg(target_os = "macos")]
pub use macos::MacOSBackend as PlatformBackend;
