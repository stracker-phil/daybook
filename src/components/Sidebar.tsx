import { Settings } from "lucide-react";
import { api } from "../api";

/** The always-dark sidebar: the app's name, and the Settings gear at the bottom. */
export function Sidebar() {
  return (
    <aside className="sidebar" aria-label="Sidebar">
      <div className="sidebar-titlebar" data-tauri-drag-region />
      <div className="sidebar-body">
        <div className="sidebar-app-name">Daybook</div>
      </div>
      <footer className="sidebar-footer">
        <button
          type="button"
          className="icon-btn"
          aria-label="Settings"
          title="Settings (⌘,)"
          onClick={() => void api.runCommand("app:settings")}
        >
          <Settings size="1.077rem" aria-hidden="true" />
        </button>
      </footer>
    </aside>
  );
}
