import { useEffect, useState } from "react";

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

  return (
    <main>
      <h1>Camux</h1>
      <p>A lightweight, cross-platform webcam splitter.</p>

      <button disabled={loading} onClick={() => void refreshDevices()}>
        {loading ? "Finding cameras…" : "Refresh cameras"}
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
