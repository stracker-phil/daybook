import { api, DEFAULT_TEXT_SIZE, TEXT_SIZES, type TextSize } from "../api";
import { fail } from "../state/requests";
import { useStore } from "../state/store";
import { allCommands, bindingFor, bindingLabel, getCommand, isRecordingTarget, registerCommands, resolveKeydown, setOverrides, type Command } from "./registry";
import { activeScopes, registerScope } from "./scopes";

export * from "./registry";
export * from "./scopes";

/** Registers the core scopes; safe to call again (tests and re-installs do). */
export function registerCoreScopes() {
  registerScope({ id: "global", parent: null, label: "Everywhere", isActive: () => true });
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
  const s = useStore.getState();
  return s.dialog !== null || s.paletteOpen;
}

function togglePalette() {
  const s = useStore.getState();
  s.setPaletteOpen(!s.paletteOpen);
}

/** `?` opens the overview of the scopes active now; pressed again it closes it. */
function toggleShortcuts() {
  const s = useStore.getState();
  if (s.dialog?.type === "shortcuts") return s.closeDialog();
  if (!s.dialog) s.openDialog({ type: "shortcuts", scopes: activeScopes() });
}

function toggleLaunchAtLogin() {
  const s = useStore.getState();
  if (s.settings) void s.setLaunchAtLogin(!s.settings.launchAtLogin);
}

function toggleSettings() {
  const s = useStore.getState();
  if (s.dialog?.type === "settings") s.closeDialog();
  else s.openDialog({ type: "settings" });
}

const APP_COMMANDS: Command[] = [
  { id: "settings.toggle", title: "Settings…", scope: "global", defaultBinding: "Mod+,", menu: true, run: toggleSettings },
  { id: "app.launchAtLogin", title: "Launch at login", scope: "global", menu: true, menuOnly: true, when: () => useStore.getState().settings !== null, checked: () => !!useStore.getState().settings?.launchAtLogin, run: toggleLaunchAtLogin },
  { id: "shortcuts.show", title: "Keyboard shortcuts", scope: "global", defaultBinding: "Shift+?", menu: true, run: toggleShortcuts },
  { id: "palette.toggle", title: "Command palette", scope: "global", defaultBinding: "Mod+K", menu: true, run: togglePalette },
  { id: "sidebar.toggle", title: "Toggle sidebar", scope: "global", defaultBinding: "Mod+Ctrl+S", menu: true, run: () => useStore.getState().toggleSidebar() },
  { id: "zoom.in", title: "Larger text", scope: "global", defaultBinding: "Mod+=", run: () => zoomText(1) },
  { id: "zoom.out", title: "Smaller text", scope: "global", defaultBinding: "Mod+-", run: () => zoomText(-1) },
  { id: "zoom.reset", title: "Default text size", scope: "global", defaultBinding: "Mod+0", run: () => saveTextSize(DEFAULT_TEXT_SIZE) },
  { id: "window.hide", title: "Close window", scope: "global", defaultBinding: "Mod+W", menu: true, run: () => void api.hideWindow() },
  { id: "logs.open", title: "Open log folder", scope: "global", menu: true, run: () => void api.openLogFolder() },
  { id: "help.readme", title: "Open README", scope: "global", menu: true, run: () => void api.openReadme().catch((e) => fail(e, "Couldn't open the README")) },
  { id: "app.background", title: "Hide to Menu Bar", scope: "global", defaultBinding: "Mod+Q", menu: true, run: () => void api.hideToMenuBar() },
  { id: "app.quit", title: "Quit Daybook", scope: "global", menu: true, run: () => void api.quitApp() },
];

/**
 * Registers the core scopes and the app's commands (again after `clearCommands`) and syncs user overrides
 * from the store.
 */
export function installCommands() {
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

/** While a dialog or the palette is open, only these run; other keys belong to the overlay. */
const MODAL_ALLOWED = new Set(["settings.toggle", "shortcuts.show", "palette.toggle", "zoom.in", "zoom.out", "zoom.reset", "window.hide", "app.background", "app.quit"]);

/** Commands that keep firing while their key is held. */
const REPEATABLE = new Set(["zoom.in", "zoom.out"]);

const blockedByOverlay = (id: string) => overlayOpen() && !MODAL_ALLOWED.has(id);

/** Whether a native menu item runs its command now: in an active scope, enabled, not held back by an overlay. */
export function canRunFromMenu(c: Command, scopes: string[] = activeScopes()): boolean {
  return scopes.includes(c.scope) && c.when?.() !== false && !blockedByOverlay(c.id);
}

/** Runs a native menu or tray item's command in an active scope. */
export function runMenuCommand(id: string): boolean {
  const target = getCommand(id);
  if (!target || !canRunFromMenu(target)) return false;
  void target.run();
  return true;
}

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
