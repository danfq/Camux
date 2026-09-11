import { useEffect, useState } from "react";
import { Camera, CameraOff, CircleAlert, RefreshCw } from "lucide-react";
import { WindowTitlebar } from "tauri-controls";

import { version as appVersion } from "../package.json";
import { getDevices, showMainWindow } from "./core/backend";
import type { CameraDevice } from "./core/backend";

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
    <div className="app-shell">
      <WindowTitlebar className="window-titlebar">
        <div className="window-titlebar-content" data-tauri-drag-region>
          <span data-tauri-drag-region>Camux v{appVersion}</span>
        </div>
      </WindowTitlebar>

      <main>
        <div className="page-heading">
          <div>
            <h1>Cameras</h1>
            <p>Sources available on this computer</p>
          </div>
          <button className="refresh-button" aria-busy={loading} disabled={loading} onClick={() => void refreshDevices()}>
            <RefreshCw className={loading ? "spinning" : undefined} aria-hidden="true" />
            {loading ? "Scanning" : "Refresh"}
          </button>
        </div>

        <section className="device-section" aria-labelledby="device-heading">
          <header className="section-heading">
            <h2 id="device-heading">Devices</h2>
            <span>{cameraLabel}</span>
          </header>

          <div className="device-content" aria-live="polite">
            {loading && (
              <div className="message">
                <span className="spinner" />
                <div>
                  <strong>Scanning for cameras</strong>
                  <p>Connected devices will appear here.</p>
                </div>
              </div>
            )}

            {!loading && error && (
              <div className="message message-error" role="alert">
                <CircleAlert className="message-icon error-icon" aria-hidden="true" />
                <div>
                  <strong>Could not scan for cameras</strong>
                  <p>{error}</p>
                </div>
              </div>
            )}

            {!loading && !error && cameraCount === 0 && (
              <div className="message">
                <span className="device-icon muted">
                  <CameraOff aria-hidden="true" />
                </span>
                <div>
                  <strong>No cameras found</strong>
                  <p>Connect a camera and refresh the list.</p>
                </div>
              </div>
            )}

            {!loading && !error && cameraCount > 0 && (
              <ul className="device-list" aria-label="Camera devices">
                {devices.map((device) => (
                  <li key={device.id}>
                    <span className="device-icon">
                      <Camera aria-hidden="true" />
                    </span>
                    <span className="device-details">
                      <strong>{device.name}</strong>
                      <small>{device.id}</small>
                    </span>
                    <span className="available">
                      <i />
                      Available
                    </span>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </section>
      </main>
    </div>
  );
}
