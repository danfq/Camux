import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";

type ResizeDirection = Parameters<ReturnType<typeof getCurrentWindow>["startResizeDragging"]>[0];

const handles: Array<{ direction: ResizeDirection; className: string }> = [
  { direction: "North", className: "inset-x-3 top-0 h-1.5 cursor-n-resize" },
  { direction: "South", className: "inset-x-3 bottom-0 h-1.5 cursor-s-resize" },
  { direction: "West", className: "inset-y-3 left-0 w-1.5 cursor-w-resize" },
  { direction: "East", className: "inset-y-3 right-0 w-1.5 cursor-e-resize" },
  { direction: "NorthWest", className: "top-0 left-0 size-3 cursor-nw-resize" },
  { direction: "NorthEast", className: "top-0 right-0 size-3 cursor-ne-resize" },
  { direction: "SouthWest", className: "bottom-0 left-0 size-3 cursor-sw-resize" },
  { direction: "SouthEast", className: "right-0 bottom-0 size-3 cursor-se-resize" },
];

export function WindowResizeHandles() {
  const [maximized, setMaximized] = useState(false);
  const runningInTauri = isTauri();

  useEffect(() => {
    if (!runningInTauri) return;

    const appWindow = getCurrentWindow();
    let disposed = false;
    let unlisten: (() => void) | undefined;

    const updateMaximized = () => {
      void appWindow.isMaximized().then((value) => {
        if (!disposed) setMaximized(value);
      });
    };

    updateMaximized();
    void appWindow.onResized(updateMaximized).then((stopListening) => {
      if (disposed) stopListening();
      else unlisten = stopListening;
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [runningInTauri]);

  if (!runningInTauri || maximized) return null;

  const appWindow = getCurrentWindow();

  return handles.map(({ direction, className }) => (
    <div
      key={direction}
      className={`fixed z-50 ${className}`}
      aria-hidden="true"
      onMouseDown={(event) => {
        if (event.button !== 0) return;

        event.preventDefault();
        event.stopPropagation();
        void appWindow.startResizeDragging(direction);
      }}
    />
  ));
}
