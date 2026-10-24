import React from "react";
import ReactDOM from "react-dom/client";
import { ErrorBoundary } from "../../components/ErrorBoundary";
import { api, toAppError } from "../../api";
import { applyInitialLook } from "../../lib/theme";
import "../../styles";
import "./panel.css";
import { PanelApp } from "./PanelApp";

applyInitialLook();
// This window has no toasts: its errors go to the log.
const logOverlayError = (message: string) => void api.logError(`Overlay: ${message}`).catch(() => {});
window.addEventListener("error", (e) => logOverlayError(e.message));
window.addEventListener("unhandledrejection", (e) => logOverlayError(toAppError(e.reason).message));

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <PanelApp />
    </ErrorBoundary>
  </React.StrictMode>,
);
