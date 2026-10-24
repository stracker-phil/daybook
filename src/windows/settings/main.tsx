import React from "react";
import ReactDOM from "react-dom/client";
import { ErrorBoundary } from "../../components/ErrorBoundary";
import { installInputModality } from "../../lib/inputModality";
import { installNativeMenuGuard } from "../../lib/nativeMenu";
import { applyInitialLook } from "../../lib/theme";
import { installModules } from "../../modules";
import { useStore } from "../../state/store";
import "../../styles";
import { SettingsApp } from "./SettingsApp";

installModules("settings");
applyInitialLook();
installNativeMenuGuard();
installInputModality();

if (import.meta.env.VITE_DEV_HOOKS === "1") window.daybook = useStore;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <SettingsApp />
    </ErrorBoundary>
  </React.StrictMode>,
);
