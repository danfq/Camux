import { useEffect, useState } from "react";
import { Camera, CameraOff, CircleAlert, LoaderCircle, RefreshCw, Settings } from "lucide-react";
import { WindowTitlebar } from "tauri-controls";

import { version as appVersion } from "../package.json";
import { Badge } from "./components/ui/badge";
import { getDevices, showMainWindow } from "./core/backend";
import type { CameraDevice } from "./core/backend";
import { Button } from "./components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "./components/ui/card";

export default function App() {
  const [devices, setDevices] = useState<CameraDevice[]>([]);
  const [error, setError] = useState<string>();
  const [loading, setLoading] = useState(true);

  async function refreshDevices() {
    setLoading(true);
    setError(undefined);

    try {
      setDevices(await getDevices());
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    let active = true;

    void showMainWindow();
    void getDevices()
      .then((nextDevices) => {
        if (active) setDevices(nextDevices);
      })
      .catch((reason: unknown) => {
        if (active) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      })
      .finally(() => {
        if (active) setLoading(false);
      });

    return () => {
      active = false;
    };
  }, []);

  const cameraCount = devices.length;
  const cameraLabel = `${cameraCount} ${cameraCount === 1 ? "camera" : "cameras"}`;

  return (
    <div className="grid h-full w-full grid-rows-[40px_minmax(0,1fr)] bg-background text-foreground">
      <WindowTitlebar className="relative z-10 h-10 border-b bg-card/80 text-card-foreground backdrop-blur">
        <div className="flex h-full min-w-0 flex-1 items-center justify-between px-3" data-tauri-drag-region>
          <div className="flex min-w-0 items-center gap-2" data-tauri-drag-region>
            <span className="truncate text-sm font-semibold" data-tauri-drag-region>
              Camux
            </span>
            <span className="text-[0.6rem] text-muted-foreground" data-tauri-drag-region>
              v{appVersion}
            </span>
          </div>

          <Button variant="ghost" size="icon-xs" aria-label="Settings" title="Settings">
            <Settings aria-hidden="true" />
          </Button>
        </div>
      </WindowTitlebar>

      <main className="grid min-h-0 grid-rows-[auto_minmax(0,1fr)] gap-6 px-6 py-5">
        <header className="flex items-center justify-between gap-6">
          <div className="min-w-0">
            <h1 className="text-xl font-semibold tracking-tight">Cameras</h1>
            <p className="mt-1 text-sm text-muted-foreground">Sources available on this computer</p>
          </div>
          <Button variant="secondary" size="sm" aria-busy={loading} disabled={loading} onClick={() => void refreshDevices()}>
            <RefreshCw className={loading ? "animate-spin" : undefined} aria-hidden="true" />
            {loading ? "Scanning" : "Refresh"}
          </Button>
        </header>

        <Card className="min-h-0 gap-0 overflow-hidden py-0 shadow-sm" aria-labelledby="device-heading">
          <CardHeader className="flex h-12 shrink-0 flex-row items-center justify-between border-b px-4">
            <CardTitle id="device-heading" className="text-sm">
              Devices
            </CardTitle>
            <Badge variant="secondary" className="text-[11px] text-muted-foreground">
              {cameraLabel}
            </Badge>
          </CardHeader>

          <CardContent className="min-h-0 flex-1 overflow-auto p-0 [scrollbar-color:var(--border)_transparent]" aria-live="polite">
            {loading && (
              <div className="flex h-full min-h-40 items-center justify-center gap-3 px-6 py-10">
                <LoaderCircle className="size-7 animate-spin text-muted-foreground" aria-hidden="true" />
                <div>
                  <strong className="block text-sm font-medium">Scanning for cameras</strong>
                  <p className="mt-1 text-xs text-muted-foreground">Connected devices will appear here.</p>
                </div>
              </div>
            )}

            {!loading && error && (
              <div className="flex h-full min-h-40 items-center justify-center gap-3 px-6 py-10" role="alert">
                <span className="grid size-9 shrink-0 place-items-center rounded-lg bg-destructive/10 text-destructive">
                  <CircleAlert className="size-5" aria-hidden="true" />
                </span>
                <div>
                  <strong className="block text-sm font-medium">Could not scan for cameras</strong>
                  <p className="mt-1 max-w-md text-xs leading-relaxed text-muted-foreground wrap-break-word">{error}</p>
                </div>
              </div>
            )}

            {!loading && !error && cameraCount === 0 && (
              <div className="flex h-full min-h-40 items-center justify-center gap-3 px-6 py-10">
                <span className="grid size-9 shrink-0 place-items-center rounded-lg border bg-muted text-muted-foreground">
                  <CameraOff className="size-5" aria-hidden="true" />
                </span>
                <div>
                  <strong className="block text-sm font-medium">No cameras found</strong>
                  <p className="mt-1 text-xs text-muted-foreground">Connect a camera and refresh the list.</p>
                </div>
              </div>
            )}

            {!loading && !error && cameraCount > 0 && (
              <ul className="divide-y" aria-label="Camera devices">
                {devices.map((device) => (
                  <li
                    key={device.id}
                    className="grid min-h-16 grid-cols-[40px_minmax(0,1fr)_auto] items-center gap-3 px-4 py-2.5 transition-colors hover:bg-muted/50"
                  >
                    <span className="grid size-10 place-items-center rounded-lg bg-muted text-muted-foreground">
                      <Camera className="size-5" aria-hidden="true" />
                    </span>
                    <span className="min-w-0">
                      <strong className="block truncate text-sm font-medium">{device.name}</strong>
                      <small className="mt-1 block truncate font-mono text-[10px] text-muted-foreground">{device.id}</small>
                    </span>
                    <Badge variant="secondary" className="gap-1.5 text-[11px] font-normal text-muted-foreground">
                      <span className="size-1.5 rounded-full bg-emerald-500" aria-hidden="true" />
                      Available
                    </Badge>
                  </li>
                ))}
              </ul>
            )}
          </CardContent>
        </Card>
      </main>
    </div>
  );
}
