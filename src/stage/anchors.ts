/**
 * The anchors Stage sees: elements marked `data-stage="<id>"`, with `data-stage-label` (on the element or
 * inside it), `data-stage-role` (else inferred from the element or its only control) and
 * `data-stage-list` (on the row or a container); the states come from the usual aria attributes.
 */
import { isInView, visiblePart } from "./rect";
import { isStageRole, type StagePageAnchor, type StageRect, type StageRole } from "./types";

const MAX_LABEL = 80;
export const INTERACTIVE =
  "button, a[href], input, select, textarea, summary, [role='button'], [role='tab'], [role='radio'], [role='switch'], [role='checkbox'], [role='option'], [role='menuitem'], [tabindex]:not([tabindex='-1'])";
export const FIELD = "input:not([type=hidden]):not([type=checkbox]):not([type=radio]), textarea, [contenteditable='true'], [contenteditable='']";

function text(value: string | null | undefined): string {
  return (value ?? "").replace(/\s+/g, " ").trim();
}

export function rectOf(element: Element): StageRect {
  const rect = element.getBoundingClientRect();
  return { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
}

function hasSize(rect: StageRect): boolean {
  return rect.width > 0 && rect.height > 0;
}

function labelOf(element: Element): string {
  const inner = element.querySelector("[data-stage-label]");
  const label =
    text(element.getAttribute("data-stage-label")) ||
    text(inner?.getAttribute("data-stage-label")) ||
    text(element.getAttribute("aria-label")) ||
    text(element.getAttribute("title")) ||
    text(element.textContent);
  return label.length > MAX_LABEL ? `${label.slice(0, MAX_LABEL - 1)}…` : label;
}

/** The scroll areas around an element (innermost first, as padding boxes), then the window. */
export function clipsOf(element: Element): StageRect[] {
  const clips: StageRect[] = [];
  for (let parent = element.parentElement; parent && parent !== document.body && parent !== document.documentElement; parent = parent.parentElement) {
    const style = getComputedStyle(parent);
    if (style.overflowX === "visible" && style.overflowY === "visible") continue;
    if (parent.clientWidth === 0 && parent.clientHeight === 0) continue;
    const box = parent.getBoundingClientRect();
    clips.push({ x: box.x + parent.clientLeft, y: box.y + parent.clientTop, width: parent.clientWidth, height: parent.clientHeight });
  }
  clips.push({ x: 0, y: 0, width: window.innerWidth, height: window.innerHeight });
  return clips;
}

/** The control an anchor stands for: itself, or the only control inside a wrapper. */
function controlOf(element: Element): Element {
  const selector = `${INTERACTIVE}, ${FIELD}`;
  if (element.matches(selector)) return element;
  const inner = element.querySelectorAll(selector);
  return inner.length === 1 ? inner[0] : element;
}

function roleOf(element: Element, control: Element): StageRole | undefined {
  const explicit = element.getAttribute("data-stage-role");
  if (isStageRole(explicit)) return explicit;
  const role = control.getAttribute("role");
  if (role === "switch" || role === "checkbox" || (control instanceof HTMLInputElement && control.type === "checkbox")) return "toggle";
  if (control.matches(FIELD) || role === "textbox" || role === "combobox" || role === "searchbox") return "field";
  if (role === "tab") return "tab";
  if (role === "option" || role === "menuitem" || role === "radio") return "item";
  if (control.matches("a[href]")) return "link";
  if (control.matches("button, [role='button'], summary")) return "button";
  return undefined;
}

/** An aria-style attribute as a flag: absent is undefined, "false" false, anything else true. */
function flag(element: Element, name: string): boolean | undefined {
  const value = element.getAttribute(name);
  return value === null ? undefined : value !== "false";
}

/** `data-state` as a flag: true for an `on` value, false for an `off` one, else undefined. */
function dataState(state: string | null, on: string[], off: string[]): boolean | undefined {
  if (state === null) return undefined;
  if (on.includes(state)) return true;
  return off.includes(state) ? false : undefined;
}

function statesOf(control: Element): Pick<StagePageAnchor, "selected" | "checked" | "expanded" | "disabled"> {
  const state = control.getAttribute("data-state");
  const choice = control instanceof HTMLInputElement && (control.type === "checkbox" || control.type === "radio");
  return {
    selected:
      flag(control, "aria-selected") ??
      flag(control, "aria-current") ??
      flag(control, "data-selected") ??
      dataState(state, ["active"], ["inactive"]),
    checked:
      flag(control, "aria-checked") ??
      flag(control, "aria-pressed") ??
      (choice ? control.checked : undefined) ??
      dataState(state, ["checked", "on"], ["unchecked", "off"]),
    expanded: flag(control, "aria-expanded"),
    disabled: control.matches(":disabled") || control.getAttribute("aria-disabled") === "true" ? true : undefined,
  };
}

/** Every marked element with a size, the first one per id, in document order. */
export function collectAnchors(root: Document | Element = document): StagePageAnchor[] {
  const seen = new Set<string>();
  const anchors: StagePageAnchor[] = [];
  for (const element of root.querySelectorAll("[data-stage]")) {
    const id = element.getAttribute("data-stage")?.trim();
    if (!id || seen.has(id)) continue;
    const rect = rectOf(element);
    if (!hasSize(rect)) continue;
    seen.add(id);
    const clips = clipsOf(element);
    const control = controlOf(element);
    const anchor: StagePageAnchor = { id, label: labelOf(element), rect, inView: isInView(rect, clips), visible: visiblePart(rect, clips) };
    const role = roleOf(element, control);
    if (role) anchor.role = role;
    const list = element.closest("[data-stage-list]")?.getAttribute("data-stage-list");
    if (list) anchor.list = list;
    anchors.push({ ...anchor, ...statesOf(control) });
  }
  return anchors;
}

/** The first element marked `id`, when it has a size (a later sized duplicate is not looked for). */
export function findAnchor(id: string): HTMLElement | null {
  const element = document.querySelector<HTMLElement>(`[data-stage="${CSS.escape(id)}"]`);
  return element && hasSize(rectOf(element)) ? element : null;
}
