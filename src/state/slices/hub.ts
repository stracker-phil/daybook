import { api, type HubView } from "../../api";
import { fail } from "../requests";
import type { Slice } from "../store";

export interface HubSlice {
  /** What the Daybook window shows; `null` until it loads. */
  hub: HubView | null;
  /** The day shown (`YYYY-MM-DD`); `null` is today, so the window follows midnight. */
  shownDay: string | null;
  loadHub: () => Promise<void>;
  /** Shows another day read-only; `null` returns to today. */
  showDay: (day: string | null) => void;
}

export const hubSlice: Slice<HubSlice> = {
  create: (set, get) => ({
    hub: null,
    shownDay: null,

    async loadHub() {
      const day = get().shownDay;
      try {
        const hub = await api.hubView(day ?? undefined);
        // A slower load for a day no longer shown must not replace the newer one. Today's date shows today.
        if (get().shownDay === day) set({ hub, shownDay: hub.isToday ? null : day });
      } catch (e) {
        fail(e, "Could not load the Daybook window");
      }
    },

    showDay(day) {
      set({ shownDay: day });
      void get().loadHub();
    },
  }),
};
