import { invoke, isTauri } from "@tauri-apps/api/core";

export interface CameraDriver {
  name: string;
  version: string;
  busInfo: string;
}

export interface CameraFrameRateRange {
  min: number;
  max: number;
}

export type CameraFrameSize =
  | {
      kind: "discrete";
      width: number;
      height: number;
      frameRates: CameraFrameRateRange[];
    }
  | {
      kind: "stepwise";
      minWidth: number;
      maxWidth: number;
      widthStep: number;
      minHeight: number;
      maxHeight: number;
      heightStep: number;
      frameRates: CameraFrameRateRange[];
    };

export interface CameraFormat {
  pixelFormat: string;
  description: string | null;
  compressed: boolean;
  emulated: boolean;
  /** Raw native format flags, when the platform exposes a bitmask. */
  flagBits: number | null;
  frameSizes: CameraFrameSize[];
  fieldOfView: number | null;
  binned: boolean | null;
  hdr: boolean | null;
  minIso: number | null;
  maxIso: number | null;
}

export interface CameraActiveFormat {
  pixelFormat: string;
  width: number;
  height: number;
  frameRate: number | null;
  stride: number | null;
  imageSize: number | null;
  fieldOrder: string | null;
  colorSpace: string | null;
  quantization: string | null;
  transferFunction: string | null;
}

export type CameraControlValue = number | boolean | string | number[] | null;

export interface CameraControlMenuItem {
  index: number;
  name: string | null;
  value: number | null;
}

export interface CameraControl {
  id: number;
  name: string;
  controlType: string;
  minimum: number;
  maximum: number;
  step: number;
  default: number;
  value: CameraControlValue | null;
  /** Raw native control flags; `flags` contains their readable names. */
  flagBits: number;
  flags: string[];
  menuItems: CameraControlMenuItem[];
}

export interface CameraDevice {
  id: string;
  name: string;
  manufacturer: string | null;
  model: string | null;
  serialNumber: string | null;
  transport: string | null;
  deviceType: string | null;
  position: "front" | "back" | "unspecified";
  connected: boolean;
  inUse: boolean | null;
  inUseBy: string[];
  suspended: boolean | null;
  driver: CameraDriver | null;
  /** Raw native capability flags, when the platform exposes a bitmask. */
  capabilityBits: number | null;
  capabilities: string[];
  formats: CameraFormat[];
  activeFormat: CameraActiveFormat | null;
  controls: CameraControl[];
}

export interface CameraAvailability {
  id: string;
  connected: boolean;
  inUse: boolean;
  inUseBy: string[];
}

export interface CameraStreamInfo {
  sessionId: number;
  deviceId: string;
  width: number;
  height: number;
  frameRate: number;
  pixelFormat: string;
}

export interface CameraRouteStatus {
  deviceId: string;
  width: number;
  height: number;
  frameRate: number;
  detail: string;
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
  preferredFrameRate: number | null;
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
  preferredFrameRate: null,
  disconnectBehavior: "blackFrame",
  virtualCameraName: "Camux Camera",
  hardwareAcceleration: "automatic",
};

let browserSettings = DEFAULT_SETTINGS;
let latestCameraDevices: CameraDevice[] = [];
let pendingCameraDevices: Promise<CameraDevice[]> | undefined;

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

/** Get camera descriptors directly from the native camera API. */
export function getDevices(): Promise<CameraDevice[]> {
  if (pendingCameraDevices) return pendingCameraDevices;

  const request = loadDevices();
  pendingCameraDevices = request;
  const clearPending = () => {
    if (pendingCameraDevices === request) pendingCameraDevices = undefined;
  };
  void request.then(clearPending, clearPending);
  return request;
}

async function loadDevices(): Promise<CameraDevice[]> {
  if (!isTauri()) {
    throw new Error("The native backend is unavailable. Start Camux with `just run`.");
  }

  latestCameraDevices = await invoke<CameraDevice[]>("get_devices");

  const defaultCamera = (await getSettings().catch(() => DEFAULT_SETTINGS)).defaultCamera;
  if (defaultCamera) {
    latestCameraDevices.sort((left, right) => Number(right.id === defaultCamera) - Number(left.id === defaultCamera));
  }

  return latestCameraDevices;
}

/** Refresh lightweight connection and process-ownership state without reopening camera streams. */
export function getCameraAvailability(deviceIds: string[]): Promise<CameraAvailability[]> {
  if (!isTauri()) return Promise.resolve([]);

  return invoke<CameraAvailability[]>("get_camera_availability", { deviceIds });
}

/** Apply a camera control immediately and return the driver's refreshed control snapshot. */
export async function setCameraControl(deviceId: string, controlId: number, value: CameraControlValue): Promise<CameraControl[]> {
  if (!isTauri()) throw new Error("Camera controls require the native Camux backend.");

  const controls = await invoke<CameraControl[]>("set_camera_control", {
    deviceId,
    controlId,
    value,
  });
  const device = latestCameraDevices.find((candidate) => candidate.id === deviceId);
  if (device) device.controls = controls;
  return controls;
}

/** Start native V4L2 capture; frames are retained in a latest-frame mailbox. */
export function startCameraStream(deviceId: string): Promise<CameraStreamInfo> {
  if (!isTauri()) return Promise.reject(new Error("Camera streaming requires the native Camux backend."));

  return invoke<CameraStreamInfo>("start_camera_stream", { deviceId });
}

/** Wait for and return the newest frame after the previous pull. */
export function nextCameraFrame(sessionId: number): Promise<ArrayBuffer> {
  if (!isTauri()) return Promise.reject(new Error("Camera streaming requires the native Camux backend."));

  return invoke<ArrayBuffer>("next_camera_frame", { sessionId });
}

/** Stop preview delivery; a camera routed to the virtual device keeps running. */
export function stopCameraPreview(sessionId: number): Promise<void> {
  if (!isTauri()) return Promise.resolve();

  return invoke("stop_camera_preview", { sessionId });
}

/** Tee an active native capture session into Camux's virtual-camera device. */
export function routeCameraToVirtual(sessionId: number): Promise<CameraRouteStatus> {
  if (!isTauri()) return Promise.reject(new Error("Virtual-camera routing requires the native Camux backend."));

  return invoke<CameraRouteStatus>("route_camera_to_virtual", { sessionId });
}

/** Stop routing the active capture session and release its physical camera when no preview remains. */
export function stopCameraRoute(): Promise<void> {
  if (!isTauri()) return Promise.resolve();

  return invoke("stop_camera_route");
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
