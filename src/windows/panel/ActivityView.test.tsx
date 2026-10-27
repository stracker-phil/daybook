import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type ActivityProject, type ActivitySession, type ActivitySuggestion, type TimerChanged, type TimerState } from "../../api";
import { ActivityView } from "./ActivityView";

const mocked = vi.mocked(api, true);

const tt = (id: string, name: string) => ({ id, name, client: null, color: "blue", shade: 0 });
const PROJECTS: ActivityProject[] = [
  { name: "Oscorp", ttProjects: [tt("qrs", "QRS"), tt("zeno", "Zeno")], hint: null },
  { name: "Private", ttProjects: [tt("priv", "Private")], hint: null },
  { name: "Acme", ttProjects: [], hint: "Link a TimeTracker project to Acme in Settings." },
];

const session = (patch: Partial<ActivitySession> = {}): ActivitySession => ({
  mode: "switch",
  prompt: "What are you working on?",
  projects: PROJECTS,
  project: "Oscorp",
  ttProjectId: "qrs",
  note: "QRS-1 review",
  tag: "t-session",
  tt: "available",
  ...patch,
});

const suggestion = (patch: Partial<ActivitySuggestion>): ActivitySuggestion => ({
  group: "recentTimers",
  note: "",
  project: null,
  ttProjectId: null,
  tag: null,
  display: patch.note ?? "",
  ...patch,
});

const MEETING = suggestion({ group: "meetings", note: "Sprint planning", project: "Oscorp", ttProjectId: "qrs", display: "Oscorp · Sprint planning" });
const TICKET = suggestion({ group: "ticketsToday", note: "QRS-7 fix", project: "Oscorp", ttProjectId: "qrs", display: "Oscorp · QRS-7 fix" });
const RECENT = suggestion({ group: "recentTimers", note: "Lunch talk", ttProjectId: "priv", tag: "t-recent", display: "Private · Lunch talk" });
// Deliberately not in display order.
const ALL = [RECENT, TICKET, MEETING];

const timerState = (patch: Partial<TimerState>): TimerState => ({
  recordingId: "r1",
  projectId: "qrs",
  project: "QRS",
  client: null,
  tagId: null,
  tag: null,
  note: "QRS-1 review",
  startedAt: 0,
  ...patch,
});

async function renderView(patch: Partial<ActivitySession> = {}, suggestions: ActivitySuggestion[] = ALL, tracking: TimerChanged = { tt: "available", timer: null }) {
  mocked.activitySuggestions.mockResolvedValue(suggestions);
  mocked.timetrackerTimer.mockResolvedValue(tracking);
  render(<ActivityView session={session(patch)} />);
  await vi.waitFor(() => expect(mocked.activitySuggestions).toHaveBeenCalled());
  if (suggestions.length > 0) await screen.findAllByRole("option");
}

const input = () => screen.getByRole("textbox", { name: "Note" });
const pressed = () => screen.getAllByRole("button", { pressed: true }).map((b) => b.textContent);
const options = () => screen.queryAllByRole("option").map((o) => o.textContent);
const selected = () => screen.queryAllByRole("option").find((o) => o.getAttribute("aria-selected") === "true")?.textContent;
const footer = (text: string) => screen.getByText(text);
const rejection = (kind: string, message: string) => ({ kind, message, code: null });

describe("ActivityView", () => {
  beforeEach(() => vi.clearAllMocks());

  describe("opening", () => {
    it("shows the prompt, the prefilled note and the session's project pressed", async () => {
      await renderView();
      expect(screen.getByText("What are you working on?")).toBeInTheDocument();
      expect(input()).toHaveValue("QRS-1 review");
      expect(pressed()).toEqual(["QRS"]);
    });

    it("groups TimeTracker projects under their Daybook project and leaves out a name that repeats", async () => {
      await renderView();
      const oscorp = within(screen.getByRole("group", { name: "Oscorp" }));
      expect(oscorp.getByText("Oscorp")).toBeInTheDocument();
      expect(oscorp.getAllByRole("button").map((b) => b.textContent)).toEqual(["QRS", "Zeno"]);
      const own = within(screen.getByRole("group", { name: "Private" }));
      expect(own.getAllByText("Private")).toHaveLength(1);
      expect(own.getByRole("button", { name: "Private" })).toBeInTheDocument();
    });

    it("shows a project without a link as Not linked and its hint in the footer when clicked", async () => {
      await renderView();
      await userEvent.click(screen.getByRole("button", { name: "Not linked" }));
      expect(screen.getByText("Link a TimeTracker project to Acme in Settings.")).toBeInTheDocument();
      expect(pressed()).toEqual(["QRS"]);
    });

    it("lists the project keys in picker order", async () => {
      await renderView();
      expect(screen.getByLabelText("Project keys")).toHaveTextContent("⌘1 QRS⌘2 Zeno⌘3 Private");
    });
  });

  describe("project keys", () => {
    it.each([
      ["{Meta>}2{/Meta}", "Zeno"],
      ["{Meta>}3{/Meta}", "Private"],
      ["{Meta>}9{/Meta}", "QRS"],
    ])("%s leaves %s pressed", async (keys, name) => {
      await renderView();
      await userEvent.keyboard(keys);
      expect(pressed()).toEqual([name]);
    });

    it("Tab picks the next project and wraps, Shift+Tab goes back", async () => {
      await renderView();
      await userEvent.keyboard("{Tab}");
      expect(pressed()).toEqual(["Zeno"]);
      await userEvent.keyboard("{Tab}{Tab}");
      expect(pressed()).toEqual(["QRS"]);
      await userEvent.keyboard("{Shift>}{Tab}{/Shift}");
      expect(pressed()).toEqual(["Private"]);
    });

    it("swallows Tab even with a single project", async () => {
      await renderView({ projects: [PROJECTS[1]], ttProjectId: "priv" });
      expect(fireEvent.keyDown(input(), { key: "Tab" })).toBe(false);
      expect(input()).toHaveFocus();
    });

    it("picks a project by click and keeps the focus in the field", async () => {
      await renderView();
      await userEvent.click(screen.getByRole("button", { name: "Zeno" }));
      expect(pressed()).toEqual(["Zeno"]);
      expect(input()).toHaveFocus();
    });
  });

  describe("suggestions", () => {
    it("shows them under Meetings, Tickets Today and Recent Timers in that order", async () => {
      await renderView();
      expect(options()).toEqual(["Oscorp · Sprint planning", "Oscorp · QRS-7 fix", "Private · Lunch talk"]);
      expect(screen.getAllByRole("presentation").map((h) => h.textContent)).toEqual(["Meetings", "Tickets Today", "Recent Timers"]);
    });

    it("shows at most eight", async () => {
      const many = Array.from({ length: 12 }, (_, i) => suggestion({ note: `Timer ${i}`, display: `Timer ${i}` }));
      await renderView({}, many);
      expect(screen.getAllByRole("option")).toHaveLength(8);
    });

    it("keeps them all until the note is edited, then filters fuzzily", async () => {
      await renderView();
      expect(screen.getAllByRole("option")).toHaveLength(3);
      await userEvent.clear(input());
      await userEvent.type(input(), "lunch");
      expect(options()).toEqual(["Private · Lunch talk"]);
    });

    it("offers to start the text when typing hides every suggestion", async () => {
      await renderView();
      await userEvent.clear(input());
      await userEvent.type(input(), "zzz");
      expect(screen.getByText("Press ↵ to start it.")).toBeInTheDocument();
      expect(screen.queryAllByRole("option")).toHaveLength(0);
    });
  });

  describe("arrow keys", () => {
    it("put the suggestion into the field and keep the pick when it belongs to the suggestion's project", async () => {
      await renderView();
      await userEvent.keyboard("{ArrowDown}");
      expect(input()).toHaveValue("Sprint planning");
      expect(selected()).toBe("Oscorp · Sprint planning");
      expect(pressed()).toEqual(["QRS"]);
    });

    it("move the pick to the TimeTracker project the suggestion carries", async () => {
      await renderView({ ttProjectId: "zeno" });
      await userEvent.keyboard("{ArrowDown}");
      expect(pressed()).toEqual(["QRS"]);
    });

    it("switch to the project of a meeting from another Daybook project", async () => {
      await renderView({}, [suggestion({ group: "meetings", note: "Dinner", project: "Private", ttProjectId: "priv", display: "Private · Dinner" })]);
      await userEvent.keyboard("{ArrowDown}");
      expect(pressed()).toEqual(["Private"]);
    });

    it("keep the pick for a suggestion without a project", async () => {
      await renderView({ ttProjectId: "zeno" }, [suggestion({ group: "meetings", note: "Call", display: "Call" })]);
      await userEvent.keyboard("{ArrowDown}");
      expect(pressed()).toEqual(["Zeno"]);
    });

    it("pick a recent timer's own TimeTracker project", async () => {
      await renderView();
      await userEvent.keyboard("{ArrowDown}{ArrowDown}{ArrowDown}");
      expect(input()).toHaveValue("Lunch talk");
      expect(pressed()).toEqual(["Private"]);
    });

    it("wrap from your own input to the last suggestion and back to your own input", async () => {
      await renderView();
      await userEvent.keyboard("{ArrowUp}");
      expect(selected()).toBe("Private · Lunch talk");
      await userEvent.keyboard("{ArrowDown}");
      expect(selected()).toBeUndefined();
      expect(input()).toHaveValue("QRS-1 review");
      expect(pressed()).toEqual(["QRS"]);
    });
  });

  describe("taking a suggestion over", () => {
    it("makes it your own input when you type", async () => {
      await renderView();
      await userEvent.keyboard("{ArrowDown}");
      await userEvent.keyboard("!");
      expect(input()).toHaveValue("Sprint planning!");
      expect(selected()).toBeUndefined();
    });

    it("keeps a recent timer's project when you type", async () => {
      await renderView();
      await userEvent.keyboard("{ArrowDown}{ArrowDown}{ArrowDown}");
      await userEvent.keyboard("x");
      expect(input()).toHaveValue("Lunch talkx");
      expect(pressed()).toEqual(["Private"]);
    });

    it("keeps the note when you pick another project", async () => {
      await renderView();
      await userEvent.keyboard("{ArrowDown}{ArrowDown}{ArrowDown}");
      await userEvent.click(screen.getByRole("button", { name: "Zeno" }));
      expect(input()).toHaveValue("Lunch talk");
      expect(pressed()).toEqual(["Zeno"]);
      expect(selected()).toBeUndefined();
    });
  });

  describe("submitting", () => {
    it("sends the shown note and project with the session's tag while both stay", async () => {
      await renderView();
      await userEvent.keyboard("{Enter}");
      expect(mocked.activitySubmit).toHaveBeenCalledWith("switch", { ttProjectId: "qrs", note: "QRS-1 review", tag: "t-session" });
    });

    it("sends the mode of the session", async () => {
      await renderView({ mode: "checkin" });
      await userEvent.keyboard("{Enter}");
      expect(mocked.activitySubmit).toHaveBeenCalledWith("checkin", expect.objectContaining({ ttProjectId: "qrs" }));
    });

    it("sends the note verbatim and keeps the tag when only whitespace differs", async () => {
      await renderView();
      await userEvent.clear(input());
      await userEvent.type(input(), "QRS-1 review  {Enter}");
      expect(mocked.activitySubmit).toHaveBeenCalledWith("switch", { ttProjectId: "qrs", note: "QRS-1 review  ", tag: "t-session" });
    });

    it("keeps the session's tag for another note on the same project", async () => {
      await renderView();
      await userEvent.clear(input());
      await userEvent.type(input(), "Something else{Enter}");
      expect(mocked.activitySubmit).toHaveBeenCalledWith("switch", { ttProjectId: "qrs", note: "Something else", tag: "t-session" });
    });

    it("sends no tag for another project", async () => {
      await renderView();
      await userEvent.keyboard("{Meta>}2{/Meta}{Enter}");
      expect(mocked.activitySubmit).toHaveBeenCalledWith("switch", { ttProjectId: "zeno", note: "QRS-1 review", tag: null });
    });

    it("sends a highlighted recent timer with its project and tag", async () => {
      await renderView();
      await userEvent.keyboard("{ArrowDown}{ArrowDown}{ArrowDown}{Enter}");
      expect(mocked.activitySubmit).toHaveBeenCalledWith("switch", { ttProjectId: "priv", note: "Lunch talk", tag: "t-recent" });
    });

    it("keeps a recent timer's tag after you edit its note but drops it when you pick another project", async () => {
      await renderView();
      await userEvent.keyboard("{ArrowDown}{ArrowDown}{ArrowDown}");
      await userEvent.keyboard("x{Enter}");
      expect(mocked.activitySubmit).toHaveBeenLastCalledWith("switch", { ttProjectId: "priv", note: "Lunch talkx", tag: "t-recent" });
      await userEvent.keyboard("{Meta>}2{/Meta}{Enter}");
      expect(mocked.activitySubmit).toHaveBeenLastCalledWith("switch", { ttProjectId: "zeno", note: "Lunch talkx", tag: null });
    });

    it("starts a clicked suggestion", async () => {
      await renderView();
      await userEvent.click(screen.getByRole("option", { name: "Private · Lunch talk" }));
      expect(mocked.activitySubmit).toHaveBeenCalledWith("switch", { ttProjectId: "priv", note: "Lunch talk", tag: "t-recent" });
    });
  });

  describe("footer", () => {
    it("lists the keys", async () => {
      await renderView();
      expect(footer("⇥ next project · ↑↓ suggestions · ↵ start · esc cancel")).toBeInTheDocument();
    });

    it("leaves out the next project with a single TimeTracker project", async () => {
      await renderView({ projects: [PROJECTS[1]], ttProjectId: "priv" });
      expect(footer("↑↓ suggestions · ↵ start · esc cancel")).toBeInTheDocument();
    });

    it("says ↵ still on it in a check-in while the live timer matches what is shown, until it differs", async () => {
      await renderView({ mode: "checkin" }, ALL, { tt: "available", timer: timerState({ note: " QRS-1 review " }) });
      expect(await screen.findByText(/↵ still on it/)).toBeInTheDocument();
      await userEvent.clear(input());
      await userEvent.type(input(), "Other");
      expect(screen.getByText(/↵ start/)).toBeInTheDocument();
      expect(screen.queryByText(/still on it/)).not.toBeInTheDocument();
    });

    it("says ↵ still on it again when the project differs and returns", async () => {
      await renderView({ mode: "checkin" }, ALL, { tt: "available", timer: timerState({}) });
      await screen.findByText(/↵ still on it/);
      await userEvent.keyboard("{Meta>}2{/Meta}");
      expect(screen.getByText(/↵ start/)).toBeInTheDocument();
      await userEvent.keyboard("{Meta>}1{/Meta}");
      expect(screen.getByText(/↵ still on it/)).toBeInTheDocument();
    });

    it("keeps ↵ start in a switch even when the live timer matches", async () => {
      await renderView({ mode: "switch" }, ALL, { tt: "available", timer: timerState({}) });
      await vi.waitFor(() => expect(mocked.timetrackerTimer).toHaveBeenCalled());
      expect(screen.getByText(/↵ start/)).toBeInTheDocument();
    });
  });

  describe("errors", () => {
    it("shows that TimeTracker isn't running after a refused submit and keeps the note and the pick", async () => {
      mocked.activitySubmit.mockRejectedValueOnce(rejection("timetracker_not_running", "not running"));
      await renderView();
      await userEvent.keyboard("{Meta>}2{/Meta}");
      await userEvent.clear(input());
      await userEvent.type(input(), "Typed{Enter}");
      expect(await screen.findByText("TimeTracker isn't running.")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Open TimeTracker" })).toBeInTheDocument();
      expect(input()).toHaveValue("Typed");
      expect(pressed()).toEqual(["Zeno"]);
    });

    it("shows that TimeTracker isn't running when the session says so", async () => {
      await renderView({ tt: "notRunning" }, ALL, { tt: "notRunning", timer: null });
      expect(await screen.findByText("TimeTracker isn't running.")).toBeInTheDocument();
    });

    it("clears the message once TimeTracker is opened", async () => {
      await renderView({ tt: "notRunning" }, ALL, { tt: "notRunning", timer: null });
      await userEvent.click(await screen.findByRole("button", { name: "Open TimeTracker" }));
      await vi.waitFor(() => expect(screen.queryByText("TimeTracker isn't running.")).not.toBeInTheDocument());
      expect(mocked.timetrackerFocus).toHaveBeenCalled();
    });

    it("shows the message when TimeTracker can't be opened", async () => {
      mocked.timetrackerFocus.mockRejectedValueOnce(rejection("io", "Could not launch it"));
      await renderView({ tt: "notRunning" }, ALL, { tt: "notRunning", timer: null });
      await userEvent.click(await screen.findByRole("button", { name: "Open TimeTracker" }));
      expect(await screen.findByText("Could not launch it")).toBeInTheDocument();
    });

    it("points to Settings when TimeTracker isn't installed", async () => {
      mocked.activitySubmit.mockRejectedValueOnce(rejection("timetracker_missing", "missing"));
      await renderView();
      await userEvent.keyboard("{Enter}");
      expect(await screen.findByText("TimeTracker isn't installed (Settings → TimeTracker).")).toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "Open TimeTracker" })).not.toBeInTheDocument();
    });

    it("shows any other message and clears it on the next keystroke", async () => {
      mocked.activitySubmit.mockRejectedValueOnce(rejection("io", "Disk full"));
      await renderView();
      await userEvent.keyboard("{Enter}");
      expect(await screen.findByText("Disk full")).toBeInTheDocument();
      expect(input()).toHaveValue("QRS-1 review");
      await userEvent.type(input(), "1");
      expect(screen.queryByText("Disk full")).not.toBeInTheDocument();
    });

    it("shows why suggestions could not load", async () => {
      mocked.activitySuggestions.mockRejectedValue(rejection("io", "No suggestions today"));
      mocked.timetrackerTimer.mockResolvedValue({ tt: "available", timer: null });
      render(<ActivityView session={session()} />);
      expect(await screen.findByText("No suggestions today")).toBeInTheDocument();
    });
  });

  describe("timer_changed", () => {
    const push = (value: TimerChanged) => act(() => mocked.onTimerChanged.mock.lastCall![0](value));

    it("shows TimeTracker's new state", async () => {
      await renderView();
      push({ tt: "notRunning", timer: null });
      expect(await screen.findByText("TimeTracker isn't running.")).toBeInTheDocument();
    });

    it("fetches the suggestions again and keeps the old list while they load", async () => {
      await renderView();
      let resolve!: (value: ActivitySuggestion[]) => void;
      mocked.activitySuggestions.mockReturnValueOnce(new Promise((r) => (resolve = r)));
      push({ tt: "notRunning", timer: null });
      await screen.findByText("TimeTracker isn't running.");
      expect(options()).toHaveLength(3);
      await act(async () => resolve([suggestion({ note: "Fresh", display: "Fresh" })]));
      expect(options()).toEqual(["Fresh"]);
    });
  });

  describe("check-in timeout", () => {
    beforeEach(() => vi.useFakeTimers({ shouldAdvanceTime: true }));
    afterEach(() => vi.useRealTimers());

    const focusWindow = () => act(() => void window.dispatchEvent(new Event("focus")));
    const blurWindow = () => act(() => void window.dispatchEvent(new Event("blur")));

    it("closes the overlay after a minute in a check-in", async () => {
      await renderView({ mode: "checkin" });
      focusWindow();
      await vi.advanceTimersByTimeAsync(55_000);
      expect(mocked.panelClose).not.toHaveBeenCalled();
      await vi.advanceTimersByTimeAsync(6_000);
      expect(mocked.panelClose).toHaveBeenCalled();
    });

    it("never closes a switch", async () => {
      await renderView({ mode: "switch" });
      focusWindow();
      await vi.advanceTimersByTimeAsync(300_000);
      expect(mocked.panelClose).not.toHaveBeenCalled();
    });

    it("restarts the minute when you type", async () => {
      await renderView({ mode: "checkin" });
      focusWindow();
      await vi.advanceTimersByTimeAsync(50_000);
      await userEvent.keyboard("x");
      await vi.advanceTimersByTimeAsync(50_000);
      expect(mocked.panelClose).not.toHaveBeenCalled();
      await vi.advanceTimersByTimeAsync(11_000);
      expect(mocked.panelClose).toHaveBeenCalled();
    });

    it("pauses while the window is blurred and starts the minute again on refocus", async () => {
      await renderView({ mode: "checkin" });
      focusWindow();
      await vi.advanceTimersByTimeAsync(50_000);
      blurWindow();
      await vi.advanceTimersByTimeAsync(300_000);
      expect(mocked.panelClose).not.toHaveBeenCalled();
      focusWindow();
      await vi.advanceTimersByTimeAsync(50_000);
      expect(mocked.panelClose).not.toHaveBeenCalled();
      await vi.advanceTimersByTimeAsync(11_000);
      expect(mocked.panelClose).toHaveBeenCalled();
    });
  });
});
