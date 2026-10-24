import { useStore } from "../state/store";
import { Modal } from "./Modal";
import { PromptDialog } from "./PromptDialog";
import { SettingsDialog } from "./settings/SettingsDialog";
import { ShortcutsDialog } from "./ShortcutsDialog";

function ConfirmDialog({ title, message, confirmLabel, cancelLabel = "Cancel", onConfirm }: { title: string; message: string; confirmLabel: string; cancelLabel?: string; onConfirm: () => void }) {
  const close = useStore((s) => s.closeDialog);
  return (
    <Modal onClose={close} labelledBy="confirm-title">
      <h2 id="confirm-title">{title}</h2>
      <p className="muted">{message}</p>
      <div className="actions">
        <button className="btn" onClick={close}>{cancelLabel}</button>
        <button
          className="btn btn-danger"
          autoFocus
          onClick={() => {
            close();
            onConfirm();
          }}
        >
          {confirmLabel}
        </button>
      </div>
    </Modal>
  );
}

export function Dialogs() {
  const dialog = useStore((s) => s.dialog);
  switch (dialog?.type) {
    case "settings":
      return <SettingsDialog />;
    case "shortcuts":
      return <ShortcutsDialog scopes={dialog.scopes} />;
    case "confirm":
      return <ConfirmDialog {...dialog} />;
    case "prompt":
      return <PromptDialog {...dialog} />;
    default:
      return null;
  }
}
