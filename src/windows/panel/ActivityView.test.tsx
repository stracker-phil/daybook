import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type ActivityProject, type ActivitySession, type ActivitySuggestion, type TimerChanged, type TimerState } from "../../api";
import { hintKeys } from "../../test/panelHints";
import { ActivityView } from "./ActivityView";

const mocked = vi.mocked(api, true);

const tt = (id: string, name: string) => ({ id, name, client: null, color: "blue", shade: 0 });
const PROJECTS: ActivityProject[] = [
  { name: "Oscorp", ttProjects: [tt("qrs", "QRS"), tt("zeno", "Zeno")], timerRow: true },
  { name: "Private", ttProjects: [tt("priv", "Private")], timerRow: false },
  { name: "Acme", ttProjects: [], timerRow: false },
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
const RECENT = suggestion({ group: "recentTimers", note: "Lunch talk", project: "Oscorp", ttProjectId: "zeno", tag: "t-recent", display: "Oscorp · Lunch talk" });
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
const pressedIn = (group: string) => {
  const element = screen.queryByRole("group", { name: group });
  return element ? within(element).getAllByRole("button", { pressed: true }).map((b) => b.textContent) : null;
};
const project = () => pressedIn("Project");
const timer = () => pressedIn("Timer");
const timerChoices = () => within(screen.getByRole("group", { name: "Timer" })).getAllByRole("button").map((b) => b.textContent);
const options = () => screen.queryAllByRole("option").map((o) => o.textContent);
const selected = () => screen.queryAllByRole("option").find((o) => o.getAttribute("aria-selected") === "true")?.textContent;
const rejection = (kind: string, message: string) => ({ kind, message, code: null });
const keys = (text: string) => userEvent.keyboard(text);

describe("ActivityView", () => {
  beforeEach(() => vi.clearAllMocks());

  describe("opening", () => {
    it("shows the prompt, the prefilled note and the session's project and timer pressed", async () => {
      await renderView();
      expect(screen.getByText("What are you working on?")).toBeInTheDocument();
      expect(input()).toHaveValue("QRS-1 review");
      expect(project()).toEqual(["Oscorp"]);
      expect(timer()).toEqual(["QRS"]);
    });

    it("lists the Daybook projects in the Project picker", async () => {
      await renderView();
      const picker = within(screen.getByRole("group", { name: "Project" }));
      expect(picker.getAllByRole("button").map((b) => b.textContent)).toEqual(["Oscorp", "Private", "Acme"]);
    });

    it("lists the project keys in picker order", async () => {
      await renderView();
      const hints = within(screen.getByLabelText("Project keys"));
      expect(hints.getByText("Oscorp").parentElement).toHaveTextContent("⌘1");
      expect(hints.getByText("Private").parentElement).toHaveTextContent("⌘2");
      expect(hints.getByText("Acme").parentElement).toHaveTextContent("⌘3");
    });

    it("explains the two rows with icon tooltips", async () => {
      await renderView();
      expect(screen.getByTitle("What you work on")).toBeInTheDocument();
      expect(screen.getByTitle("TimeTracker timer")).toBeInTheDocument();
    });

    it("never says a project is not linked", async () => {
      await renderView();
      await keys("{Meta>}3{/Meta}");
      expect(project()).toEqual(["Acme"]);
      expect(screen.queryByText("Not linked")).not.toBeInTheDocument();
    });

    it("keeps the picker in the note's row for a few projects", async () => {
      await renderView();
      expect(input().parentElement).toContainElement(screen.getByRole("group", { name: "Project" }));
    });
  });

  describe("timer row", () => {
    it("shows None first and then the project's TimeTracker projects when it links several", async () => {
      await renderView();
      expect(timerChoices()).toEqual(["None", "QRS", "Zeno"]);
    });

    it.each([
      ["Private", "one TimeTracker project"],
      ["Acme", "none"],
    ])("is hidden for %s, which links %s", async (name) => {
      await renderView();
      await userEvent.click(screen.getByRole("button", { name }));
      expect(project()).toEqual([name]);
      expect(screen.queryByRole("group", { name: "Timer" })).not.toBeInTheDocument();
      expect(screen.queryByTitle("TimeTracker timer")).not.toBeInTheDocument();
    });

    it.each([
      ["zeno", "Zeno"],
      ["qrs", "QRS"],
      [null, "None"],
    ])("presses the session's timer project %s as %s", async (ttProjectId, label) => {
      await renderView({ ttProjectId });
      expect(timer()).toEqual([label]);
    });

    it("picks a timer project by click without changing the note or the project", async () => {
      await renderView();
      await userEvent.click(screen.getByRole("button", { name: "Zeno" }));
      expect(timer()).toEqual(["Zeno"]);
      expect(project()).toEqual(["Oscorp"]);
      expect(input()).toHaveValue("QRS-1 review");
      expect(input()).toHaveFocus();
    });
  });

  describe("project keys", () => {
    it.each([
      ["{Meta>}2{/Meta}", "Private"],
      ["{Meta>}3{/Meta}", "Acme"],
      ["{Meta>}9{/Meta}", "Oscorp"],
    ])("%s leaves %s pressed", async (pressedKeys, name) => {
      await renderView();
      await keys(pressedKeys);
      expect(project()).toEqual([name]);
    });

    it.each([["{Meta>}0{/Meta}"], ["{Meta>}4{/Meta}"]])("%s does nothing past the project count", async (pressedKeys) => {
      await renderView();
      await keys("{Meta>}2{/Meta}");
      await keys(pressedKeys);
      expect(project()).toEqual(["Private"]);
      expect(input()).toHaveValue("QRS-1 review");
    });

    it("Tab picks the next project and wraps, Shift+Tab goes back", async () => {
      await renderView();
      await keys("{Tab}");
      expect(project()).toEqual(["Private"]);
      await keys("{Tab}{Tab}");
      expect(project()).toEqual(["Oscorp"]);
      await keys("{Shift>}{Tab}{/Shift}");
      expect(project()).toEqual(["Acme"]);
    });

    it("swallows Tab even with a single project", async () => {
      await renderView({ projects: [PROJECTS[1]], project: "Private", ttProjectId: "priv" });
      expect(fireEvent.keyDown(input(), { key: "Tab" })).toBe(false);
      expect(input()).toHaveFocus();
    });

    it("picks a project by click and keeps the focus in the field", async () => {
      await renderView();
      await userEvent.click(screen.getByRole("button", { name: "Private" }));
      expect(project()).toEqual(["Private"]);
      expect(input()).toHaveFocus();
    });

    it.each([
      ["Tab", () => keys("{Tab}{Tab}{Tab}")],
      ["⌘N", async () => keys("{Meta>}2{/Meta}{Meta>}1{/Meta}")],
      ["a click", async () => {
        await userEvent.click(screen.getByRole("button", { name: "Private" }));
        await userEvent.click(screen.getByRole("button", { name: "Oscorp" }));
      }],
    ])("resets the timer row to None when %s changes the project and back", async (_, change) => {
      await renderView();
      await change();
      expect(project()).toEqual(["Oscorp"]);
      expect(timer()).toEqual(["None"]);
    });

    it("keeps the timer project when the picked project is picked again", async () => {
      await renderView({ ttProjectId: "zeno" });
      await keys("{Meta>}1{/Meta}");
      expect(timer()).toEqual(["Zeno"]);
    });
  });

  describe("timer keys", () => {
    it.each([
      ["{Alt>}1{/Alt}", "QRS"],
      ["{Alt>}2{/Alt}", "Zeno"],
      ["{Alt>}0{/Alt}", "None"],
    ])("%s presses %s and leaves the note alone", async (pressedKeys, label) => {
      await renderView({ ttProjectId: label === "QRS" ? "zeno" : "qrs" });
      await keys(pressedKeys);
      expect(timer()).toEqual([label]);
      expect(project()).toEqual(["Oscorp"]);
      expect(input()).toHaveValue("QRS-1 review");
    });

    it("ignores a number past the timer row", async () => {
      await renderView();
      await keys("{Alt>}9{/Alt}");
      expect(timer()).toEqual(["QRS"]);
      expect(input()).toHaveValue("QRS-1 review");
    });

    it("does nothing for a project without the timer row", async () => {
      await renderView({ project: "Private", ttProjectId: "priv" });
      await keys("{Alt>}1{/Alt}");
      expect(project()).toEqual(["Private"]);
      expect(input()).toHaveValue("QRS-1 review");
    });
  });

  describe("suggestions", () => {
    it("shows them under Meetings, Tickets Today and Recent Timers in that order", async () => {
      await renderView();
      expect(options()).toEqual(["Oscorp · Sprint planning", "Oscorp · QRS-7 fix", "Oscorp · Lunch talk"]);
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
      expect(options()).toEqual(["Oscorp · Lunch talk"]);
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
    it("put a suggestion into the field and show its project and timer", async () => {
      await renderView({ ttProjectId: "zeno" });
      await keys("{ArrowDown}");
      expect(input()).toHaveValue("Sprint planning");
      expect(selected()).toBe("Oscorp · Sprint planning");
      expect(project()).toEqual(["Oscorp"]);
      expect(timer()).toEqual(["QRS"]);
    });

    it("fill both rows from a recent timer", async () => {
      await renderView({ project: "Private", ttProjectId: "priv" });
      expect(timer()).toBeNull();
      await keys("{ArrowDown}{ArrowDown}{ArrowDown}");
      expect(input()).toHaveValue("Lunch talk");
      expect(project()).toEqual(["Oscorp"]);
      expect(timer()).toEqual(["Zeno"]);
    });

    it("switch to the project of a suggestion from another Daybook project", async () => {
      const dinner = suggestion({ group: "meetings", note: "Dinner", project: "Private", ttProjectId: "priv", display: "Private · Dinner" });
      await renderView({}, [dinner]);
      await keys("{ArrowDown}");
      expect(project()).toEqual(["Private"]);
      expect(timer()).toBeNull();
    });

    it("keep project and timer for a suggestion without a project", async () => {
      await renderView({ ttProjectId: "zeno" }, [suggestion({ group: "meetings", note: "Call", display: "Call" })]);
      await keys("{ArrowDown}");
      expect(input()).toHaveValue("Call");
      expect(project()).toEqual(["Oscorp"]);
      expect(timer()).toEqual(["Zeno"]);
    });

    it("wrap from your own input to the last suggestion and back to your own input", async () => {
      await renderView();
      await keys("{ArrowUp}");
      expect(selected()).toBe("Oscorp · Lunch talk");
      await keys("{ArrowDown}");
      expect(selected()).toBeUndefined();
      expect(input()).toHaveValue("QRS-1 review");
      expect(project()).toEqual(["Oscorp"]);
      expect(timer()).toEqual(["QRS"]);
    });
  });

  describe("taking a suggestion over", () => {
    it("makes it your own input when you type", async () => {
      await renderView();
      await keys("{ArrowDown}");
      await keys("!");
      expect(input()).toHaveValue("Sprint planning!");
      expect(selected()).toBeUndefined();
    });

    it("keeps a recent timer's project and timer when you type", async () => {
      await renderView();
      await keys("{ArrowDown}{ArrowDown}{ArrowDown}");
      await keys("x");
      expect(input()).toHaveValue("Lunch talkx");
      expect(project()).toEqual(["Oscorp"]);
      expect(timer()).toEqual(["Zeno"]);
    });

    it("keeps the note when you pick another project", async () => {
      await renderView();
      await keys("{ArrowDown}{ArrowDown}{ArrowDown}");
      await userEvent.click(screen.getByRole("button", { name: "Private" }));
      expect(input()).toHaveValue("Lunch talk");
      expect(project()).toEqual(["Private"]);
      expect(selected()).toBeUndefined();
    });

    it("keeps the note when you pick another timer project", async () => {
      await renderView();
      await keys("{ArrowDown}{ArrowDown}{ArrowDown}");
      await keys("{Alt>}1{/Alt}");
      expect(input()).toHaveValue("Lunch talk");
      expect(timer()).toEqual(["QRS"]);
      expect(selected()).toBeUndefined();
    });
  });

  describe("submitting", () => {
    it("sends the shown project, timer project and note with the session's tag while they stay", async () => {
      await renderView();
      await keys("{Enter}");
      expect(mocked.activitySubmit).toHaveBeenCalledWith("switch", { project: "Oscorp", ttProjectId: "qrs", note: "QRS-1 review", tag: "t-session" });
    });

    it("sends the mode of the session", async () => {
      await renderView({ mode: "checkin" });
      await keys("{Enter}");
      expect(mocked.activitySubmit).toHaveBeenCalledWith("checkin", expect.objectContaining({ project: "Oscorp", ttProjectId: "qrs" }));
    });

    it("sends the note verbatim and keeps the tag when only whitespace differs", async () => {
      await renderView();
      await userEvent.clear(input());
      await userEvent.type(input(), "QRS-1 review  {Enter}");
      expect(mocked.activitySubmit).toHaveBeenCalledWith("switch", { project: "Oscorp", ttProjectId: "qrs", note: "QRS-1 review  ", tag: "t-session" });
    });

    it("drops the session's tag for another note on the same timer project", async () => {
      await renderView();
      await userEvent.clear(input());
      await userEvent.type(input(), "Something else{Enter}");
      expect(mocked.activitySubmit).toHaveBeenCalledWith("switch", { project: "Oscorp", ttProjectId: "qrs", note: "Something else", tag: null });
    });

    it("sends the raw None choice of a project with several links, with the session's tag while it stays", async () => {
      await renderView({ ttProjectId: null });
      await keys("{Enter}");
      expect(mocked.activitySubmit).toHaveBeenCalledWith("switch", { project: "Oscorp", ttProjectId: null, note: "QRS-1 review", tag: "t-session" });
    });

    it("sends the raw choice of a project with several links", async () => {
      await renderView({ ttProjectId: null });
      await keys("{Alt>}2{/Alt}{Enter}");
      expect(mocked.activitySubmit).toHaveBeenCalledWith("switch", { project: "Oscorp", ttProjectId: "zeno", note: "QRS-1 review", tag: null });
    });

    it("sends the picked timer project without a tag and keeps the note", async () => {
      await renderView();
      await keys("{Alt>}2{/Alt}{Enter}");
      expect(mocked.activitySubmit).toHaveBeenCalledWith("switch", { project: "Oscorp", ttProjectId: "zeno", note: "QRS-1 review", tag: null });
    });

    it("sends no timer project with None, keeping project and note", async () => {
      await renderView();
      await keys("{Alt>}0{/Alt}{Enter}");
      expect(mocked.activitySubmit).toHaveBeenCalledWith("switch", { project: "Oscorp", ttProjectId: null, note: "QRS-1 review", tag: null });
    });

    it("sends no timer project after the project changes and back to a project with several links", async () => {
      await renderView();
      await keys("{Meta>}2{/Meta}{Meta>}1{/Meta}{Enter}");
      expect(mocked.activitySubmit).toHaveBeenCalledWith("switch", { project: "Oscorp", ttProjectId: null, note: "QRS-1 review", tag: null });
    });

    it("sends no timer choice for a project with one link, since its row is hidden", async () => {
      await renderView();
      await keys("{Meta>}2{/Meta}{Enter}");
      expect(mocked.activitySubmit).toHaveBeenCalledWith("switch", { project: "Private", ttProjectId: null, note: "QRS-1 review", tag: null });
    });

    it("sends no timer project for a project without links", async () => {
      await renderView();
      await keys("{Meta>}3{/Meta}{Enter}");
      expect(mocked.activitySubmit).toHaveBeenCalledWith("switch", { project: "Acme", ttProjectId: null, note: "QRS-1 review", tag: null });
    });

    it("sends a highlighted recent timer with its project, timer project, note and tag", async () => {
      await renderView();
      await keys("{ArrowDown}{ArrowDown}{ArrowDown}{Enter}");
      expect(mocked.activitySubmit).toHaveBeenCalledWith("switch", { project: "Oscorp", ttProjectId: "zeno", note: "Lunch talk", tag: "t-recent" });
    });

    it("sends a suggestion without a project with your project and timer", async () => {
      await renderView({ ttProjectId: "zeno" }, [suggestion({ group: "meetings", note: "Call", display: "Call" })]);
      await keys("{ArrowDown}{Enter}");
      expect(mocked.activitySubmit).toHaveBeenCalledWith("switch", { project: "Oscorp", ttProjectId: "zeno", note: "Call", tag: null });
    });

    it("drops a recent timer's tag as soon as you edit its note", async () => {
      await renderView();
      await keys("{ArrowDown}{ArrowDown}{ArrowDown}");
      await keys("x{Enter}");
      expect(mocked.activitySubmit).toHaveBeenLastCalledWith("switch", { project: "Oscorp", ttProjectId: "zeno", note: "Lunch talkx", tag: null });
    });

    it("drops a recent timer's tag when you pick another timer project", async () => {
      await renderView();
      await keys("{ArrowDown}{ArrowDown}{ArrowDown}");
      await keys("{Alt>}1{/Alt}{Enter}");
      expect(mocked.activitySubmit).toHaveBeenLastCalledWith("switch", { project: "Oscorp", ttProjectId: "qrs", note: "Lunch talk", tag: null });
    });

    it("starts a clicked suggestion", async () => {
      await renderView();
      await userEvent.click(screen.getByRole("option", { name: "Oscorp · Lunch talk" }));
      expect(mocked.activitySubmit).toHaveBeenCalledWith("switch", { project: "Oscorp", ttProjectId: "zeno", note: "Lunch talk", tag: "t-recent" });
    });
  });

  describe("footer", () => {
    it("lists the keys", async () => {
      await renderView();
      expect(hintKeys("Next project")).toEqual(["⇥"]);
      expect(hintKeys("Suggestions")).toEqual(["↑", "↓"]);
      expect(hintKeys("Start")).toEqual(["↵"]);
      expect(hintKeys("Cancel")).toEqual(["esc"]);
    });

    it("leaves out the next project with a single project", async () => {
      await renderView({ projects: [PROJECTS[1]], project: "Private", ttProjectId: "priv" });
      expect(hintKeys("Next project")).toBeNull();
      expect(hintKeys("Start")).toEqual(["↵"]);
    });

    it("keeps Start in a switch even when nothing changed", async () => {
      await renderView({ mode: "switch", ttProjectId: null });
      expect(hintKeys("Start")).toEqual(["↵"]);
      expect(hintKeys("Still on it")).toBeNull();
    });
  });

  describe("still on it", () => {
    it("shows in a check-in without a timer while project, None and note are unchanged", async () => {
      await renderView({ mode: "checkin", ttProjectId: null });
      expect(timer()).toEqual(["None"]);
      expect(hintKeys("Still on it")).toEqual(["↵"]);
      expect(hintKeys("Start")).toBeNull();
    });

    it.each([
      ["another note", () => userEvent.type(input(), "!")],
      ["another project", () => keys("{Meta>}2{/Meta}")],
      ["a timer project", () => keys("{Alt>}1{/Alt}")],
    ])("turns into Start with %s", async (_, change) => {
      await renderView({ mode: "checkin", ttProjectId: null });
      await change();
      expect(hintKeys("Start")).toEqual(["↵"]);
      expect(hintKeys("Still on it")).toBeNull();
    });

    it("comes back when the project changes and returns", async () => {
      await renderView({ mode: "checkin", ttProjectId: null });
      await keys("{Meta>}2{/Meta}");
      expect(hintKeys("Start")).toEqual(["↵"]);
      await keys("{Meta>}1{/Meta}");
      expect(hintKeys("Still on it")).toEqual(["↵"]);
    });

    it("ignores whitespace around the note", async () => {
      await renderView({ mode: "checkin", ttProjectId: null });
      await userEvent.type(input(), "  ");
      expect(hintKeys("Still on it")).toEqual(["↵"]);
    });

    it("shows while the live timer matches the shown timer project and note, until it differs", async () => {
      await renderView({ mode: "checkin" }, ALL, { tt: "available", timer: timerState({ note: " QRS-1 review " }) });
      await vi.waitFor(() => expect(hintKeys("Still on it")).toEqual(["↵"]));
      await keys("{Alt>}2{/Alt}");
      expect(hintKeys("Start")).toEqual(["↵"]);
      await keys("{Alt>}1{/Alt}");
      expect(hintKeys("Still on it")).toEqual(["↵"]);
      await userEvent.type(input(), "Other");
      expect(hintKeys("Start")).toEqual(["↵"]);
    });

    it("stays Start in a check-in when a timer runs and None is shown", async () => {
      await renderView({ mode: "checkin", ttProjectId: null }, ALL, { tt: "available", timer: timerState({}) });
      await vi.waitFor(() => expect(mocked.timetrackerTimer).toHaveBeenCalled());
      await vi.waitFor(() => expect(hintKeys("Start")).toEqual(["↵"]));
    });

    it("stays Start in a switch when the live timer matches", async () => {
      await renderView({ mode: "switch" }, ALL, { tt: "available", timer: timerState({}) });
      await vi.waitFor(() => expect(mocked.timetrackerTimer).toHaveBeenCalled());
      expect(hintKeys("Start")).toEqual(["↵"]);
    });
  });

  describe("errors", () => {
    it("shows that TimeTracker isn't running after a refused submit and keeps the note and the pick", async () => {
      mocked.activitySubmit.mockRejectedValueOnce(rejection("timetracker_not_running", "not running"));
      await renderView();
      await keys("{Alt>}2{/Alt}");
      await userEvent.clear(input());
      await userEvent.type(input(), "Typed{Enter}");
      expect(await screen.findByText("TimeTracker isn't running.")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Open TimeTracker" })).toBeInTheDocument();
      expect(input()).toHaveValue("Typed");
      expect(timer()).toEqual(["Zeno"]);
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
      await keys("{Enter}");
      expect(await screen.findByText("TimeTracker isn't installed (Settings → Connections → TimeTracker).")).toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "Open TimeTracker" })).not.toBeInTheDocument();
    });

    it("shows any other message and clears it on the next keystroke", async () => {
      mocked.activitySubmit.mockRejectedValueOnce(rejection("io", "Disk full"));
      await renderView();
      await keys("{Enter}");
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

    it("restarts the minute when you pick a timer project", async () => {
      await renderView({ mode: "checkin" });
      focusWindow();
      await vi.advanceTimersByTimeAsync(50_000);
      await userEvent.keyboard("{Alt>}2{/Alt}");
      await vi.advanceTimersByTimeAsync(50_000);
      expect(mocked.panelClose).not.toHaveBeenCalled();
      await vi.advanceTimersByTimeAsync(11_000);
      expect(mocked.panelClose).toHaveBeenCalled();
    });

    it("restarts the minute when you pick another project", async () => {
      await renderView({ mode: "checkin" });
      focusWindow();
      await vi.advanceTimersByTimeAsync(50_000);
      await userEvent.keyboard("{Meta>}2{/Meta}");
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
