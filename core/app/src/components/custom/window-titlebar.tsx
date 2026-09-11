import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { type as getOsType } from "@tauri-apps/plugin-os";
import { useEffect, useState, type MouseEvent, type PropsWithChildren } from "react";

type DesktopPlatform = "macos" | "windows" | "linux";

function safelyUnlisten(unlisten: () => void) {
  try {
    void Promise.resolve(unlisten()).catch(() => undefined);
  } catch {
    // The native window may already have removed its listeners while exiting.
  }
}

function detectPlatform(): DesktopPlatform {
  if (isTauri()) {
    const osType = getOsType();
    if (osType === "macos" || osType === "windows") return osType;
    return "linux";
  }

  const agent = `${navigator.platform} ${navigator.userAgent}`;

  if (/mac/i.test(agent)) return "macos";
  if (/win/i.test(agent)) return "windows";
  return "linux";
}

type WindowControlIconKind = "close" | "exit-fullscreen" | "fullscreen" | "maximize" | "minimize" | "restore";

function WindowControlIcon({ kind }: { kind: WindowControlIconKind }) {
  if (kind === "minimize") {
    return (
      <svg viewBox="0 0 16 16" aria-hidden="true">
        <path d="M3 8.5h10" />
      </svg>
    );
  }

  if (kind === "maximize") {
    return (
      <svg viewBox="0 0 16 16" aria-hidden="true">
        <rect x="3.5" y="3.5" width="9" height="9" rx="0.5" />
      </svg>
    );
  }

  if (kind === "restore") {
    return (
      <svg viewBox="0 0 16 16" aria-hidden="true">
        <path d="M5.5 5.5V3.5h7v7h-2" />
        <rect x="3.5" y="5.5" width="7" height="7" rx="0.5" />
      </svg>
    );
  }

  if (kind === "fullscreen") {
    return (
      <svg viewBox="0 0 16 16" aria-hidden="true">
        <path d="M6.5 6.5 3.5 3.5m0 0v2.25m0-2.25h2.25m3.75 6 3 3m0 0v-2.25m0 2.25h-2.25" />
      </svg>
    );
  }

  if (kind === "exit-fullscreen") {
    return (
      <svg viewBox="0 0 16 16" aria-hidden="true">
        <path d="m3.5 3.5 3 3m0 0V4.25m0 2.25H4.25m8.25 6-3-3m0 0v2.25m0-2.25h2.25" />
      </svg>
    );
  }

  return (
    <svg viewBox="0 0 16 16" aria-hidden="true">
      <path d="m4 4 8 8M12 4l-8 8" />
    </svg>
  );
}

type WindowControlButtonProps = {
  action: "close" | "maximize" | "minimize";
  icon: WindowControlIconKind;
  label: string;
  onClick: () => void;
};

function WindowControlButton({ action, icon, label, onClick }: WindowControlButtonProps) {
  const stopTitlebarDrag = (event: MouseEvent<HTMLButtonElement>) => event.stopPropagation();

  return (
    <button
      type="button"
      className={`window-control window-control--${action}`}
      aria-label={label}
      title={label}
      onMouseDown={stopTitlebarDrag}
      onDoubleClick={stopTitlebarDrag}
      onClick={onClick}
    >
      <WindowControlIcon kind={icon} />
    </button>
  );
}

export function WindowTitlebar({ children }: PropsWithChildren) {
  const runningInTauri = isTauri();
  const platform = detectPlatform();
  const [focused, setFocused] = useState(true);
  const [fullscreen, setFullscreen] = useState(false);
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    if (!runningInTauri) return;

    let disposed = false;
    const appWindow = getCurrentWindow();
    const unlisteners: Array<() => void> = [];

    void appWindow.isFocused().then((value) => {
      if (!disposed) setFocused(value);
    });
    void appWindow.isMaximized().then((value) => {
      if (!disposed) setMaximized(value);
    });
    void appWindow.isFullscreen().then((value) => {
      if (!disposed) setFullscreen(value);
    });

    void appWindow.onFocusChanged(({ payload }) => {
      if (!disposed) setFocused(payload);
    }).then((unlisten) => {
      if (disposed) safelyUnlisten(unlisten);
      else unlisteners.push(unlisten);
    });

    void appWindow.onResized(() => {
      void appWindow.isMaximized().then((value) => {
        if (!disposed) setMaximized(value);
      });
      void appWindow.isFullscreen().then((value) => {
        if (!disposed) setFullscreen(value);
      });
    }).then((unlisten) => {
      if (disposed) safelyUnlisten(unlisten);
      else unlisteners.push(unlisten);
    });

    return () => {
      disposed = true;
      for (const unlisten of unlisteners) safelyUnlisten(unlisten);
    };
  }, [runningInTauri]);

  const runWindowAction = (action: "close" | "maximize" | "minimize") => {
    if (!runningInTauri) return;

    const appWindow = getCurrentWindow();
    if (action === "close") void appWindow.close();
    else if (action === "minimize") void appWindow.minimize();
    else if (platform === "macos") {
      void appWindow.isFullscreen().then((fullscreen) => appWindow.setFullscreen(!fullscreen));
    } else {
      void appWindow.toggleMaximize();
    }
  };

  const handleTitlebarMouseDown = (event: MouseEvent<HTMLElement>) => {
    if (!runningInTauri || platform !== "macos" || event.button !== 0 || event.detail !== 2) return;

    const target = event.target;
    if (target instanceof Element && target.closest("button, a, input, select, textarea, [role='button']")) return;

    // Tauri waits for mouseup on macOS and cancels zoom if its coordinates differ
    // at all from mousedown. Handle the second press directly so normal click
    // jitter does not make title-bar zoom appear broken.
    event.preventDefault();
    event.stopPropagation();
    void getCurrentWindow().toggleMaximize();
  };

  const controls = (
    <div className="window-controls" role="group" aria-label="Window controls">
      {platform === "macos" ? (
        <>
          <WindowControlButton action="close" icon="close" label="Close" onClick={() => runWindowAction("close")} />
          <WindowControlButton action="minimize" icon="minimize" label="Minimize" onClick={() => runWindowAction("minimize")} />
          <WindowControlButton
            action="maximize"
            icon={fullscreen ? "exit-fullscreen" : "fullscreen"}
            label={fullscreen ? "Exit full screen" : "Enter full screen"}
            onClick={() => runWindowAction("maximize")}
          />
        </>
      ) : (
        <>
          <WindowControlButton action="minimize" icon="minimize" label="Minimize" onClick={() => runWindowAction("minimize")} />
          <WindowControlButton
            action="maximize"
            icon={maximized ? "restore" : "maximize"}
            label={maximized ? "Restore" : "Maximize"}
            onClick={() => runWindowAction("maximize")}
          />
          <WindowControlButton action="close" icon="close" label="Close" onClick={() => runWindowAction("close")} />
        </>
      )}
    </div>
  );

  return (
    <header
      className={`window-titlebar window-titlebar--${platform}${focused ? "" : " window-titlebar--unfocused"}`}
      data-tauri-drag-region
      onMouseDownCapture={handleTitlebarMouseDown}
    >
      {platform === "macos" && controls}
      <div className="window-titlebar__content" data-tauri-drag-region>
        {children}
      </div>
      {platform !== "macos" && controls}
    </header>
  );
}
