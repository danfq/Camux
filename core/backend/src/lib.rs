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

#[cfg(target_os = "macos")]
fn configure_macos_webview(
    window: &tauri::WebviewWindow,
) -> Result<(), Box<dyn std::error::Error>> {
    window.with_webview(|webview| unsafe {
        use objc2_app_kit::{NSAutoresizingMaskOptions, NSColor, NSView, NSWindow};

        let ns_window = &*webview.ns_window().cast::<NSWindow>();
        let webview = &*webview.inner().cast::<NSView>();

        // Wry installs the WKWebView inside an intermediate NSView. Keep both
        // views tied to their superview bounds so AppKit resizes them throughout
        // the native title-bar zoom animation instead of after it completes.
        if let Some(container) = webview.superview() {
            container.setAutoresizesSubviews(true);
            container.setAutoresizingMask(
                NSAutoresizingMaskOptions::ViewWidthSizable
                    | NSAutoresizingMaskOptions::ViewHeightSizable,
            );
            webview.setFrame(container.bounds());
        }
        webview.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );

        // Match the native backing view to the web content so even an in-flight
        // resize cannot expose AppKit's default light background.
        let background = NSColor::colorWithSRGBRed_green_blue_alpha(
            17.0 / 255.0,
            19.0 / 255.0,
            23.0 / 255.0,
            1.0,
        );
        ns_window.setBackgroundColor(Some(&background));
    })?;

    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn configure_macos_webview(
    _window: &tauri::WebviewWindow,
) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

#[cfg(all(debug_assertions, target_os = "macos"))]
fn restore_macos_bundled_icon() {
    let is_app_bundle = std::env::current_exe().is_ok_and(|executable| {
        executable
            .ancestors()
            .any(|path| path.extension().is_some_and(|extension| extension == "app"))
    });
    if !is_app_bundle {
        return;
    }

    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;

    // Tauri assigns its PNG fallback during the Ready event in development.
    // Clearing that override restores the adaptive icon from the dev bundle's
    // CFBundleIconName and Assets.car.
    let mtm = unsafe { MainThreadMarker::new_unchecked() };
    let application = NSApplication::sharedApplication(mtm);
    unsafe { application.setApplicationIconImage(None) };
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
    let app =
        tauri::Builder::default()
            .plugin(tauri_plugin_os::init())
            .setup(|app| {
                let window_config =
                    app.config().app.windows.first().cloned().ok_or_else(|| {
                        std::io::Error::other("missing main window configuration")
                    })?;

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
                configure_macos_webview(&window)?;

                Ok(())
            })
            .invoke_handler(tauri::generate_handler![get_devices, show_main_window])
            .build(tauri::generate_context!())
            .expect("failed to run Camux");

    app.run(|_app_handle, _event| {
        #[cfg(all(debug_assertions, target_os = "macos"))]
        if matches!(_event, tauri::RunEvent::Ready) {
            restore_macos_bundled_icon();
        }
    });
}
