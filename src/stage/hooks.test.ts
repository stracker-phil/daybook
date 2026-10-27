import { renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api } from "../api";
import { installStageHooks, perform } from "./hooks";
import { useStageMeta } from "./meta";
import type { StageActRequest, StageKeys } from "./types";

const mocked = vi.mocked(api, true);
const RECT = { x: 10, y: 20, width: 100, height: 30 };
const ZERO = { x: 0, y: 0, width: 0, height: 0 };
const ENTER: StageKeys = { key: "Enter", code: "Enter", meta: false, shift: false, alt: false, ctrl: false };

/** jsdom has no layout: elements are 100x30 at y 20 (or `data-y`), and 0x0 with `data-flat`. */
function layOut() {
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (this: Element) {
    if (this.hasAttribute("data-flat")) return ZERO as DOMRect;
    return { ...RECT, y: Number(this.getAttribute("data-y") ?? RECT.y) } as DOMRect;
  });
}

/** The element `elementFromPoint` finds, as a browser would at the anchor's centre. */
function hitting(element: Element | null) {
  Object.defineProperty(document, "elementFromPoint", { configurable: true, value: () => element });
}

function listen(target: EventTarget, types: string[], onEvent: (event: Event) => void = () => undefined) {
  const log: string[] = [];
  for (const type of types)
    target.addEventListener(type, (event) => {
      log.push(type);
      onEvent(event);
    });
  return log;
}

function scrollable(element: HTMLElement) {
  element.style.overflowY = "auto";
  Object.defineProperty(element, "scrollHeight", { configurable: true, value: 500 });
  Object.defineProperty(element, "clientHeight", { configurable: true, value: 100 });
  const scrollBy = vi.fn();
  const scrollTo = vi.fn();
  Object.assign(element, { scrollBy, scrollTo });
  return { scrollBy, scrollTo };
}

const act = (request: Record<string, unknown>) => ({ request: "r1", ...request }) as StageActRequest;
const byId = (id: string) => document.querySelector<HTMLElement>(`[data-stage="${id}"]`)!;

describe("perform", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    layOut();
  });

  afterEach(() => {
    vi.restoreAllMocks();
    Reflect.deleteProperty(document, "elementFromPoint");
    document.body.innerHTML = "";
  });

  describe("an anchor the page doesn't show", () => {
    it.each([
      ["is unknown", `<button data-stage="other">Other</button>`],
      ["has no size", `<button data-stage="b" data-flat>B</button>`],
    ])("is not handled when the anchor %s", async (_, html) => {
      document.body.innerHTML = html;
      expect(await perform("hub", act({ kind: "click", anchor: "b" }))).toEqual({ request: "r1", page: "hub", handled: false, ok: false });
    });
  });

  describe("reveal", () => {
    it("answers with the rectangle and its visible part without touching the anchor", async () => {
      document.body.innerHTML = `<button data-stage="b">B</button>`;
      const log = listen(byId("b"), ["pointerdown", "click", "focus"]);
      expect(await perform("hub", act({ kind: "reveal", anchor: "b" }))).toEqual({ request: "r1", page: "hub", handled: true, ok: true, rect: RECT, visible: RECT });
      expect(log).toEqual([]);
    });

    it("fails with 409 when the anchor stays out of view", async () => {
      document.body.innerHTML = `<button data-stage="b" data-y="5000">B</button>`;
      byId("b").scrollIntoView = vi.fn();
      expect(await perform("hub", act({ kind: "reveal", anchor: "b" }))).toMatchObject({
        handled: true,
        ok: false,
        status: 409,
        error: "“b” is out of view and couldn't be scrolled into view",
      });
    });
  });

  describe("click", () => {
    it("sends the pointer and mouse sequence to the element at the centre and focuses it", async () => {
      document.body.innerHTML = `<button data-stage="b">B</button>`;
      const button = byId("b");
      hitting(button);
      const log = listen(button, ["pointerdown", "mousedown", "pointerup", "mouseup", "click"]);
      expect(await perform("hub", act({ kind: "click", anchor: "b" }))).toEqual({ request: "r1", page: "hub", handled: true, ok: true, rect: RECT, visible: RECT });
      expect(log).toEqual(["pointerdown", "mousedown", "pointerup", "mouseup", "click"]);
      expect(document.activeElement).toBe(button);
    });

    it("clicks the control inside a wrapper when the point hits something else", async () => {
      document.body.innerHTML = `<div data-stage="w"><button>Inner</button></div><p id="over">Over</p>`;
      hitting(document.getElementById("over"));
      const log = listen(document.querySelector("button")!, ["click"]);
      await perform("hub", act({ kind: "click", anchor: "w" }));
      expect(log).toEqual(["click"]);
    });

    it("skips the mouse press events and the focus when pointerdown is handled", async () => {
      document.body.innerHTML = `<button data-stage="b">B</button>`;
      const button = byId("b");
      hitting(button);
      const log = listen(button, ["pointerdown", "mousedown", "pointerup", "mouseup", "click"], (event) => {
        if (event.type === "pointerdown") event.preventDefault();
      });
      await perform("hub", act({ kind: "click", anchor: "b" }));
      expect(log).toEqual(["pointerdown", "pointerup", "click"]);
      expect(document.activeElement).not.toBe(button);
    });

    it("fails with 409 and clicks nothing when the control is disabled", async () => {
      document.body.innerHTML = `<button data-stage="b" disabled>B</button>`;
      hitting(byId("b"));
      const log = listen(byId("b"), ["pointerdown", "click"]);
      expect(await perform("hub", act({ kind: "click", anchor: "b" }))).toEqual({ request: "r1", page: "hub", handled: true, ok: false, error: "“b” is disabled", status: 409 });
      expect(log).toEqual([]);
    });
  });

  describe("type", () => {
    const typing = (mode: "replace" | "append") => act({ kind: "type", anchor: "f", text: "new", mode });

    it("replaces the value of an input and fires input", async () => {
      document.body.innerHTML = `<input data-stage="f" value="old" />`;
      const input = byId("f") as HTMLInputElement;
      const seen: string[] = [];
      listen(input, ["input"], () => seen.push(input.value));
      expect(await perform("hub", typing("replace"))).toMatchObject({ handled: true, ok: true, rect: RECT });
      expect(seen).toEqual(["new"]);
      expect(input.value).toBe("new");
      expect(document.activeElement).toBe(input);
    });

    it("appends to the value of a textarea and fires input", async () => {
      document.body.innerHTML = `<textarea data-stage="f">old</textarea>`;
      const area = byId("f") as HTMLTextAreaElement;
      const log = listen(area, ["input"]);
      await perform("hub", typing("append"));
      expect(area.value).toBe("oldnew");
      expect(log).toEqual(["input"]);
    });

    it("types into the only field inside a wrapper", async () => {
      document.body.innerHTML = `<div data-stage="f"><input value="old" /></div>`;
      await perform("hub", typing("replace"));
      expect(document.querySelector("input")!.value).toBe("new");
    });

    it("fails with 409 when the anchor holds no text field", async () => {
      document.body.innerHTML = `<button data-stage="f">F</button>`;
      expect(await perform("hub", typing("replace"))).toMatchObject({ handled: true, ok: false, status: 409, error: "“f” isn't a text field" });
    });
  });

  describe("keys", () => {
    it("presses keydown and keyup on the focused element", async () => {
      document.body.innerHTML = `<input id="focused" />`;
      const input = document.getElementById("focused")!;
      input.focus();
      const seen: string[] = [];
      for (const type of ["keydown", "keyup"]) input.addEventListener(type, (e) => seen.push(`${type}:${(e as KeyboardEvent).key}:${(e as KeyboardEvent).metaKey}`));
      const keys = { ...ENTER, key: "k", code: "KeyK", meta: true };
      expect(await perform("panel", act({ kind: "keys", keys, page: "panel" }))).toEqual({ request: "r1", page: "panel", handled: true, ok: true });
      expect(seen).toEqual(["keydown:k:true", "keyup:k:true"]);
    });

    it("leaves the keys to the page Rust named", async () => {
      const log = listen(document.body, ["keydown", "keyup"]);
      expect(await perform("hub", act({ kind: "keys", keys: ENTER, page: "settings" }))).toEqual({ request: "r1", page: "hub", handled: false, ok: false });
      expect(log).toEqual([]);
    });

    it.each([
      [true, true],
      [false, false],
    ])("without a named page, handles them only when the window has focus (focus: %s)", async (focused, handled) => {
      vi.spyOn(document, "hasFocus").mockReturnValue(focused);
      const log = listen(document.body, ["keydown"]);
      expect(await perform("hub", act({ kind: "keys", keys: ENTER }))).toMatchObject({ handled });
      expect(log).toEqual(handled ? ["keydown"] : []);
    });
  });

  describe("scroll", () => {
    it("scrolls the anchor itself by the given points", async () => {
      document.body.innerHTML = `<div data-stage="list">rows</div>`;
      const { scrollBy } = scrollable(byId("list"));
      expect(await perform("hub", act({ kind: "scroll", anchor: "list", by: 120 }))).toEqual({ request: "r1", page: "hub", handled: true, ok: true, rect: RECT });
      expect(scrollBy).toHaveBeenCalledWith({ top: 120, behavior: "smooth" });
    });

    it("scrolls a scroll area inside the anchor to the bottom", async () => {
      document.body.innerHTML = `<div data-stage="wrapper"><div id="inner">rows</div></div>`;
      const { scrollTo } = scrollable(document.getElementById("inner")!);
      await perform("hub", act({ kind: "scroll", anchor: "wrapper", to: "bottom" }));
      expect(scrollTo).toHaveBeenCalledWith({ top: 500, behavior: "smooth" });
    });

    it("scrolls the scroll area around the anchor to the top", async () => {
      document.body.innerHTML = `<section id="outer"><div data-stage="row">row</div></section>`;
      const { scrollTo } = scrollable(document.getElementById("outer")!);
      await perform("hub", act({ kind: "scroll", anchor: "row", to: "top" }));
      expect(scrollTo).toHaveBeenCalledWith({ top: 0, behavior: "smooth" });
    });

    it("fails with 409 when nothing around the anchor scrolls", async () => {
      document.body.innerHTML = `<div data-stage="row">row</div>`;
      expect(await perform("hub", act({ kind: "scroll", anchor: "row", by: 10 }))).toEqual({
        request: "r1",
        page: "hub",
        handled: true,
        ok: false,
        error: "“row” isn't in anything that scrolls",
        status: 409,
      });
    });
  });
});

describe("installStageHooks", () => {
  let collect: (request: string) => void;
  let answer: (request: StageActRequest) => void;

  beforeEach(async () => {
    vi.clearAllMocks();
    layOut();
    mocked.onStageCollect.mockImplementation((handler) => {
      collect = handler;
      return Promise.resolve(() => undefined);
    });
    mocked.onStageAct.mockImplementation((handler) => {
      answer = handler;
      return Promise.resolve(() => undefined);
    });
    await installStageHooks("settings");
  });

  afterEach(() => {
    vi.restoreAllMocks();
    document.body.innerHTML = "";
  });

  it("answers a collect request with the page id, its anchors and its meta", () => {
    document.body.innerHTML = `<button data-stage="settings:back">Back</button>`;
    const { unmount } = renderHook(() => useStageMeta({ section: "automation" }));
    collect("req-1");
    expect(mocked.stageAnchors).toHaveBeenCalledWith({
      request: "req-1",
      page: "settings",
      anchors: [expect.objectContaining({ id: "settings:back", label: "Back", role: "button", rect: RECT })],
      meta: { section: "automation" },
    });
    unmount();
  });

  it("reports the outcome of an act request through stageDone", async () => {
    answer({ request: "req-2", kind: "keys", keys: ENTER, page: "settings" });
    await vi.waitFor(() => expect(mocked.stageDone).toHaveBeenCalledWith({ request: "req-2", page: "settings", handled: true, ok: true }));
  });

  it("acts once on an act request delivered twice, and again on a new request id", async () => {
    document.body.innerHTML = `<input data-stage="f" />`;
    const input = byId("f") as HTMLInputElement;
    const append = (request: string) => answer({ request, kind: "type", anchor: "f", text: "x", mode: "append" });
    append("dup-1");
    append("dup-1");
    await vi.waitFor(() => expect(mocked.stageDone).toHaveBeenCalledWith(expect.objectContaining({ request: "dup-1", ok: true })));
    expect(input.value).toBe("x");
    append("dup-2");
    await vi.waitFor(() => expect(input.value).toBe("xx"));
  });

  it("reports an act request for another page as not handled", async () => {
    answer({ request: "req-3", kind: "keys", keys: ENTER, page: "hub" });
    await vi.waitFor(() => expect(mocked.stageDone).toHaveBeenCalledWith({ request: "req-3", page: "settings", handled: false, ok: false }));
  });
});
