import React from "react";
import ReactDOM from "react-dom/client";
import { api, toAppError } from "../../api";
import { ErrorBoundary } from "../../components/ErrorBoundary";
import { installNativeMenuGuard } from "../../lib/nativeMenu";
import { applyInitialLook } from "../../lib/theme";
import "../../styles";
import "./quicknotes.css";
import { QuickNotesApp } from "./QuickNotesApp";
import { installStageHooks } from "../../stage/hooks";

applyInitialLook();
void installStageHooks("quick-notes");
installNativeMenuGuard();
// This window has no toasts: its errors go to the log.
const logEditorError = (message: string) => void api.logError(`Quick notes: ${message}`).catch(() => {});
window.addEventListener("error", (e) => logEditorError(e.message));
window.addEventListener("unhandledrejection", (e) => logEditorError(toAppError(e.reason).message));

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <QuickNotesApp />
    </ErrorBoundary>
  </React.StrictMode>,
);
