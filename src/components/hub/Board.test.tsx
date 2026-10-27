import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type HubView } from "../../api";
import { flushUiState, resetStore, useStore } from "../../state/store";
import { settings } from "../../test/settings";
import { Board } from "./Board";

const mocked = vi.mocked(api, true);

async function renderBoard(patch: Partial<HubView> = {}) {
  const hub = { ...(await api.hubView()), day: "2026-10-24", date: "24 October 2026", isToday: true, ...patch };
  resetStore({ settings: settings(), hub });
  render(<Board />);
}

describe("Board", () => {
  beforeEach(() => vi.clearAllMocks());

  afterEach(() => flushUiState());

  describe("inspector toggle", () => {
    it("is pressed and offers to hide the inspector while it is shown", async () => {
      await renderBoard();
      expect(screen.getByRole("button", { name: "Hide Inspector" })).toHaveAttribute("aria-pressed", "true");
    });

    it("hides the inspector and then offers to show it", async () => {
      await renderBoard();
      await userEvent.click(screen.getByRole("button", { name: "Hide Inspector" }));
      expect(screen.getByRole("button", { name: "Show Inspector" })).toHaveAttribute("aria-pressed", "false");
      expect(useStore.getState().uiState.inspectorHidden).toBe(true);
    });

    it("shows the inspector again on a second click", async () => {
      await renderBoard();
      await userEvent.click(screen.getByRole("button", { name: "Hide Inspector" }));
      await userEvent.click(screen.getByRole("button", { name: "Show Inspector" }));
      expect(screen.getByRole("button", { name: "Hide Inspector" })).toHaveAttribute("aria-pressed", "true");
    });
  });

  describe("TimeTracker button", () => {
    it("opens TimeTracker on today", async () => {
      await renderBoard();
      await userEvent.click(screen.getByRole("button", { name: "Open in TimeTracker" }));
      expect(mocked.runCommand).toHaveBeenCalledWith("timetracker:show");
    });

    it("shows another day in TimeTracker through its date", async () => {
      await renderBoard({ isToday: false, timetracker: { command: "timetracker:show:2026-10-24", label: "Show 24 October 2026 in TimeTracker" } });
      await userEvent.click(screen.getByRole("button", { name: "Show 24 October 2026 in TimeTracker" }));
      expect(mocked.runCommand).toHaveBeenCalledWith("timetracker:show:2026-10-24");
    });

    it("toasts when TimeTracker cannot be opened", async () => {
      mocked.runCommand.mockRejectedValueOnce({ kind: "timetrackerNotRunning", message: "It isn't running", code: null });
      await renderBoard();
      await userEvent.click(screen.getByRole("button", { name: "Open in TimeTracker" }));
      await vi.waitFor(() => expect(useStore.getState().toasts.map((t) => t.message)).toContain("Couldn't open TimeTracker: It isn't running"));
    });
  });
});
