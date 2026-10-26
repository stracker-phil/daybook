import { useEffect, useRef } from "react";

/** `index` moved by `step` in a list of `count`, wrapping at both ends. */
export const cycle = (index: number, step: number, count: number) => (index + step + count) % count;

/** A ref for the highlighted row that scrolls it into view whenever `active` changes. */
export function useActiveScroll<T extends HTMLElement>(active: unknown) {
  const ref = useRef<T>(null);
  useEffect(() => {
    // jsdom has no scrollIntoView.
    ref.current?.scrollIntoView?.({ block: "nearest" });
  }, [active]);
  return ref;
}
