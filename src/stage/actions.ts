/** What a page does to an anchor when Rust asks: reveal, click, type, scroll, keys. */
import { FIELD, INTERACTIVE, clipsOf, rectOf } from "./anchors";
import { centerOf, isInView } from "./rect";
import type { StageKeys, StageRect, StageScrollTo, StageTypeMode } from "./types";

/** A smooth scroll is over once the anchor stops moving, or after this long. */
const SCROLL_SETTLE_MS = 800;
const SCROLL_POLL_MS = 50;
/** Hidden windows don't render frames; answer after this long instead. */
const FRAME_FALLBACK_MS = 50;
// What `focus()` lands on, unlike INTERACTIVE: any tabindex and editors, but no bare ARIA roles.
const FOCUSABLE = "button, a[href], input, select, textarea, summary, [tabindex], [contenteditable='true'], [contenteditable='']";

/** An action that can't be done: 409 when it can't be done now, 500 when it failed. */
export class ActError extends Error {
  constructor(
    message: string,
    readonly status = 500,
  ) {
    super(message);
  }
}

const pause = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

/** One rendered frame, so an immediate `describe` sees what the action changed. */
export function nextFrame(): Promise<void> {
  return new Promise((resolve) => {
    const timer = setTimeout(resolve, FRAME_FALLBACK_MS);
    requestAnimationFrame(() => {
      clearTimeout(timer);
      resolve();
    });
  });
}

/** Polls a position until two readings match, or the time is up. */
async function settle(position: () => string): Promise<void> {
  const started = Date.now();
  let last = "";
  while (Date.now() - started < SCROLL_SETTLE_MS) {
    await pause(SCROLL_POLL_MS);
    const now = position();
    if (now === last) return;
    last = now;
  }
}

/** Scrolls the anchor into view as a presenter would (smoothly) and waits until it stops moving. */
export async function reveal(anchor: HTMLElement): Promise<void> {
  const rect = rectOf(anchor);
  const clips = clipsOf(anchor);
  if (isInView(rect, clips)) return;
  // Taller than its scroll area: its top comes into view.
  const tall = rect.height > clips[0].height;
  anchor.scrollIntoView({ behavior: "smooth", block: tall ? "start" : "nearest", inline: "nearest" });
  await settle(() => {
    const now = rectOf(anchor);
    return `${now.x},${now.y}`;
  });
}

/** A real click's events at the centre of the visible part, on the deepest element there. */
export function click(anchor: HTMLElement, visible: StageRect): void {
  const point = centerOf(visible);
  const hit = document.elementFromPoint(point.x, point.y);
  const target = hit && anchor.contains(hit) ? hit : (anchor.querySelector(INTERACTIVE) ?? anchor);
  if (target.closest(":disabled, [aria-disabled='true']")) throw new ActError("is disabled", 409);
  const init = {
    bubbles: true,
    cancelable: true,
    composed: true,
    clientX: point.x,
    clientY: point.y,
    screenX: window.screenX + point.x,
    screenY: window.screenY + point.y,
    button: 0,
  };
  const pointer = (type: string, buttons: number) =>
    target.dispatchEvent(new PointerEvent(type, { ...init, buttons, pointerId: 1, pointerType: "mouse", isPrimary: true }));
  const mouse = (type: string, buttons: number) => target.dispatchEvent(new MouseEvent(type, { ...init, buttons, detail: 1 }));

  const pressAllowed = pointer("pointerdown", 1);
  const mouseDown = pressAllowed && mouse("mousedown", 1);
  if (mouseDown) {
    const focusable = target.closest(FOCUSABLE);
    if (focusable instanceof HTMLElement) focusable.focus();
  }
  pointer("pointerup", 0);
  if (pressAllowed) mouse("mouseup", 0);
  mouse("click", 0);
}

function fieldOf(element: HTMLElement): HTMLElement | null {
  if (element.matches(FIELD) || element.isContentEditable) return element;
  return element.querySelector<HTMLElement>(FIELD);
}

/** The new value through the native setter (React sees an `input` event), the caret at the end. */
function typeIntoInput(field: HTMLInputElement | HTMLTextAreaElement, value: string, mode: StageTypeMode): void {
  const next = mode === "append" ? field.value + value : value;
    // The prototype's setter: React tracks the instance's own value and would miss the change.
    Reflect.set(Object.getPrototypeOf(field) as object, "value", next, field);
    try {
      field.setSelectionRange(next.length, next.length);
    } catch {
      // Inputs without a caret (number, email) keep theirs.
    }
    field.dispatchEvent(new Event("input", { bubbles: true }));
}

/** The text typed at the caret (as CodeMirror expects), over everything for `replace`. */
function typeIntoEditor(field: HTMLElement, value: string, mode: StageTypeMode): void {
  const selection = window.getSelection();
  const caretInside = !!selection?.anchorNode && field.contains(selection.anchorNode);
  if (selection && (mode === "replace" || !caretInside)) {
    const range = document.createRange();
    range.selectNodeContents(field);
    if (mode === "append") range.collapse(false);
    selection.removeAllRanges();
    selection.addRange(range);
  }
  if (value) document.execCommand("insertText", false, value);
  else document.execCommand("delete");
}

export function typeInto(anchor: HTMLElement, value: string, mode: StageTypeMode): void {
  const field = fieldOf(anchor);
  if (!field) throw new ActError("isn't a text field", 409);
  field.focus();
  if (field instanceof HTMLInputElement || field instanceof HTMLTextAreaElement) typeIntoInput(field, value, mode);
  else typeIntoEditor(field, value, mode);
}

function scrollsVertically(element: Element): boolean {
  return element.scrollHeight > element.clientHeight + 1 && /(auto|scroll|overlay)/.test(getComputedStyle(element).overflowY);
}

/** The scroll area the anchor is, else one inside it, else the one it is in, else the page. */
function scrollAreaOf(anchor: HTMLElement): Element | null {
  if (scrollsVertically(anchor)) return anchor;
  for (const inner of anchor.querySelectorAll("*")) if (scrollsVertically(inner)) return inner;
  for (let parent = anchor.parentElement; parent; parent = parent.parentElement) if (scrollsVertically(parent)) return parent;
  const root = document.scrollingElement;
  return root && root.scrollHeight > root.clientHeight + 1 ? root : null;
}

/** Scrolls the anchor's scroll area to `to`, else by `by` points, and waits until it stops. */
export async function scroll(anchor: HTMLElement, target: { by?: number; to?: StageScrollTo }): Promise<void> {
  const area = scrollAreaOf(anchor);
  if (!area) throw new ActError("isn't in anything that scrolls", 409);
  if (target.to) area.scrollTo({ top: target.to === "top" ? 0 : area.scrollHeight, behavior: "smooth" });
  else area.scrollBy({ top: target.by ?? 0, behavior: "smooth" });
  await settle(() => String(area.scrollTop));
}

/** keydown and keyup on the focused element, as a typed combination would. */
export function press(keys: StageKeys): void {
  const target = document.activeElement instanceof HTMLElement ? document.activeElement : document.body;
  const init = { key: keys.key, code: keys.code, metaKey: keys.meta, shiftKey: keys.shift, altKey: keys.alt, ctrlKey: keys.ctrl, bubbles: true, cancelable: true };
  target.dispatchEvent(new KeyboardEvent("keydown", init));
  target.dispatchEvent(new KeyboardEvent("keyup", init));
}
