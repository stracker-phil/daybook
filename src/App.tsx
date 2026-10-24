import { useEffect } from "react";
import { api, toAppError, type LaunchEvent } from "./api";
import { Dialogs } from "./components/Dialogs";
import { EmptyView } from "./components/EmptyView";
import { Sidebar } from "./components/Sidebar";
import { Toasts } from "./components/Toasts";
import { handleKeydown, installCommands } from "./commands";
import { answerRemote } from "./commands/remote";
import { applyAppearance, useSystemDark } from "./lib/theme";
import { flushUiState, useStore } from "./state/store";
import "./styles";

function handleLaunchEvents(events: LaunchEvent[]) {
  const { toast } = useStore.getState();
  for (const event of events) {
    if (event.type === "error") toast("error", event.message);
    if (event.type === "notice") toast("info", event.message);
  }
}

function useBackendEvents() {
  useEffect(() => {
    const { loadSettings, loadUiState, applySettings, toast, openDialog } = useStore.getState();
    const drainLaunches = () => api.takeLaunchEvents().then(handleLaunchEvents, (e) => toast("error", `Launch request failed: ${toAppError(e).message}`));

    // Re-installing applies the user's keybinding overrides, which arrive with the settings.
    void loadSettings().then(() => (installCommands(), drainLaunches()));
    void loadUiState();
    const unlisten = [
      api.onLaunch(() => void drainLaunches()),
      api.onOpenSettings((page) => openDialog({ type: "settings", page: page ?? undefined })),
      api.onRemote((e) => void answerRemote(e)),
      api.onSettingsChanged(applySettings),
    ];

    const onError = (e: ErrorEvent) => toast("error", `Unexpected error: ${e.message}`);
    const onRejection = (e: PromiseRejectionEvent) => toast("error", `Unexpected error: ${toAppError(e.reason).message}`);
    window.addEventListener("keydown", handleKeydown, true);
    window.addEventListener("error", onError);
    window.addEventListener("unhandledrejection", onRejection);
    window.addEventListener("beforeunload", flushUiState);
    return () => {
      unlisten.forEach((p) => void p.then((fn) => fn()).catch(() => {}));
      window.removeEventListener("keydown", handleKeydown, true);
      window.removeEventListener("error", onError);
      window.removeEventListener("unhandledrejection", onRejection);
      window.removeEventListener("beforeunload", flushUiState);
    };
  }, []);
}

/** Hiding the sidebar removes it entirely; `main` keeps its tree position so views keep their state. */
function Workspace() {
  const sidebarHidden = useStore((s) => s.uiState.sidebarHidden);
  return (
    <div className={sidebarHidden ? "workspace no-sidebar" : "workspace"}>
      {!sidebarHidden && <Sidebar />}
      <main className="main">
        <EmptyView />
      </main>
    </div>
  );
}

export default function App() {
  const settings = useStore((s) => s.settings);
  const systemDark = useSystemDark();
  useBackendEvents();

  useEffect(() => {
    if (settings) applyAppearance(settings, systemDark);
  }, [settings, systemDark]);

  return (
    <>
      <div className="app">
        <Workspace />
      </div>
      <Dialogs />
      <Toasts />
    </>
  );
}
