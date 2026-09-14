import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./global.css";
import App from "./App.tsx";
import { initializeTheme } from "./lib/theme";

initializeTheme(); // Apply the saved palette before the first paint.

const preventDefault = (event: Event) => event.preventDefault();

for (const eventName of ["auxclick", "contextmenu", "dragover", "dragstart", "drop", "selectstart"]) {
  window.addEventListener(eventName, preventDefault, { capture: true });
}

window.addEventListener(
  "wheel",
  (event) => {
    if (event.ctrlKey || event.metaKey) event.preventDefault();
  },
  { capture: true, passive: false },
);

const browserShortcutKeys = new Set(["+", "-", "0", "=", "f", "g", "h", "j", "l", "o", "p", "r", "s", "u"]);

window.addEventListener(
  "keydown",
  (event) => {
    const shortcutKey = event.key.toLowerCase();
    const browserShortcut = (event.ctrlKey || event.metaKey) && browserShortcutKeys.has(shortcutKey);
    const developerToolShortcut =
      ((event.ctrlKey && event.shiftKey) || (event.metaKey && event.altKey)) && ["c", "i", "j"].includes(shortcutKey);
    const browserFunctionKey = event.key === "F5" || event.key === "F7" || event.key === "F12";
    const historyShortcut = event.altKey && (event.key === "ArrowLeft" || event.key === "ArrowRight");

    if (browserShortcut || developerToolShortcut || browserFunctionKey || historyShortcut) {
      event.preventDefault();
      event.stopPropagation();
    }
  },
  { capture: true },
);

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
