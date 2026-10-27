import React from "react";
import ReactDOM from "react-dom/client";
import { api, toAppError } from "../../api";
import { ErrorBoundary } from "../../components/ErrorBoundary";
import { installNativeMenuGuard } from "../../lib/nativeMenu";
import { applyInitialLook } from "../../lib/theme";
import "../../styles";
import "./railtip.css";
import { RailTipApp } from "./RailTipApp";

applyInitialLook();
installNativeMenuGuard();
// This window has no toasts: its errors go to the log.
const logTipError = (message: string) => void api.logError(`Rail tooltip: ${message}`).catch(() => {});
window.addEventListener("error", (e) => logTipError(e.message));
window.addEventListener("unhandledrejection", (e) => logTipError(toAppError(e.reason).message));

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <RailTipApp />
    </ErrorBoundary>
  </React.StrictMode>,
);
