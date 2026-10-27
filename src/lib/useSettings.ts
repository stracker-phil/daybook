import { api, type Settings } from "../api";
import { usePushed } from "./usePushed";

/** The saved settings, followed while the page lives; `null` until they load. For windows without the store. */
export const useSettings = () => usePushed<Settings | null>(api.getSettings, api.onSettingsChanged, null);
