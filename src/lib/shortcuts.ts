/** Global shortcuts in Settings notation (`shift+cmd+k`): reading a key press, comparing, showing as glyphs. */

interface ParsedShortcut {
  cmd: boolean;
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  key: string;
}

function parseShortcut(shortcut?: string | null): ParsedShortcut | undefined {
  if (!shortcut) return undefined;
  const parts = shortcut
    .toLowerCase()
    .split("+")
    .map((p) => p.trim());
  const key = parts.pop();
  if (!key) return undefined;
  return {
    cmd: parts.includes("cmd") || parts.includes("command"),
    ctrl: parts.includes("ctrl") || parts.includes("control"),
    alt: parts.includes("opt") || parts.includes("alt") || parts.includes("option"),
    shift: parts.includes("shift"),
    key,
  };
}

/** One spelling per shortcut, so "CMD+Shift+K" and "shift+cmd+k" compare equal. */
function canonicalShortcut(shortcut?: string): string | undefined {
  const parsed = parseShortcut(shortcut);
  if (!parsed) return undefined;
  const modifiers = [parsed.ctrl && "ctrl", parsed.alt && "alt", parsed.shift && "shift", parsed.cmd && "cmd"];
  return [...modifiers.filter(Boolean), parsed.key].join("+");
}

const KEYS_BY_CODE: Record<string, string> = {
  Comma: ",",
  Period: ".",
  Slash: "/",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  BracketLeft: "[",
  BracketRight: "]",
  Minus: "-",
  Equal: "=",
  Backquote: "`",
  Space: "space",
  Enter: "enter",
  Backspace: "backspace",
  Delete: "delete",
  Tab: "tab",
  ArrowUp: "arrowup",
  ArrowDown: "arrowdown",
  ArrowLeft: "arrowleft",
  ArrowRight: "arrowright",
};

/** The physical key, because ⌥ changes the typed character (⌥K types "˚"). */
function keyFromCode(code: string): string | undefined {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3).toLowerCase();
  if (/^Digit\d$/.test(code)) return code.slice(5);
  if (/^F\d{1,2}$/.test(code)) return code.toLowerCase();
  return KEYS_BY_CODE[code];
}

/**
 * The shortcut a key press describes, e.g. "shift+cmd+k". Needs ⌘, ⌃ or ⌥ (plain letters would fire while
 * typing), except for function keys; null for anything else.
 */
export function shortcutFromKeyEvent(event: Pick<KeyboardEvent, "code" | "metaKey" | "ctrlKey" | "altKey" | "shiftKey">): string | null {
  const key = keyFromCode(event.code);
  if (!key) return null;
  const isFunctionKey = /^f\d+$/.test(key);
  if (!isFunctionKey && !event.metaKey && !event.ctrlKey && !event.altKey) return null;
  const modifiers = [event.ctrlKey && "ctrl", event.altKey && "alt", event.shiftKey && "shift", event.metaKey && "cmd"];
  return [...modifiers.filter(Boolean), key].join("+");
}

export interface TakenShortcut {
  shortcut: string;
  label: string;
}

/** Shortcuts macOS owns system-wide, or that every app's standard menus use. */
export const SYSTEM_SHORTCUTS: TakenShortcut[] = [
  { shortcut: "cmd+space", label: "Spotlight" },
  { shortcut: "alt+cmd+space", label: "Finder Search" },
  { shortcut: "ctrl+space", label: "Input Sources" },
  { shortcut: "ctrl+cmd+space", label: "Emoji & Symbols" },
  { shortcut: "cmd+tab", label: "App Switcher" },
  { shortcut: "cmd+`", label: "Next Window" },
  { shortcut: "alt+cmd+escape", label: "Force Quit" },
  { shortcut: "ctrl+cmd+q", label: "Lock Screen" },
  { shortcut: "shift+cmd+3", label: "Screenshot" },
  { shortcut: "shift+cmd+4", label: "Screenshot of Selection" },
  { shortcut: "shift+cmd+5", label: "Screenshot Tool" },
  { shortcut: "cmd+q", label: "Quit" },
  { shortcut: "cmd+w", label: "Close Window" },
  { shortcut: "cmd+h", label: "Hide" },
  { shortcut: "alt+cmd+h", label: "Hide Others" },
  { shortcut: "cmd+m", label: "Minimize" },
  { shortcut: "cmd+,", label: "Settings" },
  { shortcut: "cmd+z", label: "Undo" },
  { shortcut: "shift+cmd+z", label: "Redo" },
  { shortcut: "cmd+x", label: "Cut" },
  { shortcut: "cmd+c", label: "Copy" },
  { shortcut: "cmd+v", label: "Paste" },
  { shortcut: "cmd+a", label: "Select All" },
  { shortcut: "ctrl+cmd+f", label: "Enter Full Screen" },
];

export function findShortcutConflict(shortcut: string | undefined, taken: TakenShortcut[]): TakenShortcut | undefined {
  const wanted = canonicalShortcut(shortcut);
  if (!wanted) return undefined;
  return taken.find((entry) => canonicalShortcut(entry.shortcut) === wanted);
}

const KEY_LABELS: Record<string, string> = {
  enter: "↩",
  return: "↩",
  backspace: "⌫",
  delete: "⌦",
  escape: "Esc",
  space: "Space",
  tab: "⇥",
  arrowup: "↑",
  arrowdown: "↓",
  arrowleft: "←",
  arrowright: "→",
};

/** `ctrl+alt+cmd+j` → `["⌃", "⌥", "⌘", "J"]`, in the usual order. */
export function shortcutKeys(shortcut?: string | null): string[] | undefined {
  const parsed = parseShortcut(shortcut);
  if (!parsed) return undefined;
  const keys: string[] = [];
  if (parsed.ctrl) keys.push("⌃");
  if (parsed.alt) keys.push("⌥");
  if (parsed.shift) keys.push("⇧");
  if (parsed.cmd) keys.push("⌘");
  keys.push(KEY_LABELS[parsed.key] ?? parsed.key.toUpperCase());
  return keys;
}
