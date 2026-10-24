import type { Settings } from "../api/types";
import defaults from "./default-settings.json";

/** Equal to Rust's `Settings::default()`, but with the orange accent tests look for. */
export function settings(patch: Partial<Settings> = {}): Settings {
  return { ...(structuredClone(defaults) as Settings), accent: "orange", ...patch };
}
