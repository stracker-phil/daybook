import { useStore } from "../state/store";
import { ShortcutsDialog } from "./ShortcutsDialog";

export function Dialogs() {
  const dialog = useStore((s) => s.dialog);
  return dialog?.type === "shortcuts" ? <ShortcutsDialog scopes={dialog.scopes} /> : null;
}
