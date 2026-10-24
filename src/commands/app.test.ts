import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api, DEFAULT_TEXT_SIZE } from "../api";
import { resetStore, useStore } from "../state/store";
import { settings } from "../test/settings";
import { handleKeydown, installCommands, runCommand } from "./index";
import { allCommands, bindingFor } from "./registry";

const mocked = vi.mocked(api, true);

async function press(keys: string) {
  window.addEventListener("keydown", handleKeydown, true);
  try {
    await userEvent.keyboard(keys);
  } finally {
    window.removeEventListener("keydown", handleKeydown, true);
  }
}

describe("app commands", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore({ settings: settings({ textSize: "medium" }) });
    installCommands();
  });

  it("registers only the window's own commands", () => {
    expect(allCommands().map((c) => c.id).sort()).toEqual(["shortcuts.show", "sidebar.toggle", "zoom.in", "zoom.out", "zoom.reset"]);
  });

  it.each(["app.settings", "palette.toggle", "app.background", "app.quit", "settings.toggle"])("has no %s command", (id) => {
    expect(runCommand(id)).toBe(false);
  });

  describe("sidebar.toggle", () => {
    it("hides and shows the sidebar", () => {
      expect(runCommand("sidebar.toggle")).toBe(true);
      expect(useStore.getState().uiState.sidebarHidden).toBe(true);
      runCommand("sidebar.toggle");
      expect(useStore.getState().uiState.sidebarHidden).toBe(false);
    });

    it("does not run on its keys while a dialog is open", async () => {
      resetStore({ settings: settings(), dialog: { type: "settings" } });
      await press("{Meta>}{Control>}s{/Control}{/Meta}");
      expect(useStore.getState().uiState.sidebarHidden).toBe(false);
    });
  });

  describe("shortcuts.show", () => {
    it("opens the shortcuts dialog on ?", async () => {
      await press("{Shift>}?{/Shift}");
      expect(useStore.getState().dialog).toMatchObject({ type: "shortcuts" });
    });

    it("closes it on ? again", async () => {
      await press("{Shift>}?{/Shift}");
      await press("{Shift>}?{/Shift}");
      expect(useStore.getState().dialog).toBeNull();
    });

    it("leaves another open dialog alone", () => {
      resetStore({ settings: settings(), dialog: { type: "settings" } });
      runCommand("shortcuts.show");
      expect(useStore.getState().dialog).toEqual({ type: "settings" });
    });
  });

  describe("zoom", () => {
    it("saves the next larger text size on Cmd+=", async () => {
      await press("{Meta>}={/Meta}");
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ textSize: "large" }));
    });

    it("saves the next smaller text size on Cmd+-", async () => {
      await press("{Meta>}-{/Meta}");
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ textSize: "small" }));
    });

    it.each([
      ["zoom.in", "large"],
      ["zoom.out", "small"],
    ])("saves nothing from %s at the %s end", (id, size) => {
      resetStore({ settings: settings({ textSize: size as "large" | "small" }) });
      runCommand(id);
      expect(mocked.updateSettings).not.toHaveBeenCalled();
    });

    it("returns to the default size with zoom.reset", () => {
      resetStore({ settings: settings({ textSize: "large" }) });
      runCommand("zoom.reset");
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ textSize: DEFAULT_TEXT_SIZE }));
    });

    it("has no key for zoom.reset, since Cmd+0 belongs to the native menu", () => {
      expect(bindingFor("zoom.reset")).toBeNull();
    });

    it("still zooms while a dialog is open", async () => {
      resetStore({ settings: settings({ textSize: "medium" }), dialog: { type: "settings" } });
      await press("{Meta>}={/Meta}");
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ textSize: "large" }));
    });
  });
});
