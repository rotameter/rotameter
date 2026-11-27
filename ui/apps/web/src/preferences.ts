export type ThemePreference = "light" | "dark" | "system";
export type DensityPreference = "compact" | "standard" | "comfortable";

export interface UiPreferences {
  theme: ThemePreference;
  density: DensityPreference;
  sidebarCollapsed: boolean;
  streamPaused: boolean;
}

const PREFS_KEY = "rotameter_ui_prefs";

export const DEFAULT_PREFERENCES: UiPreferences = {
  theme: "system",
  density: "standard",
  sidebarCollapsed: false,
  streamPaused: false,
};

export function loadPreferences(storage?: Storage): UiPreferences {
  const store = storage ?? (typeof localStorage !== "undefined" ? localStorage : undefined);
  if (!store) return DEFAULT_PREFERENCES;
  try {
    const raw = store.getItem(PREFS_KEY);
    if (!raw) return DEFAULT_PREFERENCES;
    const parsed = JSON.parse(raw) as Partial<UiPreferences>;
    return {
      theme:
        parsed.theme === "light" || parsed.theme === "dark" || parsed.theme === "system"
          ? parsed.theme
          : "system",
      density:
        parsed.density === "compact" ||
        parsed.density === "standard" ||
        parsed.density === "comfortable"
          ? parsed.density
          : "standard",
      sidebarCollapsed: Boolean(parsed.sidebarCollapsed),
      streamPaused: Boolean(parsed.streamPaused),
    };
  } catch {
    return DEFAULT_PREFERENCES;
  }
}

export function savePreferences(prefs: UiPreferences, storage?: Storage): void {
  const store = storage ?? (typeof localStorage !== "undefined" ? localStorage : undefined);
  if (!store) return;
  try {
    store.setItem(PREFS_KEY, JSON.stringify(prefs));
  } catch {
    // ignore storage quota / private mode failures
  }
}

export function resolveTheme(theme: ThemePreference, prefersDark = false): "light" | "dark" {
  if (theme === "light" || theme === "dark") return theme;
  return prefersDark ? "dark" : "light";
}

export function applyPreferencesToDom(prefs: UiPreferences): void {
  if (typeof document === "undefined") return;
  const prefersDark =
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-color-scheme: dark)").matches;
  const root = document.documentElement;
  root.setAttribute("data-theme", resolveTheme(prefs.theme, prefersDark));
  root.setAttribute("data-density", prefs.density);
}
