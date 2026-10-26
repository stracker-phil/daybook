import { useState } from "react";
import { api, toAppError } from "../../../api";

async function copiedLink(path: string): Promise<string> {
  const link = await api.searchLink(path);
  try {
    await navigator.clipboard.writeText(link);
  } catch {
    return "Clipboard is not available";
  }
  return `Copied ${link}`;
}

/** Opening a note and copying its link, with the footer's status: what was done or what failed. */
export function useResultActions() {
  const [status, setStatus] = useState<string | null>(null);

  const report = (action: Promise<string | void>) =>
    void action.then(
      (done) => {
        if (done) setStatus(done);
      },
      (e) => setStatus(toAppError(e).message),
    );

  return {
    status,
    setStatus,
    open: (path: string) => report(api.searchOpen(path)),
    copyLink: (path: string) => report(copiedLink(path)),
  };
}
