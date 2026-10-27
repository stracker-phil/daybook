import React from "react";
import ReactDOM from "react-dom/client";
import { api, toAppError } from "../../api";
import { ErrorBoundary } from "../../components/ErrorBoundary";
import { installNativeMenuGuard } from "../../lib/nativeMenu";
import { applyInitialLook } from "../../lib/theme";
import "../../styles";
import "./rail.css";
import { RailApp } from "./RailApp";
import { installStageHooks } from "../../stage/hooks";

applyInitialLook();
void installStageHooks("rail");
installNativeMenuGuard();
// This window has no toasts: its errors go to the log.
const logRailError = (message: string) => void api.logError(`Rail: ${message}`).catch(() => {});
window.addEventListener("error", (e) => logRailError(e.message));
window.addEventListener("unhandledrejection", (e) => logRailError(toAppError(e.reason).message));

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <RailApp />
    </ErrorBoundary>
  </React.StrictMode>,
);
