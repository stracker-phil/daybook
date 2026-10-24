import { api, type HubView } from "../../api";
import { fail } from "../requests";
import type { Slice } from "../store";

export interface HubSlice {
  /** What the Daybook window shows; `null` until it loads. */
  hub: HubView | null;
  loadHub: () => Promise<void>;
}

export const hubSlice: Slice<HubSlice> = {
  create: (set) => ({
    hub: null,

    async loadHub() {
      try {
        set({ hub: await api.hubView() });
      } catch (e) {
        fail(e, "Could not load the Daybook window");
      }
    },
  }),
};
