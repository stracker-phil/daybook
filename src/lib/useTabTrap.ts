import { useEffect, type RefObject } from "react";

const TABBABLE = [
  "input:not([disabled]):not([type=hidden])",
  "textarea:not([disabled])",
  "select:not([disabled])",
  "button:not([disabled])",
  "a[href]",
  '[tabindex]:not([tabindex="-1"])',
].join(",");

function isVisible(element: HTMLElement): boolean {
  return element.getClientRects().length > 0 && getComputedStyle(element).visibility !== "hidden";
}

function nextStop(index: number, count: number, backwards: boolean): number {
  if (index < 0) return backwards ? count - 1 : 0;
  return (index + (backwards ? -1 : 1) + count) % count;
}

/**
 * Tab and ⇧Tab cycle through the focusable elements of `root` (the whole page without one) and never
 * leave it: leaving the web content moves focus out of the window, and an overlay that loses focus closes.
 * A view that uses Tab itself calls `preventDefault()` first.
 */
export function useTabTrap(root?: RefObject<HTMLElement | null>): void {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Tab" || event.defaultPrevented || event.metaKey || event.ctrlKey || event.altKey) return;
      event.preventDefault();
      const scope = root?.current ?? document.body;
      const items = [...scope.querySelectorAll<HTMLElement>(TABBABLE)].filter(isVisible);
      if (items.length === 0) return;
      const index = items.indexOf(document.activeElement as HTMLElement);
      items[nextStop(index, items.length, event.shiftKey)].focus();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [root]);
}
