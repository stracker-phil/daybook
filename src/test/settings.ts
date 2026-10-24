import type { Settings } from "../api/types";

/** Equal to Rust's `Settings::default()`, but with the orange accent tests look for. */
export function settings(patch: Partial<Settings> = {}): Settings {
  return {
    lightScheme: "default",
    darkScheme: "dark",
    accent: "orange",
    textSize: "medium",
    launchAtLogin: false,
    ...patch,
  };
}
