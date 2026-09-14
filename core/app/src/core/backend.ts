import { invoke, isTauri } from "@tauri-apps/api/core";

export interface CameraDevice {
  id: string;
  name: string;
}

export type Appearance = "dark" | "light" | "system";
export type PreferredQuality = "automatic" | "hd" | "fullHd" | "ultraHd";
export type DisconnectBehavior = "blackFrame" | "freezeLastFrame" | "testPattern";
export type HardwareAcceleration = "automatic" | "enabled" | "disabled";

export interface AppSettings {
  appearance: Appearance;
  launchAtLogin: boolean;
  keepRunningWhenClosed: boolean;
  restorePreviousSession: boolean;
  defaultCamera: string | null;
  preferredQuality: PreferredQuality;
  disconnectBehavior: DisconnectBehavior;
  virtualCameraName: string;
  hardwareAcceleration: HardwareAcceleration;
}

export interface VirtualCameraStatus {
  state: "ready" | "needsRepair" | "notInstalled";
  detail: string;
}

export const DEFAULT_SETTINGS: AppSettings = {
  appearance: "system",
  launchAtLogin: false,
  keepRunningWhenClosed: true,
  restorePreviousSession: true,
  defaultCamera: null,
  preferredQuality: "automatic",
  disconnectBehavior: "blackFrame",
  virtualCameraName: "Camux Camera",
  hardwareAcceleration: "automatic",
};

let browserSettings = DEFAULT_SETTINGS;

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

/** Read the native, persisted application settings. */
export function getSettings(): Promise<AppSettings> {
  if (!isTauri()) return Promise.resolve(browserSettings);

  return invoke<AppSettings>("get_settings");
}

/** Validate, apply, and persist the complete application settings. */
export function setSettings(settings: AppSettings): Promise<AppSettings> {
  if (!isTauri()) {
    browserSettings = settings;
    return Promise.resolve(settings);
  }

  return invoke<AppSettings>("set_settings", { settings });
}

/** Restore defaults and undo system integrations such as launch at login. */
export function resetSettings(): Promise<AppSettings> {
  if (!isTauri()) {
    browserSettings = DEFAULT_SETTINGS;
    return Promise.resolve(DEFAULT_SETTINGS);
  }

  return invoke<AppSettings>("reset_settings");
}

/** Inspect whether the OS-visible Camux virtual camera is healthy. */
export function getVirtualCameraStatus(): Promise<VirtualCameraStatus> {
  if (!isTauri()) {
    return Promise.resolve({ state: "notInstalled", detail: "Available in the desktop app" });
  }

  return invoke<VirtualCameraStatus>("get_virtual_camera_status");
}

/** Install or repair the OS virtual-camera integration. */
export function repairVirtualCamera(): Promise<VirtualCameraStatus> {
  if (!isTauri()) {
    return Promise.reject(new Error("Virtual-camera repair is available in the desktop app."));
  }

  return invoke<VirtualCameraStatus>("repair_virtual_camera");
}

/** Reveal Camux's native log directory in the platform file manager. */
export function openLogs(): Promise<void> {
  if (!isTauri()) return Promise.reject(new Error("Logs are available in the desktop app."));

  return invoke("open_logs");
}
