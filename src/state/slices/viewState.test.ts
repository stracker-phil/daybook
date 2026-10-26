import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, EMPTY_UI_STATE } from "../../api";
import { flushUiState, resetStore, useStore } from "../store";

const mocked = vi.mocked(api, true);

describe("view state slice", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    resetStore();
  });

  afterEach(() => {
    flushUiState();
    vi.useRealTimers();
  });

  describe("loadUiState", () => {
    it("takes the layout Rust stored", async () => {
      mocked.getUiState.mockResolvedValueOnce({ ...EMPTY_UI_STATE, sidebarHidden: true, paneSizes: { list: 300 } });
      await useStore.getState().loadUiState();
      expect(useStore.getState().uiState).toMatchObject({ sidebarHidden: true, paneSizes: { list: 300 } });
    });

    it("toasts and keeps the empty layout when loading fails", async () => {
      mocked.getUiState.mockRejectedValueOnce({ kind: "io", message: "Disk full", code: null });
      await useStore.getState().loadUiState();
      expect(useStore.getState().uiState).toEqual(EMPTY_UI_STATE);
      expect(useStore.getState().toasts[0]).toMatchObject({ kind: "error", message: "Could not load the window layout: Disk full" });
    });
  });

  describe("toggleSidebar", () => {
    it("hides the sidebar and writes the layout after the debounce", () => {
      useStore.getState().toggleSidebar();
      expect(useStore.getState().uiState.sidebarHidden).toBe(true);
      expect(mocked.setUiState).not.toHaveBeenCalled();
      vi.advanceTimersByTime(500);
      expect(mocked.setUiState).toHaveBeenCalledWith(expect.objectContaining({ sidebarHidden: true }));
    });

    it("shows a hidden sidebar again", () => {
      useStore.getState().toggleSidebar();
      useStore.getState().toggleSidebar();
      expect(useStore.getState().uiState.sidebarHidden).toBe(false);
    });
  });

  describe("setPaneSize", () => {
    it("stores the rounded size under the pane id and keeps the other panes", () => {
      useStore.getState().setPaneSize("list", 240.6);
      useStore.getState().setPaneSize("detail", 120);
      expect(useStore.getState().uiState.paneSizes).toEqual({ list: 241, detail: 120 });
    });

    it("writes only the last of several quick changes", () => {
      useStore.getState().setPaneSize("list", 100);
      useStore.getState().setPaneSize("list", 200);
      vi.advanceTimersByTime(500);
      expect(mocked.setUiState).toHaveBeenCalledTimes(1);
      expect(mocked.setUiState).toHaveBeenCalledWith(expect.objectContaining({ paneSizes: { list: 200 } }));
    });
  });

  describe("toggleTodoGroup", () => {
    it("folds a group and unfolds it again", () => {
      useStore.getState().toggleTodoGroup("Today");
      expect(useStore.getState().uiState.collapsedTodoGroups).toEqual(["Today"]);
      useStore.getState().toggleTodoGroup("Today");
      expect(useStore.getState().uiState.collapsedTodoGroups).toEqual([]);
    });

    it("keeps the other folded groups", () => {
      useStore.getState().toggleTodoGroup("Today");
      useStore.getState().toggleTodoGroup("Last week");
      useStore.getState().toggleTodoGroup("Today");
      expect(useStore.getState().uiState.collapsedTodoGroups).toEqual(["Last week"]);
    });

    it("writes the folded groups after the debounce", () => {
      useStore.getState().toggleTodoGroup("Today");
      vi.advanceTimersByTime(500);
      expect(mocked.setUiState).toHaveBeenCalledWith(expect.objectContaining({ collapsedTodoGroups: ["Today"] }));
    });
  });

  it("toasts when saving the layout fails", async () => {
    mocked.setUiState.mockRejectedValueOnce({ kind: "io", message: "Read-only", code: null });
    useStore.getState().toggleSidebar();
    await vi.advanceTimersByTimeAsync(500);
    expect(useStore.getState().toasts[0].message).toBe("Could not save the window layout: Read-only");
  });
});
