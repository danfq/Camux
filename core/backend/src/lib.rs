mod platform;

use platform::{CameraBackend, CameraDevice, PlatformBackend};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tauri::webview::PageLoadEvent;

#[cfg(target_os = "linux")]
fn reveal_when_loaded(
    window: &tauri::WebviewWindow,
    content_loaded: Arc<AtomicBool>,
) -> Result<(), Box<dyn std::error::Error>> {
    use gtk::{glib::ControlFlow, prelude::*};
    use std::time::Duration;

    let gtk_window = window.gtk_window()?;
    gtk::glib::timeout_add_local(Duration::from_millis(8), move || {
        if content_loaded.load(Ordering::Acquire) {
            gtk_window.show_all();
            gtk_window.present();
            ControlFlow::Break
        } else {
            ControlFlow::Continue
        }
    });

    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn reveal_when_loaded(
    _window: &tauri::WebviewWindow,
    _content_loaded: Arc<AtomicBool>,
) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

#[tauri::command]
fn get_devices() -> Result<Vec<CameraDevice>, String> {
    PlatformBackend::new().devices()
}

#[tauri::command]
fn show_main_window(window: tauri::WebviewWindow) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let _ = window;
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    {
        window.show().map_err(|error| error.to_string())?;
        window.set_focus().map_err(|error| error.to_string())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_os::init())
        .setup(|app| {
            let window_config = app
                .config()
                .app
                .windows
                .first()
                .cloned()
                .ok_or_else(|| std::io::Error::other("missing main window configuration"))?;

            let content_loaded = Arc::new(AtomicBool::new(false));
            let load_signal = Arc::clone(&content_loaded);
            let window = tauri::WebviewWindowBuilder::from_config(app, &window_config)?
                .on_page_load(move |_window, payload| {
                    if payload.event() == PageLoadEvent::Finished {
                        load_signal.store(true, Ordering::Release);
                    }
                })
                .build()?;

            reveal_when_loaded(&window, content_loaded)?;

            // In addition to AppKit's autoresizing mask, keep the webview bounds in sync
            // on every native resize event. This avoids a stale viewport during macOS's
            // title-bar zoom animation.
            window.as_ref().set_auto_resize(true)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_devices, show_main_window])
        .run(tauri::generate_context!())
        .expect("failed to run Camux");
}
