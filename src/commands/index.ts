import { DEFAULT_TEXT_SIZE, TEXT_SIZES, api, type TextSize } from "../api";
import { fail } from "../state/requests";
import { useStore } from "../state/store";
import {
  allCommands,
  bindingFor,
  bindingLabel,
  getCommand,
  isRecordingTarget,
  isTextInput,
  registerCommands,
  resolveKeydown,
  setOverrides,
  type Command,
} from "./registry";
import { activeScopes, registerScope } from "./scopes";

export * from "./registry";
export * from "./scopes";

/** The window this page runs in: the Daybook window (`main`) or Settings. */
export type WindowName = "main" | "settings";

let currentWindow: WindowName = "main";

/** Registers the core scopes; safe to call again (tests and re-installs do). */
export function registerCoreScopes() {
  registerScope({ id: "global", parent: null, label: "Every window", isActive: () => true });
  // Settings lists the Daybook window's keys without running them.
  registerScope({ id: "hub", parent: "global", label: "Daybook window", isActive: () => currentWindow === "main" });
}

function saveTextSize(textSize: TextSize) {
  const s = useStore.getState();
  if (s.settings && s.settings.textSize !== textSize) void s.saveSettings({ ...s.settings, textSize }, { quiet: true });
}

/** The next text size step; past the smallest or largest it stays. */
function zoomText(step: 1 | -1) {
  const current = TEXT_SIZES.findIndex((t) => t.size === useStore.getState().settings?.textSize);
  const next = TEXT_SIZES[Math.min(Math.max(current + step, 0), TEXT_SIZES.length - 1)];
  saveTextSize(next.size);
}

function overlayOpen(): boolean {
  return useStore.getState().dialog !== null;
}

/** `?` opens the overview of the scopes active now; pressed again it closes it. */
function toggleShortcuts() {
  const s = useStore.getState();
  if (s.dialog?.type === "shortcuts") return s.closeDialog();
  if (!s.dialog) s.openDialog({ type: "shortcuts", scopes: activeScopes() });
}

function closeWindow() {
  void api.runCommand("window:close").catch((e) => fail(e, "Could not close the window"));
}

const notTyping = () => !isTextInput(document.activeElement);

/** The day before or after the shown one that has a note; nothing at either end. */
function stepDay(direction: "previousDay" | "nextDay") {
  const { hub, showDay } = useStore.getState();
  const day = hub?.[direction];
  if (day) showDay(day);
}

/** The windows' own keys; Settings, the palette and ⌘W are Rust commands with app menu keys. */
const APP_COMMANDS: Command[] = [
  { id: "shortcuts.show", title: "Keyboard shortcuts", scope: "hub", defaultBinding: "Shift+?", run: toggleShortcuts },
  { id: "sidebar.toggle", title: "Toggle sidebar", scope: "hub", defaultBinding: "Mod+Ctrl+S", run: () => useStore.getState().toggleSidebar() },
  { id: "day.previous", title: "Previous day with a note", scope: "hub", defaultBinding: "P", when: notTyping, run: () => stepDay("previousDay") },
  { id: "day.next", title: "Next day with a note", scope: "hub", defaultBinding: "N", when: notTyping, run: () => stepDay("nextDay") },
  { id: "day.today", title: "Back to today", scope: "hub", defaultBinding: "O", when: notTyping, run: () => useStore.getState().showDay(null) },
  { id: "inspector.toggle", title: "Toggle inspector", scope: "hub", defaultBinding: "Mod+Ctrl+I", run: () => useStore.getState().toggleInspector() },
  { id: "window.close", title: "Close window", scope: "global", defaultBinding: "Escape", when: notTyping, run: closeWindow },
  { id: "zoom.in", title: "Larger text", scope: "global", defaultBinding: "Mod+=", run: () => zoomText(1) },
  { id: "zoom.out", title: "Smaller text", scope: "global", defaultBinding: "Mod+-", run: () => zoomText(-1) },
  { id: "zoom.reset", title: "Default text size", scope: "global", run: () => saveTextSize(DEFAULT_TEXT_SIZE) },
];

/** Every window registers every command, so Settings can list them; a scope keeps a key to its window. */
export function installCommands(windowName: WindowName = currentWindow) {
  currentWindow = windowName;
  registerCoreScopes();
  if (allCommands().length === 0) registerCommands(APP_COMMANDS);
  setOverrides(useStore.getState().keybindings);
}

/** The key label of a command (`⌘K`), or "" when unbound; follows the user's rebinding. */
export function useBindingLabel(id: string): string {
  const custom = useStore((s) => s.keybindings);
  return bindingLabel(bindingFor(id, custom));
}

/** Runs an enabled command by id, without the scope and overlay checks of keys and menus. */
export function runCommand(id: string): boolean {
  const c = getCommand(id);
  if (!c || c.when?.() === false) return false;
  void c.run();
  return true;
}

/** While a dialog is open, only these run; other keys belong to the dialog. */
const MODAL_ALLOWED = new Set(["shortcuts.show", "zoom.in", "zoom.out", "zoom.reset"]);

/** Commands that keep firing while their key is held. */
const REPEATABLE = new Set(["zoom.in", "zoom.out"]);

const blockedByOverlay = (id: string) => overlayOpen() && !MODAL_ALLOWED.has(id);

/** A context menu takes its own keys, like a dialog. */
const inMenu = (target: EventTarget | null) => target instanceof Element && !!target.closest('[role="menu"]');

/** ↵ and Space activate the focused button or link rather than a command. */
function activatesControl(e: KeyboardEvent): boolean {
  if (e.metaKey || e.ctrlKey || e.altKey || e.shiftKey || (e.key !== "Enter" && e.key !== " ")) return false;
  return e.target instanceof Element && !!e.target.closest('button, a[href], [role="button"], [role="option"]');
}

/** Capture-phase keydown handler for the window. */
export function handleKeydown(e: KeyboardEvent) {
  if (isRecordingTarget(e.target) || activatesControl(e)) return;
  const c = resolveKeydown(e, activeScopes());
  if (!c || blockedByOverlay(c.id) || (inMenu(e.target) && !MODAL_ALLOWED.has(c.id))) return;
  e.preventDefault();
  e.stopPropagation();
  // A held key is swallowed rather than passed on, so the native menu accelerator does not repeat it either.
  if (e.repeat && !REPEATABLE.has(c.id)) return;
  void c.run();
}
