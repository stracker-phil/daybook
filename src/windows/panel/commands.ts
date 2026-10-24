import { api } from "../../api";
import { activeScopes, registerCommands, registerScope, resolveKeydown } from "../../commands";

/** The overlay's own keys; the window has no other scope. Safe to call again. */
export function installPanelCommands() {
  registerScope({ id: "panel", parent: null, label: "Overlays", isActive: () => true });
  registerCommands([{ id: "panel.close", title: "Close overlay", scope: "panel", defaultBinding: "Escape", run: () => void api.panelClose() }]);
}

/** Keydown handler for the overlay window; a view that handles a key itself calls `preventDefault()`. */
export function handlePanelKeydown(e: KeyboardEvent) {
  if (e.defaultPrevented) return;
  const command = resolveKeydown(e, activeScopes());
  if (!command) return;
  e.preventDefault();
  void command.run();
}
