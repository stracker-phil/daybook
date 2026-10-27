import { type RefObject, useEffect, useRef, useState } from "react";
import { api } from "../../api";

/** The largest and smallest text size while presenting, in CSS px (points on macOS). */
export interface PresentationFont {
  max: number;
  min: number;
}

export const DEFAULT_PRESENTATION_FONT: PresentationFont = { max: 156, min: 42 };

/** The largest whole size in `[min, max]` that `fits`, else `min`; assumes smaller sizes fit when larger ones do. */
export function fitFontSize(font: PresentationFont, fits: (size: number) => boolean): number {
  let low = Math.ceil(font.min);
  let high = Math.floor(font.max);
  if (high <= low) return low;
  let best = low;
  while (low <= high) {
    const mid = Math.floor((low + high) / 2);
    if (fits(mid)) {
      best = mid;
      low = mid + 1;
    } else {
      high = mid - 1;
    }
  }
  return best;
}

/** Whether Rust presents the editor: asked once on load, then followed through `quick_note_presentation`. */
export function usePresenting(): boolean {
  const [presenting, setPresenting] = useState(false);
  useEffect(() => {
    let heard = false;
    void api.quickNotesPresentation().then(
      (state) => {
        if (!heard) setPresenting(state.presenting);
      },
      () => {},
    );
    const unlisten = api.onQuickNotePresentation((state) => {
      heard = true;
      setPresenting(state.presenting);
    });
    return () => void unlisten.then((fn) => fn()).catch(() => {});
  }, []);
  return presenting;
}

/** The mirror's text: a trailing newline still counts as a line, as in the editor. */
const mirrorText = (text: string) => `${text}\u200b`;

/** Counts the size changes of `box` while `active`. */
function useResizeTick(box: RefObject<HTMLElement | null>, active: boolean): number {
  const [tick, setTick] = useState(0);
  useEffect(() => {
    if (!active || !box.current || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(() => setTick((n) => n + 1));
    observer.observe(box.current);
    return () => observer.disconnect();
  }, [box, active]);
  return tick;
}

/** Fits `text` into `box` through `--presentation-font-size`, measuring against the hidden `mirror`; `onFit` runs after each fit. */
export function usePresentationFont(
  { text, active, onFit }: { text: string; active: boolean; onFit?: () => void },
  font: PresentationFont = DEFAULT_PRESENTATION_FONT,
) {
  const box = useRef<HTMLDivElement>(null);
  const mirror = useRef<HTMLDivElement>(null);
  const latestOnFit = useRef(onFit);
  useEffect(() => {
    latestOnFit.current = onFit;
  });
  const resized = useResizeTick(box, active);

  useEffect(() => {
    const target = box.current;
    const copy = mirror.current;
    if (!active || !target || !copy) return;
    const frame = requestAnimationFrame(() => {
      copy.textContent = mirrorText(text);
      let size = fitFontSize(font, (candidate) => {
        copy.style.fontSize = `${candidate}px`;
        return copy.scrollHeight <= copy.clientHeight && copy.scrollWidth <= copy.clientWidth;
      });
      const apply = () => target.style.setProperty("--presentation-font-size", `${size}px`);
      apply();
      // CodeMirror wraps a little differently from the mirror, so the editor itself has the last word.
      const editor = target.querySelector<HTMLElement>(".cm-scroller");
      while (editor && size > font.min && editor.scrollHeight > editor.clientHeight) {
        size -= 1;
        apply();
      }
      latestOnFit.current?.();
    });
    return () => cancelAnimationFrame(frame);
  }, [active, text, resized, font]);

  return { box, mirror };
}
