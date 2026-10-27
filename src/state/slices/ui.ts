import { api } from "../../api";
import type { Slice } from "../store";

export interface Toast {
  id: number;
  kind: "error" | "info" | "success";
  message: string;
  /** How many times the same message was reported while it was showing. */
  count: number;
  /** A button in the toast, e.g. Undo; running it dismisses the toast. */
  action?: ToastAction;
}

export interface ToastAction {
  label: string;
  run: () => void;
  /** Overrides how long the toast stays. */
  ttlMs?: number;
}

/** The shortcut overview of the scopes that were active when it opened. */
export type Dialog = { type: "shortcuts"; scopes: string[] };

export interface UiSlice {
  toasts: Toast[];
  dialog: Dialog | null;
  toast: (kind: Toast["kind"], message: string, action?: ToastAction) => void;
  dismissToast: (id: number) => void;
  openDialog: (dialog: Dialog) => void;
  closeDialog: () => void;
}

let nextToastId = 1;

export const uiSlice: Slice<UiSlice> = {
  create: (set, get) => ({
    toasts: [],
    dialog: null,

    toast(kind, message, action) {
      if (kind === "error") void api.logError(message).catch(() => {});
      const same = get().toasts.find((t) => t.kind === kind && t.message === message);
      if (same) {
        set((s) => ({ toasts: s.toasts.map((t) => (t.id === same.id ? { ...t, count: t.count + 1 } : t)) }));
        return;
      }
      const id = nextToastId++;
      set((s) => ({ toasts: [...s.toasts.slice(-4), { id, kind, message, count: 1, action }] }));
    },

    dismissToast(id) {
      set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) }));
    },

    openDialog(dialog) {
      set({ dialog });
    },

    closeDialog() {
      set({ dialog: null });
    },
  }),
};
