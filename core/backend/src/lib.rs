mod platform;

use platform::{CameraBackend, CameraDevice, PlatformBackend};

#[tauri::command]
fn get_devices() -> Result<Vec<CameraDevice>, String> {
    PlatformBackend::new().devices()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let window_config = app
                .config()
                .app
                .windows
                .first()
                .cloned()
                .ok_or_else(|| std::io::Error::other("missing main window configuration"))?;

            let window = tauri::WebviewWindowBuilder::from_config(app, &window_config)?.build()?;

            // In addition to AppKit's autoresizing mask, keep the webview bounds in sync
            // on every native resize event. This avoids a stale viewport during macOS's
            // title-bar zoom animation.
            window.as_ref().set_auto_resize(true)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_devices])
        .run(tauri::generate_context!())
        .expect("failed to run Camux");
}
