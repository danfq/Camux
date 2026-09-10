mod platform;

use platform::{CameraBackend, CameraDevice, PlatformBackend};

#[tauri::command]
fn get_devices() -> Result<Vec<CameraDevice>, String> {
    PlatformBackend::new().devices()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_devices])
        .run(tauri::generate_context!())
        .expect("failed to run Camux");
}
