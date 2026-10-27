import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, EMPTY_UI_STATE, type DayActivity, type DayActivityProject, type DayActivityRow, type HubView, type TimeTrackerDay } from "../../api";
import { flushUiState, resetStore, useStore } from "../../state/store";
import { ActivityTab } from "./ActivityTab";

const mocked = vi.mocked(api, true);

const row = (patch: Partial<DayActivityRow> = {}): DayActivityRow => ({
  kind: "work",
  start: "09:00",
  end: "10:25",
  duration: "1h 25m",
  durationMs: 5_100_000,
  title: "Apollo · Fix sync",
  project: "Apollo",
  client: null,
  note: "Fix sync",
  tag: null,
  color: "blue",
  shade: 4,
  suggestedBy: null,
  copyText: "Fix sync",
  recordingId: "r1",
  ...patch,
});

const apollo: DayActivityProject = {
  projectId: "p1",
  project: "Apollo",
  client: "Acme",
  color: "blue",
  shade: 4,
  total: "1h 25m",
  totalMs: 5_100_000,
  recordings: [{ recordingId: "r1", note: "Fix sync", tag: "Review", spans: "09:00–10:25", duration: "1h 25m", durationMs: 5_100_000, running: false }],
};

const activity = (patch: Partial<DayActivity> = {}): DayActivity => ({
  day: "2026-10-24",
  isToday: true,
  rows: [row()],
  projects: [apollo],
  total: "1h 25m",
  totalMs: 5_100_000,
  endLabel: "10:25",
  empty: null,
  running: false,
  ...patch,
});

const day = (patch: Partial<DayActivity> = {}): TimeTrackerDay => ({ state: "available", activity: activity(patch) });

let hub: HubView;

async function renderTab(activityView: "timeline" | "byProject" = "timeline") {
  hub = { ...(await api.hubView()), day: "2026-10-24" };
  resetStore({ uiState: { ...EMPTY_UI_STATE, activityView } });
  return render(<ActivityTab hub={hub} />);
}

const toastMessages = () => useStore.getState().toasts.map((t) => t.message);

describe("ActivityTab", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.timetrackerDay.mockImplementation(async (d: string) => day({ day: d }));
  });

  afterEach(() => flushUiState());

  it("reads TimeTracker's day for the shown day and lists its recordings", async () => {
    await renderTab();
    expect(await screen.findByRole("list", { name: "Timeline" })).toBeInTheDocument();
    expect(mocked.timetrackerDay).toHaveBeenCalledWith("2026-10-24");
    expect(screen.getByRole("button", { name: /Apollo · Fix sync/ })).toBeInTheDocument();
  });

  describe("without TimeTracker", () => {
    beforeEach(() => mocked.timetrackerDay.mockResolvedValue({ state: "notRunning", activity: null }));

    it("says TimeTracker isn't running and opens it from the button", async () => {
      await renderTab();
      expect(await screen.findByText("TimeTracker isn't running")).toBeInTheDocument();
      await userEvent.click(screen.getByRole("button", { name: "Open TimeTracker" }));
      expect(mocked.runCommand).toHaveBeenCalledWith(hub.timetracker.command);
    });

    it("toasts when TimeTracker cannot be opened", async () => {
      mocked.runCommand.mockRejectedValueOnce({ kind: "io", message: "Not installed", code: null });
      await renderTab();
      await userEvent.click(await screen.findByRole("button", { name: "Open TimeTracker" }));
      await vi.waitFor(() => expect(toastMessages()).toContain("Couldn't open TimeTracker: Not installed"));
    });
  });

  describe("the recorded total", () => {
    it("shows the day's total when something is recorded", async () => {
      await renderTab();
      expect(await screen.findByText("Recorded")).toBeInTheDocument();
      expect(screen.getAllByTitle("Copy 1h 25m").length).toBeGreaterThan(0);
    });

    it("is left out when nothing is recorded, and the empty text shows as given", async () => {
      mocked.timetrackerDay.mockResolvedValue(day({ rows: [], projects: [], total: "0m", totalMs: 0, endLabel: null, empty: "Nothing recorded yet" }));
      await renderTab();
      expect(await screen.findByText("Nothing recorded yet")).toBeInTheDocument();
      expect(screen.queryByText("Recorded")).not.toBeInTheDocument();
    });

    it("shows no empty text once there are rows", async () => {
      await renderTab();
      await screen.findByText("Recorded");
      expect(screen.queryByText(/Nothing recorded/)).not.toBeInTheDocument();
    });
  });

  it("shows nothing while the day of the data differs from the shown day", async () => {
    mocked.timetrackerDay.mockResolvedValue(day({ day: "2026-10-23" }));
    const { container } = await renderTab();
    await vi.waitFor(() => expect(mocked.timetrackerDay).toHaveBeenCalled());
    expect(container).toBeEmptyDOMElement();
  });

  describe("the view switch", () => {
    it("shows the timeline by default and By Project after switching", async () => {
      await renderTab();
      await screen.findByRole("list", { name: "Timeline" });
      await userEvent.click(screen.getByRole("button", { name: "By Project" }));
      expect(screen.getByRole("button", { name: "By Project" })).toHaveAttribute("aria-pressed", "true");
      expect(screen.getByRole("region", { name: "Apollo" })).toBeInTheDocument();
      expect(screen.queryByRole("list", { name: "Timeline" })).not.toBeInTheDocument();
      expect(screen.getByText("Review · 09:00–10:25")).toBeInTheDocument();
    });

    it("saves the chosen view with the window layout", async () => {
      await renderTab();
      await userEvent.click(await screen.findByRole("button", { name: "By Project" }));
      flushUiState();
      expect(mocked.setUiState).toHaveBeenCalledWith(expect.objectContaining({ activityView: "byProject" }));
    });

    it("opens in the saved view", async () => {
      await renderTab("byProject");
      expect(await screen.findByRole("region", { name: "Apollo" })).toBeInTheDocument();
    });
  });

  describe("copying", () => {
    it("copies a duration with a toast", async () => {
      const user = userEvent.setup();
      await renderTab();
      await user.click((await screen.findAllByTitle("Copy 1h 25m"))[0]);
      expect(await navigator.clipboard.readText()).toBe("1h 25m");
      expect(toastMessages()).toContain("Copied 1h 25m");
    });

    it("copies the note of a row", async () => {
      const user = userEvent.setup();
      await renderTab();
      await user.click(await screen.findByTitle("Copy the note"));
      expect(await navigator.clipboard.readText()).toBe("Fix sync");
      expect(toastMessages()).toContain("Copied the note");
    });
  });

  describe("reloading", () => {
    it("reads the day again when TimeTracker reports a change", async () => {
      await renderTab();
      await screen.findByText("Recorded");
      mocked.timetrackerDay.mockResolvedValue(day({ rows: [row({ note: "New work", copyText: "New work" })] }));
      const onChange = mocked.onActivityChanged.mock.calls[0][0];
      act(() => onChange());
      expect(await screen.findByText(/New work/)).toBeInTheDocument();
    });

    it("reads the day again when the window gets focus", async () => {
      await renderTab();
      await screen.findByText("Recorded");
      mocked.timetrackerDay.mockResolvedValue(day({ rows: [row({ note: "After focus", copyText: "After focus" })] }));
      fireEvent.focus(window);
      expect(await screen.findByText(/After focus/)).toBeInTheDocument();
    });

    it("reads the day again every minute", async () => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
      try {
        await renderTab();
        await screen.findByText("Recorded");
        mocked.timetrackerDay.mockResolvedValue(day({ rows: [row({ note: "A minute later", copyText: "A minute later" })] }));
        await act(() => vi.advanceTimersByTimeAsync(60_000));
        expect(await screen.findByText(/A minute later/)).toBeInTheDocument();
      } finally {
        vi.useRealTimers();
      }
    });

    it("reads the new day when the shown day changes", async () => {
      const { rerender } = await renderTab();
      await screen.findByText("Recorded");
      mocked.timetrackerDay.mockImplementation(async (d: string) => day({ day: d, rows: [row({ note: `Work on ${d}`, copyText: `Work on ${d}` })] }));
      rerender(<ActivityTab hub={{ ...hub, day: "2026-10-23" }} />);
      expect(await screen.findByText(/Work on 2026-10-23/)).toBeInTheDocument();
      expect(mocked.timetrackerDay).toHaveBeenCalledWith("2026-10-23");
    });
  });

  it("toasts when TimeTracker's day cannot be read", async () => {
    mocked.timetrackerDay.mockRejectedValue({ kind: "io", message: "Refused", code: null });
    await renderTab();
    await vi.waitFor(() => expect(toastMessages()).toContain("Couldn't read TimeTracker's day: Refused"));
  });
});
