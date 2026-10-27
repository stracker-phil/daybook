import React from "react";
import ReactDOM from "react-dom/client";
import { ErrorBoundary } from "../../components/ErrorBoundary";
import { installNativeMenuGuard } from "../../lib/nativeMenu";
import { applyInitialLook } from "../../lib/theme";
import { installModules } from "../../modules";
import { useStore } from "../../state/store";
import "../../styles";
import { SettingsApp } from "./SettingsApp";
import { installStageHooks } from "../../stage/hooks";

installModules("settings");
applyInitialLook();
void installStageHooks("settings");
installNativeMenuGuard();

if (import.meta.env.VITE_DEV_HOOKS === "1") window.daybook = useStore;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <SettingsApp />
    </ErrorBoundary>
  </React.StrictMode>,
);
