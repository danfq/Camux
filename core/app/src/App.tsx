import { useEffect, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { getDevices } from "./core/backend";
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

  useEffect(() => {
    if (loading) return;

    // Let React commit the settled camera state and let the browser paint it before
    // revealing the native window. This prevents a blank or intermediate loading
    // frame from flashing at startup.
    let cancelled = false;
    let secondFrame = 0;
    const firstFrame = requestAnimationFrame(() => {
      secondFrame = requestAnimationFrame(() => {
        if (!cancelled && isTauri()) {
          void getCurrentWindow().show();
        }
      });
    });

    return () => {
      cancelled = true;
      cancelAnimationFrame(firstFrame);
      cancelAnimationFrame(secondFrame);
    };
  }, [loading]);

  return (
    <main>
      <h1>Camux</h1>
      <p>A lightweight, cross-platform webcam splitter.</p>

      <button
        aria-busy={loading}
        disabled={loading}
        onClick={() => void refreshDevices()}
      >
        Refresh cameras
      </button>

      {error && <p role="alert">Could not load cameras: {error}</p>}

      {!loading && !error && devices.length === 0 && <p>No cameras found.</p>}

      {devices.length > 0 && (
        <ul aria-label="Camera devices">
          {devices.map((device) => (
            <li key={device.id}>
              <strong>{device.name}</strong>
              <small>{device.id}</small>
            </li>
          ))}
        </ul>
      )}
    </main>
  );
}
