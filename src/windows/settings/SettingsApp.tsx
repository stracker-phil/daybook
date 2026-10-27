import { useEffect, useState } from "react";
import { api, toAppError } from "../../api";
import { Dialogs } from "../../components/Dialogs";
import { SettingsWindow } from "../../components/settings/SettingsWindow";
import { Toasts } from "../../components/Toasts";
import { handleKeydown, installCommands } from "../../commands";
import { applyAppearance, useSystemDark } from "../../lib/theme";
import { trackWindowFocus } from "../../lib/windowFocus";
import { useContextColor } from "../../lib/useContextColor";
import { useStore } from "../../state/store";

/** The page Rust opened the window at (`?page=`); later pages arrive as `open_settings`. */
function initialPage(): string | undefined {
  return new URLSearchParams(window.location.search).get("page") ?? undefined;
}

/** Loads the settings, follows saves from the other windows and the pages Rust asks for, and routes the keys. */
function useSettingsEvents(setPage: (page: string) => void) {
  useEffect(() => {
    const { loadSettings, applySettings, applyKeybindings } = useStore.getState();
    void loadSettings().then(() => installCommands("settings"));
    const unlisten = [
      api.onOpenSettings(setPage),
      api.onSettingsChanged(applySettings),
      api.onKeybindingsChanged(applyKeybindings),
    ];
    window.addEventListener("keydown", handleKeydown, true);
    return () => {
      unlisten.forEach((p) => void p.then((fn) => fn()).catch(() => {}));
      window.removeEventListener("keydown", handleKeydown, true);
    };
  }, [setPage]);
}

function useErrorToasts() {
  useEffect(() => {
    const report = (message: string) => useStore.getState().toast("error", `Unexpected error: ${message}`);
    const onError = (e: ErrorEvent) => report(e.message);
    const onRejection = (e: PromiseRejectionEvent) => report(toAppError(e.reason).message);
    window.addEventListener("error", onError);
    window.addEventListener("unhandledrejection", onRejection);
    return () => {
      window.removeEventListener("error", onError);
      window.removeEventListener("unhandledrejection", onRejection);
    };
  }, []);
}

export function SettingsApp() {
  const [page, setPage] = useState(initialPage);
  const settings = useStore((s) => s.settings);
  const systemDark = useSystemDark();
  useContextColor();
  useSettingsEvents(setPage);
  useErrorToasts();
  useEffect(trackWindowFocus, []);

  useEffect(() => {
    if (settings) applyAppearance(settings, systemDark);
  }, [settings, systemDark]);

  return (
    <>
      <SettingsWindow page={page} onPage={setPage} />
      <Dialogs />
      <Toasts />
    </>
  );
}
