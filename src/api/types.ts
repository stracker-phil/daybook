// Re-exports the IPC types generated from Rust (`bun run types`); everything defined below is frontend-only.
import type { TextSize, UiState } from "./types.gen";

export type * from "./types.gen";

/** The text size steps, smallest first, with their pixel sizes: `ui` for the chrome (it also scales rem), `data` for
 *  times, durations and titles. */
export const TEXT_SIZES: readonly { size: TextSize; ui: number; data: number }[] = [
  { size: "small", ui: 12, data: 11 },
  { size: "medium", ui: 13, data: 12 },
  { size: "large", ui: 15, data: 14 },
];

/** Rust's `TextSize::default()`. */
export const DEFAULT_TEXT_SIZE: TextSize = "medium";

/** Accent hues, defined by every palette. Rust's `ACCENTS` is the source of truth; keep these equal. */
export const ACCENTS = ["orange", "gold", "red", "magenta", "purple", "indigo", "blue", "teal", "graphite"] as const;

/** The app accent that follows the macOS accent color (Rust's `SYSTEM_ACCENT`). */
export const SYSTEM_ACCENT = "system";

/** Shortcut overrides by command id; `null` unbinds. Rust: the `Keybindings` alias (a map, so ts-rs can't export it). */
export type Keybindings = Record<string, string | null>;

/** Until Rust's state loads. */
export const EMPTY_UI_STATE: UiState = { sidebarHidden: false, inspectorHidden: false, sidebarWidth: 200, paneSizes: {}, hidePastEvents: false, collapsedTodoGroups: [] };
