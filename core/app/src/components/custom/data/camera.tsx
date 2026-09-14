import {
  nextCameraFrame,
  routeCameraToVirtual,
  setCameraControl,
  startCameraStream,
  stopCameraPreview,
  stopCameraRoute,
  type CameraControlValue,
  type CameraDevice,
  type CameraStreamInfo,
} from "@/core/backend";
import { Camera, CameraOff, LoaderCircle, Minus, Plus } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Pagination, PaginationContent, PaginationItem, PaginationNext, PaginationPrevious } from "@/components/ui/pagination";
import { Tabs as TabsPrimitive } from "radix-ui";
import { useCallback, useEffect, useRef, useState } from "react";
import { StatusIndicator } from "@/components/custom/data/status-indicator";
import type { CameraStatusKind } from "@/components/custom/data/status-indicator";
import { Input } from "@/components/ui/input";
import { parseError } from "@/lib/utils";

const normalizeControlName = (name: string) =>
  name
    .trim()
    .replace(/([a-z\d])([A-Z])/g, "$1 $2")
    .replace(/([A-Z]+)([A-Z][a-z])/g, "$1 $2")
    .replace(/[_-]+/g, " ")
    .replace(/\s+/g, " ");

type CameraControl = CameraDevice["controls"][number];
const controlsPerPage = 2;
let routedCameraId: string | undefined;
const routeListeners = new Set<(deviceId: string | undefined) => void>();

const announceRoutedCamera = (deviceId: string | undefined) => {
  routedCameraId = deviceId;
  routeListeners.forEach((listener) => listener(deviceId));
};

const cameraStatus = (device: CameraDevice): { kind: CameraStatusKind; label: string } => {
  if (!device.connected) return { kind: "disconnected", label: "Disconnected" };
  if (device.inUse) {
    return {
      kind: "inUse",
      label: device.inUseBy.length > 0 ? `Connected · In use by ${device.inUseBy.join(", ")}` : "Connected · In use by another app",
    };
  }
  return { kind: "available", label: "Connected · Available" };
};

const permanentlyBlockedFlags = new Set(["disabled", "readOnly"]);
const blockedControlFlags = new Set([...permanentlyBlockedFlags, "grabbed", "inactive"]);
const editableControlTypes = new Set(["Integer", "Integer64", "Boolean", "Menu", "IntegerMenu", "Button", "String", "Bitmask"]);
const rangedControlTypes = new Set(["Integer", "Integer64", "Bitmask"]);

const controlAllowsChanges = (control: CameraControl) => {
  if (!editableControlTypes.has(control.controlType)) return false;
  if (control.flags.some((flag) => permanentlyBlockedFlags.has(flag))) return false;
  if (rangedControlTypes.has(control.controlType) && control.minimum >= control.maximum) return false;
  if ((control.controlType === "Menu" || control.controlType === "IntegerMenu") && control.menuItems.length < 2) {
    return false;
  }
  return true;
};

const groupCameraControls = (controls: CameraControl[]) => {
  const groups: { id: string; name: string; controls: CameraControl[] }[] = [];
  let currentGroup: (typeof groups)[number] | undefined;

  controls.forEach((control) => {
    if (control.controlType === "CtrlClass") {
      currentGroup = { id: String(control.id), name: normalizeControlName(control.name), controls: [] };
      groups.push(currentGroup);
      return;
    }

    if (!controlAllowsChanges(control)) return;

    if (!currentGroup) {
      currentGroup = { id: "controls", name: "Controls", controls: [] };
      groups.push(currentGroup);
    }
    currentGroup.controls.push(control);
  });

  return groups.filter((group) => group.controls.length > 0);
};

const displayControlValue = (control: CameraDevice["controls"][number]) => {
  if (control.value === null) return control.controlType === "Button" ? "Action" : "Value not reported";
  if (typeof control.value === "boolean") return control.value ? "Enabled" : "Disabled";

  if (typeof control.value === "number") {
    const menuItem = control.menuItems.find((item) => item.value === control.value || item.index === control.value);
    if (menuItem?.name) return normalizeControlName(menuItem.name);
    return control.value.toLocaleString();
  }

  if (Array.isArray(control.value)) return control.value.join(", ");
  return normalizeControlName(control.value);
};

const ControlEditor = ({
  control,
  error,
  onChange,
}: {
  control: CameraControl;
  error?: string;
  onChange: (value: CameraControlValue, throttle?: boolean) => void;
}) => {
  const name = normalizeControlName(control.name);
  const blocked = control.flags.some((flag) => blockedControlFlags.has(flag));
  const editable = editableControlTypes.has(control.controlType) && !blocked;
  const numericValue = typeof control.value === "number" ? control.value : control.default;
  const step = Math.max(control.step, 1);
  const isSlider = control.controlType === "Integer" || control.controlType === "Integer64";

  if (isSlider) {
    const changeBy = (direction: -1 | 1) => {
      const nextValue = Math.min(control.maximum, Math.max(control.minimum, numericValue + direction * step));
      onChange(nextValue);
    };

    return (
      <div className="min-w-0 rounded-xl border bg-muted/30 px-3 py-2.5">
        <div className="mb-2 flex items-center justify-between gap-3">
          <span className="min-w-0 truncate text-xs text-muted-foreground" title={name}>
            {name}
          </span>
          <div className="flex shrink-0 items-center gap-1">
            <Button
              type="button"
              variant="outline"
              size="icon-xs"
              aria-label={`Decrease ${name} by ${step}`}
              disabled={!editable || numericValue <= control.minimum}
              onClick={() => changeBy(-1)}
            >
              <Minus aria-hidden="true" />
            </Button>
            <Input
              aria-label={`${name} value`}
              className="h-7 w-20 rounded-lg border bg-background px-2 text-center font-mono text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring/30 disabled:opacity-50"
              type="number"
              min={control.minimum}
              max={control.maximum}
              step={step}
              value={numericValue}
              disabled={!editable}
              onChange={(event) => {
                if (event.target.value === "") return;
                const value = Number(event.target.value);
                if (Number.isFinite(value)) {
                  const steppedValue = control.minimum + Math.round((value - control.minimum) / step) * step;
                  onChange(Math.min(control.maximum, Math.max(control.minimum, steppedValue)), true);
                }
              }}
            />
            <Button
              type="button"
              variant="outline"
              size="icon-xs"
              aria-label={`Increase ${name} by ${step}`}
              disabled={!editable || numericValue >= control.maximum}
              onClick={() => changeBy(1)}
            >
              <Plus aria-hidden="true" />
            </Button>
          </div>
        </div>
        <Input
          aria-label={`${name} slider`}
          className="h-5 w-full accent-primary disabled:opacity-50"
          type="range"
          min={control.minimum}
          max={control.maximum}
          step={step}
          value={numericValue}
          disabled={!editable}
          onChange={(event) => onChange(Number(event.target.value), true)}
        />
        <div className="mt-1 flex items-center justify-between gap-3 text-[10px] text-muted-foreground">
          <span>Min {control.minimum.toLocaleString()}</span>
          <span>Step {step.toLocaleString()}</span>
          <span>Max {control.maximum.toLocaleString()}</span>
        </div>
        {blocked && <div className="mt-1 text-[10px] text-muted-foreground">Not currently editable</div>}
        {error && <div className="mt-1 text-[10px] text-destructive">{error}</div>}
      </div>
    );
  }

  let editor: React.ReactNode;
  if (!editable) {
    editor = <span className="text-sm font-medium">{displayControlValue(control)}</span>;
  } else if (control.controlType === "Boolean") {
    editor = <Switch aria-label={name} checked={control.value === true} onCheckedChange={(checked) => onChange(checked)} />;
  } else if (control.controlType === "Menu" || control.controlType === "IntegerMenu") {
    editor = (
      <Select value={String(numericValue)} onValueChange={(value) => onChange(Number(value))}>
        <SelectTrigger size="sm" className="w-full bg-background">
          <SelectValue />
        </SelectTrigger>
        <SelectContent position="popper" align="end">
          {control.menuItems.map((item) => (
            <SelectItem key={item.index} value={String(item.index)}>
              {item.name ? normalizeControlName(item.name) : (item.value?.toLocaleString() ?? item.index.toLocaleString())}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    );
  } else if (control.controlType === "Button") {
    editor = (
      <Button type="button" variant="secondary" size="sm" onClick={() => onChange(null)}>
        Run
      </Button>
    );
  } else if (control.controlType === "String") {
    editor = (
      <Input
        aria-label={name}
        className="h-8 w-full rounded-lg border bg-background px-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring/30"
        type="text"
        value={typeof control.value === "string" ? control.value : ""}
        onChange={(event) => onChange(event.target.value, true)}
      />
    );
  } else {
    editor = (
      <Input
        aria-label={name}
        className="h-8 w-full rounded-lg border bg-background px-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring/30"
        type="number"
        min={control.minimum}
        max={control.maximum}
        step={step}
        value={numericValue}
        onChange={(event) => onChange(Number(event.target.value), true)}
      />
    );
  }

  return (
    <div className="min-w-0 rounded-xl border bg-muted/30 px-3 py-2.5">
      <div className="mb-2 flex min-h-5 items-center justify-between gap-3">
        <span className="truncate text-xs text-muted-foreground" title={name}>
          {name}
        </span>
        {control.controlType === "Boolean" && <span>{editor}</span>}
      </div>
      {control.controlType !== "Boolean" && <div>{editor}</div>}
      {control.controlType === "Bitmask" && (
        <div className="mt-1 flex items-center justify-between gap-3 text-[10px] text-muted-foreground">
          <span>Min {control.minimum.toLocaleString()}</span>
          <span>Step {step.toLocaleString()}</span>
          <span>Max {control.maximum.toLocaleString()}</span>
        </div>
      )}
      {blocked && <div className="mt-1 text-[10px] text-muted-foreground">Not currently editable</div>}
      {error && <div className="mt-1 text-[10px] text-destructive">{error}</div>}
    </div>
  );
};

export const CameraItem = ({ device }: { device: CameraDevice }) => {
  // details state
  const [detailsOpen, setDetailsOpen] = useState(false);
  const [controls, setControls] = useState(device.controls);
  const [controlErrors, setControlErrors] = useState<Record<number, string>>({});
  const [controlPages, setControlPages] = useState<Record<string, number>>({});
  const [streamLoading, setStreamLoading] = useState(false);
  const [streamError, setStreamError] = useState<string>();
  const updateTimers = useRef(new Map<number, ReturnType<typeof setTimeout>>());
  const updateRevisions = useRef(new Map<number, number>());
  const confirmedControls = useRef(device.controls);
  const queuedUpdates = useRef(
    new Map<
      number,
      {
        control: CameraControl;
        value: CameraControlValue;
        revision: number;
        throttle: boolean;
      }
    >(),
  );
  const activeUpdates = useRef(new Set<number>());
  const mounted = useRef(true);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const [canvasMounted, setCanvasMounted] = useState(false);
  const [streamInfo, setStreamInfo] = useState<CameraStreamInfo>();
  const [routeLoading, setRouteLoading] = useState(false);
  const [routeActive, setRouteActive] = useState(false);
  const [routeError, setRouteError] = useState<string>();
  const controlGroups = groupCameraControls(controls);
  const availability = cameraStatus(device);

  const attachPreviewCanvas = useCallback((canvas: HTMLCanvasElement | null) => {
    canvasRef.current = canvas;
    setCanvasMounted(canvas !== null);
  }, []);

  useEffect(() => {
    const timers = updateTimers.current;
    const queue = queuedUpdates.current;
    mounted.current = true;
    return () => {
      mounted.current = false;
      queue.clear();
      timers.forEach((timer) => clearTimeout(timer));
    };
  }, []);

  useEffect(() => {
    const updateRouteState = (deviceId: string | undefined) => setRouteActive(deviceId === device.id);
    routeListeners.add(updateRouteState);
    updateRouteState(routedCameraId);
    return () => {
      routeListeners.delete(updateRouteState);
    };
  }, [device.id]);

  const flushControlUpdate = async (controlId: number) => {
    if (activeUpdates.current.has(controlId)) return;
    const update = queuedUpdates.current.get(controlId);
    if (!update) return;

    queuedUpdates.current.delete(controlId);
    activeUpdates.current.add(controlId);

    try {
      const refreshedControls = await setCameraControl(device.id, update.control.id, update.value);
      confirmedControls.current = refreshedControls;
      if (mounted.current && updateRevisions.current.get(controlId) === update.revision) {
        setControls(refreshedControls);
      }
    } catch (reason) {
      if (!mounted.current || updateRevisions.current.get(controlId) !== update.revision) return;
      setControls(confirmedControls.current);
      setControlErrors((current) => ({
        ...current,
        [controlId]: reason instanceof Error ? reason.message : String(reason),
      }));
    } finally {
      activeUpdates.current.delete(controlId);
      if (mounted.current) {
        const delay = update.throttle ? 60 : 0;
        updateTimers.current.set(
          controlId,
          setTimeout(() => {
            updateTimers.current.delete(controlId);
            void flushControlUpdate(controlId);
          }, delay),
        );
      }
    }
  };

  const updateControl = (control: CameraControl, value: CameraControlValue, throttle = false) => {
    const revision = (updateRevisions.current.get(control.id) ?? 0) + 1;
    updateRevisions.current.set(control.id, revision);
    setControls((current) => current.map((item) => (item.id === control.id ? { ...item, value } : item)));
    setControlErrors((current) => {
      const next = { ...current };
      delete next[control.id];
      return next;
    });

    queuedUpdates.current.set(control.id, { control, value, revision, throttle });
    if (!activeUpdates.current.has(control.id) && !updateTimers.current.has(control.id)) {
      void flushControlUpdate(control.id);
    }
  };

  // Native V4L2 capture avoids WebKit's slow PipeWire startup and delivers each
  // MJPEG image as one complete binary frame, preventing partial-frame tearing.
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!detailsOpen || !canvasMounted || !canvas) return;

    let cancelled = false;
    let sessionId: number | undefined;
    let decodeFailures = 0;
    let firstFrame = true;
    setStreamLoading(true);
    setStreamError(undefined);
    setStreamInfo(undefined);

    const openStream = async () => {
      try {
        const info = await startCameraStream(device.id);
        if (cancelled) {
          await stopCameraPreview(info.sessionId);
          return;
        }
        sessionId = info.sessionId;
        setStreamInfo(info);

        // Pull only after the previous frame has been decoded and drawn. Rust
        // retains one latest frame, so neither side can accumulate a frame queue.
        while (!cancelled) {
          const frame = await nextCameraFrame(info.sessionId);
          if (cancelled) break;
          try {
            const bitmap = await createImageBitmap(new Blob([frame], { type: "image/jpeg" }));
            if (cancelled) {
              bitmap.close();
              break;
            }
            if (canvas.width !== bitmap.width || canvas.height !== bitmap.height) {
              canvas.width = bitmap.width;
              canvas.height = bitmap.height;
            }
            const context = canvas.getContext("2d", { alpha: false });
            if (!context) throw new Error("The preview canvas is unavailable.");
            context.drawImage(bitmap, 0, 0);
            bitmap.close();
            decodeFailures = 0;
            setStreamError(undefined);
            if (firstFrame) {
              firstFrame = false;
              setStreamLoading(false);
            }
          } catch {
            decodeFailures += 1;
            if (decodeFailures >= 3) {
              setStreamLoading(false);
              setStreamError("The camera returned JPEG frames that WebKit could not decode.");
            }
          }
        }
      } catch (reason) {
        if (cancelled) return;
        setStreamLoading(false);
        setStreamError(reason instanceof Error ? reason.message : String(reason));
      }
    };

    void openStream();

    return () => {
      cancelled = true;
      const context = canvas.getContext("2d");
      context?.clearRect(0, 0, canvas.width, canvas.height);
      if (sessionId !== undefined) void stopCameraPreview(sessionId);
    };
    // Availability polling replaces the descriptor object every two seconds;
    // only the stable native ID identifies whether this is a different camera.
  }, [detailsOpen, device.id, canvasMounted]);

  const useCamera = useCallback(async () => {
    if (routeLoading || (!routeActive && !streamInfo)) return;

    setRouteLoading(true);
    setRouteError(undefined);
    try {
      if (routeActive) {
        await stopCameraRoute();
        announceRoutedCamera(undefined);
        return;
      }

      announceRoutedCamera(undefined);
      await routeCameraToVirtual(streamInfo!.sessionId);
      announceRoutedCamera(device.id);
    } catch (error) {
      setRouteError(parseError(error));
    } finally {
      setRouteLoading(false);
    }
  }, [device.id, routeActive, routeLoading, streamInfo]);

  // camera item
  return (
    <>
      <div
        className="grid min-h-16 cursor-pointer grid-cols-[40px_minmax(0,1fr)_auto] items-center gap-3 px-4 py-2.5 transition-colors hover:bg-muted/50"
        onClick={() => {
          confirmedControls.current = device.controls;
          setControls(device.controls);
          setRouteError(undefined);
          setDetailsOpen(true);
        }}
      >
        <span className="grid size-10 place-items-center rounded-lg bg-muted text-muted-foreground">
          <Camera className="size-5" aria-hidden="true" />
        </span>
        <span className="min-w-0">
          <strong className="block truncate text-sm font-medium">{device.name}</strong>
          <small className="mt-1 block truncate font-mono text-[10px] text-muted-foreground">{device.id}</small>
        </span>
        <Badge variant="secondary" className="gap-1.5 text-[11px] font-normal items-center text-muted-foreground">
          <StatusIndicator status={availability.kind} />
          <span className="max-w-48 truncate" title={availability.label}>
            {availability.label}
          </span>
        </Badge>
      </div>

      {/* details dialog */}
      <Dialog open={detailsOpen} onOpenChange={setDetailsOpen}>
        <DialogContent
          className="grid-rows-[auto_minmax(0,1fr)_19rem_auto] gap-0 overflow-hidden p-0"
          style={{ height: "min(52rem, calc(100vh - 2rem))", maxWidth: "48rem" }}
        >
          <div className="flex items-start gap-3 border-b px-6 py-5 pr-16">
            {/* status indicator */}
            <StatusIndicator status={availability.kind} className="mt-1.5 size-2" />

            {/* name & id */}
            <DialogHeader className="min-w-0 gap-0.5">
              <DialogTitle className="leading-5">{device.name}</DialogTitle>
              <DialogDescription className="leading-4">
                <span className="block">{device.id}</span>
              </DialogDescription>
            </DialogHeader>
          </div>

          {/* realtime stream */}
          <div className="min-h-0 border-b p-4">
            <div className="relative grid h-full w-full place-items-center overflow-hidden rounded-2xl bg-card/50">
              <canvas
                id="webcam-stream"
                ref={attachPreviewCanvas}
                role="img"
                aria-label={`Live preview from ${device.name}`}
                className="absolute inset-0 h-full w-full object-contain"
              />
              {streamLoading && (
                <div className="z-10 flex items-center gap-2 text-sm text-muted-foreground">
                  <LoaderCircle className="size-4 animate-spin" aria-hidden="true" />
                  Starting camera…
                </div>
              )}
              {streamError && (
                <div className="z-10 flex max-w-md flex-col items-center gap-2 px-6 text-center text-sm text-muted-foreground">
                  <CameraOff className="size-6" aria-hidden="true" />
                  <span>Unable to display the camera stream.</span>
                  <small className="text-xs">{streamError}</small>
                </div>
              )}
            </div>
          </div>

          <div className="min-h-0 overflow-hidden px-5 py-4">
            {controlGroups.length > 0 ? (
              <TabsPrimitive.Root
                defaultValue={controlGroups[0].id}
                className="grid h-full grid-rows-[auto_minmax(0,1fr)]"
                aria-label="Camera control surfaces"
              >
                <div className="flex flex-row justify-between items-center mb-4">
                  <TabsPrimitive.List className="flex w-fit max-w-full flex-wrap gap-1 rounded-xl bg-muted p-1">
                    {controlGroups.map((group) => (
                      <TabsPrimitive.Trigger
                        key={group.id}
                        value={group.id}
                        className="shrink-0 rounded-lg px-3 py-1.5 text-xs font-medium text-muted-foreground outline-none transition-colors hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/30 data-[state=active]:bg-background data-[state=active]:text-foreground data-[state=active]:shadow-sm"
                      >
                        {group.name}
                      </TabsPrimitive.Trigger>
                    ))}
                  </TabsPrimitive.List>

                  <span className="text-muted-foreground text-xs">Changes are applied automatically</span>
                </div>

                {controlGroups.map((group) => {
                  const pageCount = Math.ceil(group.controls.length / controlsPerPage);
                  const currentPage = Math.min(controlPages[group.id] ?? 0, pageCount - 1);
                  const pageControls = group.controls.slice(currentPage * controlsPerPage, (currentPage + 1) * controlsPerPage);
                  const setPage = (page: number) => {
                    setControlPages((current) => ({
                      ...current,
                      [group.id]: Math.min(pageCount - 1, Math.max(0, page)),
                    }));
                  };

                  return (
                    <TabsPrimitive.Content
                      key={group.id}
                      value={group.id}
                      className="grid h-full min-h-0 grid-rows-[minmax(0,1fr)_auto] gap-3 outline-none"
                    >
                      <div className="grid min-h-0 grid-cols-1 content-start gap-2 sm:grid-cols-2">
                        {pageControls.map((control) => (
                          <ControlEditor
                            key={control.id}
                            control={control}
                            error={controlErrors[control.id]}
                            onChange={(value, throttle) => updateControl(control, value, throttle)}
                          />
                        ))}
                      </div>
                      <div className="h-8">
                        {pageCount > 1 && (
                          <Pagination aria-label={`${group.name} pages`}>
                            <PaginationContent>
                              <PaginationItem>
                                <PaginationPrevious
                                  href="#"
                                  text="Previous"
                                  aria-disabled={currentPage === 0}
                                  className={currentPage === 0 ? "pointer-events-none opacity-50" : undefined}
                                  onClick={(event) => {
                                    event.preventDefault();
                                    setPage(currentPage - 1);
                                  }}
                                />
                              </PaginationItem>
                              <PaginationItem>
                                <span className="flex h-8 min-w-20 items-center justify-center px-2 text-xs text-muted-foreground">
                                  {currentPage + 1} / {pageCount}
                                </span>
                              </PaginationItem>
                              <PaginationItem>
                                <PaginationNext
                                  href="#"
                                  text="Next"
                                  aria-disabled={currentPage === pageCount - 1}
                                  className={currentPage === pageCount - 1 ? "pointer-events-none opacity-50" : undefined}
                                  onClick={(event) => {
                                    event.preventDefault();
                                    setPage(currentPage + 1);
                                  }}
                                />
                              </PaginationItem>
                            </PaginationContent>
                          </Pagination>
                        )}
                      </div>
                    </TabsPrimitive.Content>
                  );
                })}
              </TabsPrimitive.Root>
            ) : (
              <p className="py-4 text-center text-sm text-muted-foreground">No camera controls reported.</p>
            )}
          </div>

          <div className="flex min-h-16 items-center justify-between gap-4 border-t px-5 py-3">
            <div className="min-w-0" aria-live="polite">
              {routeError ? (
                <p className="truncate text-xs text-destructive" title={routeError}>
                  {routeError}
                </p>
              ) : streamInfo ? (
                <p className="text-xs text-muted-foreground">
                  {streamInfo.width}×{streamInfo.height} · {streamInfo.frameRate.toFixed(0)} fps · {streamInfo.pixelFormat}
                </p>
              ) : (
                <p className="text-xs text-muted-foreground">Waiting for the first camera frame…</p>
              )}
            </div>
            <Button type="button" className="shrink-0" disabled={routeLoading || (!routeActive && !streamInfo)} onClick={useCamera}>
              {routeLoading ? (
                <LoaderCircle className="animate-spin" aria-hidden="true" />
              ) : routeActive ? (
                <CameraOff aria-hidden="true" />
              ) : (
                <Camera aria-hidden="true" />
              )}
              {routeActive ? "Stop using this camera" : "Use this camera"}
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
};
