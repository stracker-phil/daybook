import { useEffect, useState } from "react";

/** A value Rust pushes: read once when the page loads, then followed through its event. */
export function usePushed<T>(load: () => Promise<T>, subscribe: (handler: (payload: T) => void) => Promise<() => void>, initial: T): T {
  const [value, setValue] = useState<T>(initial);
  useEffect(() => {
    let current = true;
    void load().then((v) => current && setValue(v), () => {});
    const unlisten = subscribe(setValue);
    return () => {
      current = false;
      void unlisten.then((fn) => fn()).catch(() => {});
    };
  }, [load, subscribe]);
  return value;
}
