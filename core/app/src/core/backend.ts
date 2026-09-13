import { invoke, isTauri } from "@tauri-apps/api/core";

export interface CameraDevice {
  id: string;
  name: string;
}

/** Reveal the native window after React has committed the styled application shell. */
export function showMainWindow(): Promise<void> {
  if (!isTauri()) return Promise.resolve();

  return invoke("show_main_window");
}

/** Maximize or restore while keeping borderless macOS windows redrawing. */
export function toggleMaximizeRealtime(): Promise<void> {
  if (!isTauri()) return Promise.resolve();

  return invoke("toggle_maximize_realtime");
}

/** Get all camera devices through the native Rust backend. */
export function getDevices(): Promise<CameraDevice[]> {
  if (!isTauri()) {
    return Promise.reject(
      new Error("The native backend is unavailable. Start Camux with `just run`."),
    );
  }

  return invoke<CameraDevice[]>("get_devices");
}
