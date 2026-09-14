import { useCallback, useEffect, useState } from "react";
import { CircleAlert, CircleCheck, FolderOpen, LoaderCircle, RotateCcw, Wrench } from "lucide-react";
import { SettingsItem } from "@/components/custom/settings/item";
import { SettingsSection } from "@/components/custom/settings/section";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import {
  getDevices,
  getSettings,
  getVirtualCameraStatus,
  openLogs,
  repairVirtualCamera,
  resetSettings,
  setSettings,
  type AppSettings,
  type CameraDevice,
  type DisconnectBehavior,
  type HardwareAcceleration,
  type PreferredQuality,
  type VirtualCameraStatus,
} from "@/core/backend";
import { THEME_OPTIONS, type Theme, useTheme } from "@/lib/theme";
import { upperFirstLetter } from "@/lib/utils";

type Props = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

type PendingAction = keyof AppSettings | "load" | "logs" | "repair" | "reset";

const AUTOMATIC_CAMERA = "__automatic__";
const QUALITY_OPTIONS: ReadonlyArray<{ value: PreferredQuality; label: string }> = [
  { value: "automatic", label: "Automatic" },
  { value: "hd", label: "720p HD" },
  { value: "fullHd", label: "1080p Full HD" },
  { value: "ultraHd", label: "4K Ultra HD" },
];
const DISCONNECT_OPTIONS: ReadonlyArray<{ value: DisconnectBehavior; label: string }> = [
  { value: "blackFrame", label: "Black frame" },
  { value: "freezeLastFrame", label: "Freeze last frame" },
  { value: "testPattern", label: "Test pattern" },
];
const ACCELERATION_OPTIONS: ReadonlyArray<{ value: HardwareAcceleration; label: string }> = [
  { value: "automatic", label: "Automatic" },
  { value: "enabled", label: "Enabled" },
  { value: "disabled", label: "Disabled" },
];

const messageFrom = (reason: unknown) => (reason instanceof Error ? reason.message : String(reason));
const labelFor = <Value extends string>(options: ReadonlyArray<{ value: Value; label: string }>, value: Value) =>
  options.find((option) => option.value === value)?.label ?? value;

const SettingsSelect = <Value extends string>({
  label,
  value,
  options,
  disabled,
  onValueChange,
}: {
  label: string;
  value: Value;
  options: ReadonlyArray<{ value: Value; label: string }>;
  disabled?: boolean;
  onValueChange: (value: Value) => void;
}) => (
  <Select value={value} disabled={disabled} onValueChange={(next) => onValueChange(next as Value)}>
    <SelectTrigger aria-label={label} size="sm" className="min-w-40 justify-between">
      <SelectValue>{labelFor(options, value)}</SelectValue>
    </SelectTrigger>
    <SelectContent side="bottom" align="end">
      {options.map((option) => (
        <SelectItem key={option.value} value={option.value}>
          {option.label}
        </SelectItem>
      ))}
    </SelectContent>
  </Select>
);

export const SettingsDialog = ({ open, onOpenChange }: Props) => {
  const { setTheme } = useTheme();
  const [settings, setLocalSettings] = useState<AppSettings>();
  const [devices, setDevices] = useState<CameraDevice[]>([]);
  const [virtualCamera, setVirtualCamera] = useState<VirtualCameraStatus>();
  const [pending, setPending] = useState<PendingAction>();
  const [error, setError] = useState<string>();

  const load = useCallback(async () => {
    setPending("load");
    setError(undefined);

    try {
      const [nextSettings, nextDevices] = await Promise.all([getSettings(), getDevices().catch(() => [] as CameraDevice[])]);
      setLocalSettings(nextSettings);
      setDevices(nextDevices);
      setTheme(nextSettings.appearance);
      setVirtualCamera(await getVirtualCameraStatus());
    } catch (reason) {
      setError(messageFrom(reason));
      try {
        setLocalSettings(await getSettings());
      } catch {
        // Preserve the current UI if the backend cannot be reached for recovery.
      }
    } finally {
      setPending(undefined);
    }
  }, [setTheme]);

  useEffect(() => {
    // oxlint-disable-next-line react/set-state-in-effect -- prefetch native settings while the dialog is closed
    void load();
  }, [load]);

  const commit = async <Key extends keyof AppSettings>(key: Key, value: AppSettings[Key]) => {
    if (!settings) return;

    const nextSettings = { ...settings, [key]: value };
    setPending(key);
    setError(undefined);
    setLocalSettings(nextSettings);
    try {
      const saved = await setSettings(nextSettings);
      setLocalSettings(saved);
      if (key === "appearance") setTheme(saved.appearance);
      if (key === "virtualCameraName") setVirtualCamera(await getVirtualCameraStatus());
    } catch (reason) {
      setError(messageFrom(reason));
      try {
        setLocalSettings(await getSettings());
      } catch {
        // Preserve the current UI if the backend cannot be reached for recovery.
      }
    } finally {
      setPending(undefined);
    }
  };

  const handleRepair = async () => {
    if (!settings) return;

    setPending("repair");
    setError(undefined);
    try {
      const saved = await setSettings(settings);
      setLocalSettings(saved);
      setVirtualCamera(await repairVirtualCamera());
    } catch (reason) {
      setError(messageFrom(reason));
      try {
        setLocalSettings(await getSettings());
      } catch {
        // Preserve the current UI if the backend cannot be reached for recovery.
      }
    } finally {
      setPending(undefined);
    }
  };

  const handleOpenLogs = async () => {
    setPending("logs");
    setError(undefined);
    try {
      await openLogs();
    } catch (reason) {
      setError(messageFrom(reason));
    } finally {
      setPending(undefined);
    }
  };

  const handleReset = async () => {
    setPending("reset");
    setError(undefined);
    try {
      const defaults = await resetSettings();
      setLocalSettings(defaults);
      setTheme(defaults.appearance);
      setVirtualCamera(await getVirtualCameraStatus());
    } catch (reason) {
      setError(messageFrom(reason));
    } finally {
      setPending(undefined);
    }
  };

  const isDisabled = (action: PendingAction) => pending === action || pending === "repair" || pending === "reset";
  const busy = pending !== undefined;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className="grid-rows-[auto_minmax(0,1fr)] gap-0 overflow-hidden p-0"
        style={{ maxHeight: "calc(100vh - 2rem)", maxWidth: "42rem" }}
      >
        <DialogHeader className="border-b px-6 py-5 pr-16">
          <DialogTitle className="text-lg">Settings</DialogTitle>
          <DialogDescription>Adjust Camux&apos;s settings</DialogDescription>
        </DialogHeader>

        <div className="min-h-0 overflow-y-auto px-6 py-5 [scrollbar-color:var(--border)_transparent]">
          {pending === "load" && !settings ? (
            <div className="flex min-h-72 items-center justify-center gap-2 text-sm text-muted-foreground">
              <LoaderCircle className="size-4 animate-spin" aria-hidden="true" />
              Loading settings
            </div>
          ) : settings ? (
            <div className="flex w-full flex-col gap-7">
              <SettingsSection title="GENERAL">
                <SettingsItem title="Appearance">
                  <SettingsSelect
                    label="Appearance"
                    value={settings.appearance}
                    options={THEME_OPTIONS.map((value) => ({ value, label: upperFirstLetter(value) }))}
                    disabled={isDisabled("appearance")}
                    onValueChange={(value: Theme) => void commit("appearance", value)}
                  />
                </SettingsItem>
                <SettingsItem title="Launch Camux at login">
                  <Switch
                    aria-label="Launch Camux at login"
                    checked={settings.launchAtLogin}
                    disabled={isDisabled("launchAtLogin")}
                    onCheckedChange={(checked) => void commit("launchAtLogin", checked)}
                  />
                </SettingsItem>
                <SettingsItem title="Keep Camux running when closed">
                  <Switch
                    aria-label="Keep Camux running when closed"
                    checked={settings.keepRunningWhenClosed}
                    disabled={isDisabled("keepRunningWhenClosed")}
                    onCheckedChange={(checked) => void commit("keepRunningWhenClosed", checked)}
                  />
                </SettingsItem>
                <SettingsItem title="Restore previous session">
                  <Switch
                    aria-label="Restore previous session"
                    checked={settings.restorePreviousSession}
                    disabled={isDisabled("restorePreviousSession")}
                    onCheckedChange={(checked) => void commit("restorePreviousSession", checked)}
                  />
                </SettingsItem>
              </SettingsSection>

              <SettingsSection title="CAMERA">
                <SettingsItem title="Default camera">
                  <Select
                    value={settings.defaultCamera ?? AUTOMATIC_CAMERA}
                    disabled={isDisabled("defaultCamera")}
                    onValueChange={(value) => void commit("defaultCamera", value === AUTOMATIC_CAMERA ? null : value)}
                  >
                    <SelectTrigger aria-label="Default camera" size="sm" className="max-w-56 min-w-40 justify-between">
                      <SelectValue>
                        {settings.defaultCamera
                          ? (devices.find((device) => device.id === settings.defaultCamera)?.name ?? "Unavailable camera")
                          : "Automatic"}
                      </SelectValue>
                    </SelectTrigger>
                    <SelectContent side="bottom" align="end">
                      <SelectItem value={AUTOMATIC_CAMERA}>Automatic</SelectItem>
                      {devices.map((device) => (
                        <SelectItem key={device.id} value={device.id}>
                          {device.name}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </SettingsItem>
                <SettingsItem title="Preferred quality">
                  <SettingsSelect
                    label="Preferred quality"
                    value={settings.preferredQuality}
                    options={QUALITY_OPTIONS}
                    disabled={isDisabled("preferredQuality")}
                    onValueChange={(value) => void commit("preferredQuality", value)}
                  />
                </SettingsItem>
                <SettingsItem title="When camera disconnects">
                  <SettingsSelect
                    label="When camera disconnects"
                    value={settings.disconnectBehavior}
                    options={DISCONNECT_OPTIONS}
                    disabled={isDisabled("disconnectBehavior")}
                    onValueChange={(value) => void commit("disconnectBehavior", value)}
                  />
                </SettingsItem>
              </SettingsSection>

              <SettingsSection title="VIRTUAL CAMERA">
                <SettingsItem title={settings.virtualCameraName || "Camux Camera"}>
                  <span
                    aria-label={`Virtual camera status: ${virtualCamera?.detail ?? "Checking"}`}
                    title={virtualCamera?.detail}
                    className={
                      virtualCamera?.state === "ready"
                        ? "inline-flex items-center gap-1.5 text-xs font-medium text-emerald-600 dark:text-emerald-400"
                        : "inline-flex items-center gap-1.5 text-xs font-medium text-amber-600 dark:text-amber-400"
                    }
                  >
                    {virtualCamera?.state === "ready" ? (
                      <CircleCheck className="size-3.5" aria-hidden="true" />
                    ) : (
                      <CircleAlert className="size-3.5" aria-hidden="true" />
                    )}
                    {virtualCamera?.state === "ready"
                      ? "Ready"
                      : virtualCamera?.state === "needsRepair"
                        ? "Needs repair"
                        : virtualCamera?.state === "notInstalled"
                          ? "Not installed"
                          : "Checking"}
                  </span>
                </SettingsItem>
                <SettingsItem title="Name">
                  <div className="flex items-center gap-2">
                    <input
                      aria-label="Virtual camera name"
                      className="h-8 w-44 rounded-3xl border border-input bg-input/50 px-3 text-sm outline-none transition-[border-color,box-shadow] focus:border-ring focus:ring-3 focus:ring-ring/30 disabled:opacity-50"
                      value={settings.virtualCameraName}
                      disabled={isDisabled("virtualCameraName")}
                      maxLength={64}
                      onChange={(event) =>
                        setLocalSettings((current) => (current ? { ...current, virtualCameraName: event.target.value } : current))
                      }
                      onBlur={(event) => void commit("virtualCameraName", event.target.value)}
                      onKeyDown={(event) => {
                        if (event.key === "Enter") event.currentTarget.blur();
                      }}
                    />
                    <Button
                      variant="secondary"
                      size="sm"
                      disabled={busy}
                      onMouseDown={(event) => event.preventDefault()}
                      onClick={() => void handleRepair()}
                    >
                      {pending === "repair" ? <LoaderCircle className="animate-spin" aria-hidden="true" /> : <Wrench aria-hidden="true" />}
                      Repair
                    </Button>
                  </div>
                </SettingsItem>
              </SettingsSection>

              <SettingsSection title="ADVANCED">
                <SettingsItem title="Hardware acceleration">
                  <SettingsSelect
                    label="Hardware acceleration"
                    value={settings.hardwareAcceleration}
                    options={ACCELERATION_OPTIONS}
                    disabled={isDisabled("hardwareAcceleration")}
                    onValueChange={(value) => void commit("hardwareAcceleration", value)}
                  />
                </SettingsItem>
                <SettingsItem title="Diagnostic logs">
                  <Button variant="secondary" size="sm" disabled={busy} onClick={() => void handleOpenLogs()}>
                    {pending === "logs" ? <LoaderCircle className="animate-spin" aria-hidden="true" /> : <FolderOpen aria-hidden="true" />}
                    Open logs
                  </Button>
                </SettingsItem>
              </SettingsSection>

              {error && (
                <div className="flex items-start gap-2 rounded-xl bg-destructive/10 px-3 py-2 text-xs text-destructive" role="alert">
                  <CircleAlert className="mt-0.5 size-3.5 shrink-0" aria-hidden="true" />
                  <span>{error}</span>
                </div>
              )}

              <div className="flex justify-end border-t pt-5">
                <Button variant="destructive" size="sm" disabled={busy} onClick={() => void handleReset()}>
                  {pending === "reset" ? <LoaderCircle className="animate-spin" aria-hidden="true" /> : <RotateCcw aria-hidden="true" />}
                  Reset settings
                </Button>
              </div>
            </div>
          ) : (
            <div className="flex min-h-72 flex-col items-center justify-center gap-3 text-center">
              <CircleAlert className="size-6 text-destructive" aria-hidden="true" />
              <p className="max-w-sm text-sm text-muted-foreground">{error ?? "Settings could not be loaded."}</p>
              <Button variant="secondary" size="sm" onClick={() => void load()}>
                Try again
              </Button>
            </div>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
};
