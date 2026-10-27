import { renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { currentMeta, useStageMeta } from "./meta";
import type { StagePageMeta } from "./types";

describe("useStageMeta", () => {
  it("tells nothing while no component registered a part", () => {
    expect(currentMeta()).toEqual({});
  });

  it("merges the parts: the last view and section win, ui objects combine", () => {
    renderHook(() => useStageMeta({ view: "today", ui: { inspector: "open" } }));
    renderHook(() => useStageMeta({ view: "day:2026-10-26", section: "automation", ui: { rail: "closed" } }));
    expect(currentMeta()).toEqual({ view: "day:2026-10-26", section: "automation", ui: { inspector: "open", rail: "closed" } });
  });

  it("drops a part when its component unmounts", () => {
    const first = renderHook(() => useStageMeta({ view: "today" }));
    const second = renderHook(() => useStageMeta({ ui: { inspector: "closed" } }));
    expect(currentMeta()).toEqual({ view: "today", ui: { inspector: "closed" } });
    second.unmount();
    expect(currentMeta()).toEqual({ view: "today" });
    first.unmount();
    expect(currentMeta()).toEqual({});
  });

  it("follows a part that changes", () => {
    const { rerender, unmount } = renderHook((meta: StagePageMeta) => useStageMeta(meta), { initialProps: { section: "vault" } });
    expect(currentMeta().section).toBe("vault");
    rerender({ section: "slack" });
    expect(currentMeta().section).toBe("slack");
    unmount();
  });
});
