import { render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api, type ContextState } from "../api";
import { applyContextColor, useContextColor } from "./useContextColor";

const mocked = vi.mocked(api, true);

const STATE: ContextState = {
  contexts: [
    { id: "work", title: "Work", color: "blue" },
    { id: "private", title: "Private", color: "green" },
  ],
  current: "work",
};

const root = () => document.documentElement.dataset.context;

function Host() {
  useContextColor();
  return null;
}

describe("applyContextColor", () => {
  afterEach(() => delete document.documentElement.dataset.context);

  it("sets the current context's colour on the root", () => {
    applyContextColor({ ...STATE, current: "private" });
    expect(root()).toBe("green");
  });

  it.each([
    ["has no current context", { ...STATE, current: null }],
    ["names a context that no longer exists", { ...STATE, current: "gone" }],
    ["has no contexts", { contexts: [], current: null }],
  ])("removes the colour when the state %s", (_, state) => {
    document.documentElement.dataset.context = "red";
    applyContextColor(state);
    expect(root()).toBeUndefined();
  });
});

describe("useContextColor", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.contextState.mockResolvedValue(STATE);
  });
  afterEach(() => delete document.documentElement.dataset.context);

  it("applies the current colour on mount", async () => {
    render(<Host />);
    await vi.waitFor(() => expect(root()).toBe("blue"));
  });

  it("follows a context switch", async () => {
    let changed: () => void = () => {};
    mocked.onContextChanged.mockImplementation(async (handler) => {
      changed = () => handler({ id: "private" });
      return () => {};
    });
    render(<Host />);
    await vi.waitFor(() => expect(root()).toBe("blue"));
    mocked.contextState.mockResolvedValue({ ...STATE, current: "private" });
    changed();
    await vi.waitFor(() => expect(root()).toBe("green"));
  });

  it("follows a settings save that recolours the current context", async () => {
    let saved: () => void = () => {};
    mocked.onSettingsChanged.mockImplementation(async (handler) => {
      saved = () => handler({} as never);
      return () => {};
    });
    render(<Host />);
    await vi.waitFor(() => expect(root()).toBe("blue"));
    mocked.contextState.mockResolvedValue({ ...STATE, contexts: [{ id: "work", title: "Work", color: "red" }, STATE.contexts[1]] });
    saved();
    await vi.waitFor(() => expect(root()).toBe("red"));
  });

  it("leaves the root alone when the state cannot be read", async () => {
    mocked.contextState.mockRejectedValue({ kind: "io", message: "Unreadable", code: null });
    render(<Host />);
    await vi.waitFor(() => expect(mocked.contextState).toHaveBeenCalled());
    expect(root()).toBeUndefined();
  });
});
