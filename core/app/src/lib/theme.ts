import { useSyncExternalStore } from "react";

export const THEME_OPTIONS = ["dark", "light", "system"] as const;

export type Theme = (typeof THEME_OPTIONS)[number];
export type ResolvedTheme = Exclude<Theme, "system">;

const STORAGE_KEY = "theme";
const systemTheme = window.matchMedia("(prefers-color-scheme: dark)");

function isTheme(value: string | null): value is Theme {
  return value !== null && THEME_OPTIONS.some((theme) => theme === value);
}

function storedTheme(): Theme {
  try {
    const theme = localStorage.getItem(STORAGE_KEY);
    return isTheme(theme) ? theme : "system";
  } catch {
    return "system";
  }
}

let theme = storedTheme();
let listeningForSystemTheme = false;
const subscribers = new Set<() => void>();

function getTheme(): Theme {
  return theme;
}

export function getResolvedTheme(): ResolvedTheme {
  return theme === "system" ? (systemTheme.matches ? "dark" : "light") : theme;
}

function applyTheme() {
  const resolvedTheme = getResolvedTheme();
  const root = document.documentElement;

  root.classList.toggle("dark", resolvedTheme === "dark");
  root.dataset.theme = theme;
  root.style.colorScheme = resolvedTheme;

  if (theme === "system" && !listeningForSystemTheme) {
    systemTheme.addEventListener("change", applyTheme);
    listeningForSystemTheme = true;
  } else if (theme !== "system" && listeningForSystemTheme) {
    systemTheme.removeEventListener("change", applyTheme);
    listeningForSystemTheme = false;
  }
}

function setTheme(nextTheme: Theme) {
  if (!isTheme(nextTheme)) {
    throw new TypeError(`Invalid theme: ${String(nextTheme)}`);
  }

  if (theme === nextTheme) return;

  theme = nextTheme;

  try {
    localStorage.setItem(STORAGE_KEY, theme);
  } catch {
    // Applying the theme should still succeed if persistence is unavailable.
  }

  applyTheme();
  subscribers.forEach((notify) => notify());
}

function subscribe(notify: () => void) {
  subscribers.add(notify);
  return () => subscribers.delete(notify);
}

export function useTheme() {
  const theme = useSyncExternalStore(subscribe, getTheme, getTheme);

  return { theme, setTheme };
}

/** Apply the stored theme before the application renders. */
export function initializeTheme() {
  applyTheme();
}
