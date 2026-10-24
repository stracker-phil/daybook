import { useEffect } from "react";
import { api, toAppError, type LaunchEvent } from "./api";
import { Dialogs } from "./components/Dialogs";
import { Board } from "./components/hub/Board";
import { Inspector } from "./components/hub/Inspector";
import { Sidebar } from "./components/Sidebar";
import { Toasts } from "./components/Toasts";
import { handleKeydown, installCommands } from "./commands";
import { answerRemote } from "./commands/remote";
import { applyAppearance, useSystemDark } from "./lib/theme";
import { useContextColor } from "./lib/useContextColor";
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
    const { loadSettings, loadUiState, loadHub, applySettings, applyKeybindings, toggleInspector, toast } = useStore.getState();
    const drainLaunches = () => api.takeLaunchEvents().then(handleLaunchEvents, (e) => toast("error", `Launch request failed: ${toAppError(e).message}`));

    // Re-installing applies the user's keybinding overrides, which arrive with the settings.
    void loadSettings().then(() => (installCommands("main"), drainLaunches()));
    void loadUiState();
    void loadHub();
    const reloadHub = () => void loadHub();
    // The greeting and the day follow the clock.
    const everyMinute = window.setInterval(reloadHub, 60_000);
    const unlisten = [
      api.onLaunch(() => void drainLaunches()),
      api.onRemote((e) => void answerRemote(e)),
      api.onSettingsChanged((settings) => (applySettings(settings), reloadHub())),
      api.onKeybindingsChanged(applyKeybindings),
      api.onToggleInspector(toggleInspector),
      api.onContextChanged(reloadHub),
      api.onStatusChanged(reloadHub),
      api.onNoteChanged(reloadHub),
      api.onCalendarChanged(reloadHub),
      api.onCallChanged(reloadHub),
      api.onWindowOpened(reloadHub),
    ];

    const onError = (e: ErrorEvent) => toast("error", `Unexpected error: ${e.message}`);
    const onRejection = (e: PromiseRejectionEvent) => toast("error", `Unexpected error: ${toAppError(e.reason).message}`);
    window.addEventListener("keydown", handleKeydown, true);
    window.addEventListener("error", onError);
    window.addEventListener("unhandledrejection", onRejection);
    window.addEventListener("beforeunload", flushUiState);
    return () => {
      window.clearInterval(everyMinute);
      unlisten.forEach((p) => void p.then((fn) => fn()).catch(() => {}));
      window.removeEventListener("keydown", handleKeydown, true);
      window.removeEventListener("error", onError);
      window.removeEventListener("unhandledrejection", onRejection);
      window.removeEventListener("beforeunload", flushUiState);
    };
  }, []);
}

/** Hiding the sidebar or the inspector removes it; `main` keeps its tree position so the board keeps its state. */
function Workspace() {
  const sidebarHidden = useStore((s) => s.uiState.sidebarHidden);
  const inspectorHidden = useStore((s) => s.uiState.inspectorHidden);
  const className = ["workspace", sidebarHidden && "no-sidebar", inspectorHidden && "no-inspector"].filter(Boolean).join(" ");
  return (
    <div className={className}>
      {!sidebarHidden && <Sidebar />}
      <main className="main">
        <Board />
      </main>
      {!inspectorHidden && <Inspector />}
    </div>
  );
}

export default function App() {
  const settings = useStore((s) => s.settings);
  const systemDark = useSystemDark();
  useBackendEvents();
  useContextColor();

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
