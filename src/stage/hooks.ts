/**
 * A page's side of the Stage hook (src-tauri/src/stage, docs/stage.md): it answers Rust's
 * `stage:collect` with its anchors and meta and `stage:act` by revealing, clicking, typing into or
 * scrolling an anchor, or pressing keys. Nothing here runs unless Rust asks.
 */
import { api } from "../api";
import { ActError, click, nextFrame, press, reveal, scroll, typeInto } from "./actions";
import { clipsOf, collectAnchors, findAnchor, rectOf } from "./anchors";
import { currentMeta } from "./meta";
import { visiblePart } from "./rect";
import type { StageAct, StageActReply, StageActRequest, StagePage } from "./types";

type Outcome = Omit<StageActReply, "request" | "page">;
type AnchorAct = Exclude<StageAct, { kind: "keys" }>;
type KeysAct = Extract<StageAct, { kind: "keys" }>;

const NOT_HANDLED: Outcome = { handled: false, ok: false };

async function onKeys(page: StagePage, act: KeysAct): Promise<Outcome> {
  // The page Rust focused for the keys answers even before the focus arrives.
  const mine = act.page ? act.page === page : document.hasFocus();
  if (!mine) return NOT_HANDLED;
  press(act.keys);
  await nextFrame();
  return { handled: true, ok: true };
}

async function onAnchor(act: AnchorAct): Promise<Outcome> {
  const anchor = findAnchor(act.anchor);
  if (!anchor) return NOT_HANDLED;
  if (act.kind === "scroll") {
    await scroll(anchor, act);
    return { handled: true, ok: true, rect: rectOf(anchor) };
  }
  await reveal(anchor);
  // Measured before acting: a click may close or replace what it was on.
  const rect = rectOf(anchor);
  const visible = visiblePart(rect, clipsOf(anchor));
  if (!visible) throw new ActError("is out of view and couldn't be scrolled into view", 409);
  if (act.kind === "click") click(anchor, visible);
  else if (act.kind === "type") typeInto(anchor, act.text, act.mode);
  if (act.kind !== "reveal") await nextFrame();
  return { handled: true, ok: true, rect, visible };
}

export async function perform(page: StagePage, request: StageActRequest): Promise<StageActReply> {
  const base = { request: request.request, page };
  try {
    const outcome = request.kind === "keys" ? await onKeys(page, request) : await onAnchor(request);
    return { ...base, ...outcome };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    const subject = "anchor" in request ? `“${request.anchor}” ` : "";
    const status = error instanceof ActError ? error.status : 500;
    return { ...base, handled: true, ok: false, error: `${subject}${message}`, status };
  }
}

/** Answers `stage:collect` with this page's anchors and meta. */
export function collect(page: StagePage, request: string): Promise<void> {
  return api.stageAnchors({ request, page, anchors: collectAnchors(), meta: currentMeta() }).catch(() => undefined);
}

/** Request ids this page already acted on: Tauri may deliver one event twice, and a click or an append must not repeat. */
const seen = new Set<string>();

function firstTime(request: string): boolean {
  if (seen.has(request)) return false;
  seen.add(request);
  if (seen.size > 100) seen.delete(seen.values().next().value as string);
  return true;
}

/** Answers `stage:act` with what this page did. */
export function act(page: StagePage, request: StageActRequest): Promise<void> {
  if (!firstTime(request.request)) return Promise.resolve();
  return perform(page, request)
    .then((reply) => api.stageDone(reply))
    .catch(() => undefined);
}

/** Call once per window, before the first render; resolves to the listeners' unlisten functions. */
export function installStageHooks(page: StagePage) {
  return Promise.all([api.onStageCollect((request) => void collect(page, request)), api.onStageAct((request) => void act(page, request))]);
}
