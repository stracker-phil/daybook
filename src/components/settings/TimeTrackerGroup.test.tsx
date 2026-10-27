import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type Settings, type TimeTrackerInfo } from "../../api";
import { resetStore, useStore } from "../../state/store";
import { settings } from "../../test/settings";
import { TimeTrackerGroup } from "./TimeTrackerGroup";

const mocked = vi.mocked(api, true);
const set = vi.fn<(patch: Partial<Settings>) => void>();

const info = (patch: Partial<TimeTrackerInfo> = {}): TimeTrackerInfo => ({ path: "/Users/pat/.local/bin/timetracker", defaultPath: "~/.local/bin/timetracker", state: "available", version: "0.1.0", ...patch });

function renderGroup(path = "") {
  const s = settings();
  render(<TimeTrackerGroup s={{ ...s, timetracker: { ...s.timetracker, path } }} set={set} />);
}

describe("TimeTrackerGroup", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
  });

  it("is titled TimeTracker", () => {
    renderGroup();
    expect(screen.getByRole("heading", { name: "TimeTracker" })).toBeInTheDocument();
  });

  describe("state", () => {
    it("shows the version while TimeTracker runs", async () => {
      mocked.timetrackerInfo.mockResolvedValue(info());
      renderGroup();
      expect(await screen.findByText("Running · 0.1.0")).toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "Open TimeTracker" })).not.toBeInTheDocument();
    });

    it("shows the command path with the home folder as a tilde", async () => {
      renderGroup();
      expect(await screen.findByText("~/.local/bin/timetracker", { selector: ".mono" })).toBeInTheDocument();
    });

    it("says when TimeTracker is not installed", async () => {
      mocked.timetrackerInfo.mockResolvedValue(info({ state: "missing", version: null }));
      renderGroup();
      expect(await screen.findByText("Not installed")).toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "Open TimeTracker" })).not.toBeInTheDocument();
    });

    it("offers to open TimeTracker when it is not running and opens it", async () => {
      mocked.timetrackerInfo.mockResolvedValue(info({ state: "notRunning", version: null }));
      renderGroup();
      expect(await screen.findByText("TimeTracker isn't running")).toBeInTheDocument();
      await userEvent.click(screen.getByRole("button", { name: "Open TimeTracker" }));
      expect(mocked.timetrackerFocus).toHaveBeenCalled();
    });

    it("toasts when opening TimeTracker fails", async () => {
      mocked.timetrackerInfo.mockResolvedValue(info({ state: "notRunning", version: null }));
      mocked.timetrackerFocus.mockRejectedValueOnce({ kind: "io", message: "No such app", code: null });
      renderGroup();
      await userEvent.click(await screen.findByRole("button", { name: "Open TimeTracker" }));
      await vi.waitFor(() => expect(useStore.getState().toasts.map((t) => t.message)).toContain("Couldn't open TimeTracker: No such app"));
    });

    it("reads the state again when the timer changes", async () => {
      mocked.timetrackerInfo.mockResolvedValueOnce(info({ state: "notRunning", version: null }));
      renderGroup();
      await screen.findByText("TimeTracker isn't running");
      mocked.timetrackerInfo.mockResolvedValueOnce(info());
      await vi.waitFor(() => expect(mocked.onTimerChanged).toHaveBeenCalled());
      act(() => mocked.onTimerChanged.mock.calls.forEach(([handler]) => handler({} as never)));
      expect(await screen.findByText("Running · 0.1.0")).toBeInTheDocument();
    });
  });

  describe("Path", () => {
    it("shows the saved path", () => {
      renderGroup("/opt/tt");
      expect(screen.getByRole("textbox", { name: "Path" })).toHaveValue("/opt/tt");
    });

    it("shows the default path as the placeholder and in the description once loaded", async () => {
      mocked.timetrackerInfo.mockResolvedValue(info({ defaultPath: "~/bin/tt" }));
      renderGroup();
      expect(await screen.findByText("The timetracker command. Empty: ~/bin/tt.")).toBeInTheDocument();
      expect(screen.getByRole("textbox", { name: "Path" })).toHaveAttribute("placeholder", "~/bin/tt");
    });

    it("saves the committed path trimmed", async () => {
      renderGroup();
      await userEvent.type(screen.getByRole("textbox", { name: "Path" }), "  /opt/tt  {Enter}");
      expect(set.mock.lastCall![0].timetracker!.path).toBe("/opt/tt");
    });
  });
});
