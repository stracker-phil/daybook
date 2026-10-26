import { useCallback, useEffect, useRef } from "react";
import { toAppError } from "../api";

interface Autosave<T> {
  /** A new draft: saved after `delayMs` without further changes. */
  change: (value: T) => void;
  /** Saves a dirty draft now; resolves once the save is through. */
  flush: () => Promise<void>;
  isDirty: () => boolean;
}

/**
 * Saves a draft a moment after the last change. Saves run one after another; a failed save keeps the
 * draft dirty, so the next change or flush tries again.
 */
export function useAutosave<T>({ delayMs, save, onError }: { delayMs: number; save: (value: T) => Promise<void>; onError: (message: string) => void }): Autosave<T> {
  const latest = useRef<T | null>(null);
  const dirty = useRef(false);
  const timer = useRef<number | undefined>(undefined);
  const chain = useRef<Promise<void>>(Promise.resolve());
  const handlers = useRef({ save, onError });
  useEffect(() => {
    handlers.current = { save, onError };
  });

  const flush = useCallback((): Promise<void> => {
    window.clearTimeout(timer.current);
    const draft = latest.current;
    if (!dirty.current || draft === null) return chain.current;
    dirty.current = false;
    chain.current = chain.current.then(async () => {
      try {
        await handlers.current.save(draft);
      } catch (e) {
        dirty.current = true;
        handlers.current.onError(toAppError(e).message);
      }
    });
    return chain.current;
  }, []);

  const change = useCallback(
    (value: T) => {
      latest.current = value;
      dirty.current = true;
      window.clearTimeout(timer.current);
      timer.current = window.setTimeout(() => void flush(), delayMs);
    },
    [delayMs, flush],
  );

  const isDirty = useCallback(() => dirty.current, []);

  return { change, flush, isDirty };
}
