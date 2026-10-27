import { api, toAppError } from "../api";
import { useStore } from "./store";

/** Shows an error toast (which also logs it) and returns the error. */
export function fail(e: unknown, context: string) {
  const err = toAppError(e);
  useStore.getState().toast("error", `${context}: ${err.message}`);
  return err;
}

/** Runs a command of the Rust command bus; a failure shows as `failure: <reason>`. */
export function runBusCommand(id: string, failure: string) {
  void api.runCommand(id).catch((e) => fail(e, failure));
}
