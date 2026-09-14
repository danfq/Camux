mod platform;
mod settings;

use platform::{CameraBackend, CameraDevice, PlatformBackend};
use settings::{AppSettings, SettingsState, VirtualCameraStatus};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use tauri::Manager;
use tauri::webview::PageLoadEvent;

#[cfg(target_os = "macos")]
static RESTORED_WINDOW_FRAME: Mutex<Option<[f64; 4]>> = Mutex::new(None);

#[cfg(target_os = "macos")]
static WINDOW_FRAME_ANIMATION_ACTIVE: AtomicBool = AtomicBool::new(false);

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

        // WKWebView redraws its own backing layers as its viewport changes. The
        // default AppKit live-resize optimization instead preserves the previous
        // frame, which leaves the old-sized UI visible until resizing settles.
        ns_window.setPreservesContentDuringLiveResize(false);

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
fn get_settings(state: tauri::State<'_, SettingsState>) -> Result<AppSettings, String> {
    state.snapshot()
}

#[tauri::command]
fn set_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, SettingsState>,
    settings: AppSettings,
) -> Result<AppSettings, String> {
    state.replace(&app, settings)
}

#[tauri::command]
fn reset_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, SettingsState>,
) -> Result<AppSettings, String> {
    state.replace(&app, AppSettings::default())
}

#[tauri::command]
fn get_virtual_camera_status(
    app: tauri::AppHandle,
    state: tauri::State<'_, SettingsState>,
) -> Result<VirtualCameraStatus, String> {
    let settings = state.snapshot()?;
    settings::virtual_camera_status(&app, &settings.virtual_camera_name)
}

#[tauri::command]
fn repair_virtual_camera(
    app: tauri::AppHandle,
    state: tauri::State<'_, SettingsState>,
) -> Result<VirtualCameraStatus, String> {
    let settings = state.snapshot()?;
    settings::repair_virtual_camera(&app, &settings.virtual_camera_name)
}

#[tauri::command]
fn open_logs(app: tauri::AppHandle) -> Result<(), String> {
    settings::open_logs(&app)
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

#[cfg(target_os = "macos")]
fn animate_macos_window_frame(
    window: tauri::WebviewWindow,
    from: [f64; 4],
    to: [f64; 4],
    duration_seconds: f64,
) {
    use std::time::{Duration, Instant};

    // Updating the actual frame at display cadence makes AppKit resize the
    // WKWebView on every step. NSWindow's built-in animated setFrame instead
    // animates a cached surface and only gives WKWebView the final viewport.
    const FRAMES_PER_SECOND: f64 = 60.0;
    let step_count = (duration_seconds * FRAMES_PER_SECOND).ceil().max(1.0) as u32;
    let duration = Duration::from_secs_f64(duration_seconds);

    std::thread::spawn(move || {
        let started = Instant::now();

        for step in 1..=step_count {
            let deadline = started + duration.mul_f64(step as f64 / step_count as f64);
            std::thread::sleep(deadline.saturating_duration_since(Instant::now()));

            let progress = step as f64 / step_count as f64;
            let eased = 0.5 - (std::f64::consts::PI * progress).cos() / 2.0;
            let frame = [
                from[0] + (to[0] - from[0]) * eased,
                from[1] + (to[1] - from[1]) * eased,
                from[2] + (to[2] - from[2]) * eased,
                from[3] + (to[3] - from[3]) * eased,
            ];
            let final_step = step == step_count;

            if window
                .with_webview(move |webview| unsafe {
                    use objc2_app_kit::{NSView, NSWindow};
                    use objc2_foundation::{NSPoint, NSRect, NSSize};

                    let ns_window = &*webview.ns_window().cast::<NSWindow>();
                    let webview = &*webview.inner().cast::<NSView>();
                    let frame = NSRect::new(
                        NSPoint::new(frame[0], frame[1]),
                        NSSize::new(frame[2], frame[3]),
                    );

                    ns_window.setFrame_display(frame, true);

                    // Keep Wry's intermediate container and WKWebView locked to
                    // their current bounds before the next animation step.
                    if let Some(container) = webview.superview() {
                        if let Some(parent) = container.superview() {
                            container.setFrame(parent.bounds());
                        }
                        webview.setFrame(container.bounds());
                        container.layoutSubtreeIfNeeded();
                    }
                    webview.setNeedsDisplay(true);
                    ns_window.displayIfNeeded();

                    if final_step {
                        WINDOW_FRAME_ANIMATION_ACTIVE.store(false, Ordering::Release);
                    }
                })
                .is_err()
            {
                WINDOW_FRAME_ANIMATION_ACTIVE.store(false, Ordering::Release);
                return;
            }
        }
    });
}

#[tauri::command]
fn toggle_maximize_realtime(window: tauri::WebviewWindow) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        if WINDOW_FRAME_ANIMATION_ACTIVE.swap(true, Ordering::AcqRel) {
            return Ok(());
        }

        let animation_window = window.clone();
        if let Err(error) = window.with_webview(move |webview| unsafe {
            use objc2_app_kit::NSWindow;
            use objc2_foundation::{NSPoint, NSRect, NSSize};

            let ns_window = &*webview.ns_window().cast::<NSWindow>();
            let Some(screen) = ns_window.screen() else {
                WINDOW_FRAME_ANIMATION_ACTIVE.store(false, Ordering::Release);
                return;
            };

            let current = ns_window.frame();
            let visible = screen.visibleFrame();
            let is_maximized = (current.origin.x - visible.origin.x).abs() < 1.0
                && (current.origin.y - visible.origin.y).abs() < 1.0
                && (current.size.width - visible.size.width).abs() < 1.0
                && (current.size.height - visible.size.height).abs() < 1.0;

            let target = if is_maximized {
                RESTORED_WINDOW_FRAME
                    .lock()
                    .expect("restored window frame lock poisoned")
                    .take()
                    .map(|[x, y, width, height]| {
                        NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
                    })
                    .unwrap_or(current)
            } else {
                *RESTORED_WINDOW_FRAME
                    .lock()
                    .expect("restored window frame lock poisoned") = Some([
                    current.origin.x,
                    current.origin.y,
                    current.size.width,
                    current.size.height,
                ]);
                visible
            };

            let target = [
                target.origin.x,
                target.origin.y,
                target.size.width,
                target.size.height,
            ];
            let duration = ns_window.animationResizeTime(NSRect::new(
                NSPoint::new(target[0], target[1]),
                NSSize::new(target[2], target[3]),
            ));

            if duration <= f64::EPSILON {
                ns_window.setFrame_display(
                    NSRect::new(
                        NSPoint::new(target[0], target[1]),
                        NSSize::new(target[2], target[3]),
                    ),
                    true,
                );
                WINDOW_FRAME_ANIMATION_ACTIVE.store(false, Ordering::Release);
                return;
            }

            animate_macos_window_frame(
                animation_window,
                [
                    current.origin.x,
                    current.origin.y,
                    current.size.width,
                    current.size.height,
                ],
                target,
                duration,
            );
        }) {
            WINDOW_FRAME_ANIMATION_ACTIVE.store(false, Ordering::Release);
            return Err(error.to_string());
        }

        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        window.toggle_maximize().map_err(|error| error.to_string())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app =
        tauri::Builder::default()
            .plugin(tauri_plugin_os::init())
            .setup(|app| {
                let settings = SettingsState::load(app.handle()).map_err(std::io::Error::other)?;
                app.manage(settings);

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
            .invoke_handler(tauri::generate_handler![
                get_devices,
                get_settings,
                set_settings,
                reset_settings,
                get_virtual_camera_status,
                repair_virtual_camera,
                open_logs,
                show_main_window,
                toggle_maximize_realtime
            ])
            .build(tauri::generate_context!())
            .expect("failed to run Camux");

    app.run(|app_handle, event| {
        #[cfg(all(debug_assertions, target_os = "macos"))]
        if matches!(event, tauri::RunEvent::Ready) {
            restore_macos_bundled_icon();
        }

        if let tauri::RunEvent::WindowEvent {
            label,
            event: tauri::WindowEvent::CloseRequested { api, .. },
            ..
        } = &event
        {
            let keep_running = app_handle
                .state::<SettingsState>()
                .snapshot()
                .is_ok_and(|settings| settings.keep_running_when_closed);
            if keep_running {
                api.prevent_close();
                if let Some(window) = app_handle.get_webview_window(label) {
                    let _ = window.hide();
                }
            } else {
                app_handle.exit(0);
            }
        }

        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Reopen { .. } = event
            && let Some(window) = app_handle.get_webview_window("main")
        {
            let _ = window.show();
            let _ = window.set_focus();
        }
    });
}
