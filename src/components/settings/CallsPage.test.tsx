import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type AppRef, type CallsInfo, type Joining, type MeetingRule, type Settings } from "../../api";
import { resetStore, useStore } from "../../state/store";
import { settings } from "../../test/settings";
import { AccessibilityRow, CallsPage } from "./CallsPage";

const mocked = vi.mocked(api, true);

const SLACK: AppRef = { app: "Slack", bundleId: "com.tinyspeck.slackmacgap" };

function customRule(patch: Partial<MeetingRule> = {}): MeetingRule {
  return { enabled: true, app: "Slack", bundleId: SLACK.bundleId, windowTitle: null, slack: { keepStatus: false, emoji: ":speech_balloon:", text: "Huddling", presence: "auto" }, ...patch };
}

async function renderPage({ meetings = {}, joining = {}, info = {} }: { meetings?: Partial<Settings["meetings"]>; joining?: Partial<Joining>; info?: Partial<CallsInfo> } = {}) {
  const base = settings();
  const s = settings({ meetings: { ...base.meetings, ...meetings }, joining: { ...base.joining, ...joining } });
  mocked.callsInfo.mockResolvedValue({ ...DEFAULT_INFO, ...info });
  const set = vi.fn();
  render(<CallsPage s={s} set={set} />);
  await screen.findByRole("switch", { name: "Watch apps" });
  return { s, set };
}

const group = (title: string) => within(screen.getByRole("heading", { name: title }).closest("section")!);
const row = (label: string) => within(screen.getByText(label).closest(".settings-row") as HTMLElement);

const DEFAULT_INFO: CallsInfo = {
  detecting: true,
  active: null,
  accessibility: true,
  missing: [],
  error: null,
  builtIns: settings().meetings.rules.map((rule) => ({ rule, check: `Recognizes ${rule.app} calls.` })),
};

function resetMocks() {
  vi.clearAllMocks();
  resetStore({ settings: settings() });
  mocked.callsInfo.mockResolvedValue(DEFAULT_INFO);
  mocked.callsPickApp.mockResolvedValue(null);
  mocked.callsRequestAccess.mockResolvedValue(true);
}

describe("CallsPage", () => {
  beforeEach(resetMocks);

  describe("Watch apps", () => {
    it.each([true, false])("reflects detection %s and saves the flipped value", async (detection) => {
      const base = settings().meetings;
      const { set } = await renderPage({ meetings: { detection } });
      const toggle = screen.getByRole("switch", { name: "Watch apps" });
      expect(toggle).toHaveProperty("checked", detection);
      await userEvent.click(toggle);
      expect(set).toHaveBeenCalledWith({ meetings: { ...base, detection: !detection } });
    });

    it("hides the call state while detection is not running", async () => {
      await renderPage({ info: { detecting: false } });
      expect(screen.queryByRole("button", { name: "Check Now" })).not.toBeInTheDocument();
    });
  });

  describe("call state", () => {
    it("says No call when none is running", async () => {
      await renderPage();
      expect(await screen.findByText("No call")).toBeInTheDocument();
    });

    it("names the app, start and title of the active call", async () => {
      await renderPage({ info: { active: { app: "Zoom", title: "Weekly", since: "10:15" } } });
      expect(await screen.findByText("In a Zoom call since 10:15: Weekly")).toBeInTheDocument();
    });

    it("runs the meetings:check command from Check Now", async () => {
      await renderPage();
      await userEvent.click(screen.getByRole("button", { name: "Check Now" }));
      expect(mocked.runCommand).toHaveBeenCalledWith("meetings:check");
    });

    it("shows the last error as the row's description", async () => {
      await renderPage({ info: { error: "Chrome did not answer" } });
      expect(await screen.findByText("Chrome did not answer")).toBeInTheDocument();
    });
  });

  describe("app cards", () => {
    it("titles one card per rule by its app", async () => {
      await renderPage({ meetings: { rules: [...settings().meetings.rules, customRule()] } });
      expect(screen.getAllByRole("heading", { level: 3 }).map((h) => h.textContent)).toEqual(expect.arrayContaining(["Zoom", "Teams", "Meet", "Slack"]));
    });

    it("shows a built-in rule's check text instead of the app and window title fields", async () => {
      await renderPage();
      const zoom = group("Zoom");
      expect(zoom.getByText("Recognizes Zoom calls.")).toBeInTheDocument();
      expect(zoom.queryByText("macOS app")).not.toBeInTheDocument();
      expect(zoom.queryByLabelText("Window title")).not.toBeInTheDocument();
    });

    it("shows a custom rule's bundle id and window title field", async () => {
      await renderPage({ meetings: { rules: [customRule({ windowTitle: "Huddle" })] } });
      const slack = group("Slack");
      expect(slack.getByText(SLACK.bundleId)).toBeInTheDocument();
      expect(slack.getByLabelText("Window title")).toHaveValue("Huddle");
    });

    it("saves the Slack status shown during a call on that rule only", async () => {
      const { s, set } = await renderPage();
      const teams = group("Teams");
      await userEvent.click(teams.getByRole("button", { name: "Edit Slack status" }));
      const text = teams.getByLabelText("Text");
      await userEvent.clear(text);
      await userEvent.type(text, "Calling{Enter}");
      expect(set).toHaveBeenLastCalledWith({
        meetings: { ...s.meetings, rules: s.meetings.rules.map((r) => (r.app === "Teams" ? { ...r, slack: { ...r.slack, text: "Calling" } } : r)) },
      });
    });

    it("saves the flipped enabled state from Recognize calls", async () => {
      const { s, set } = await renderPage();
      await userEvent.click(group("Teams").getByRole("switch", { name: "Recognize calls" }));
      expect(set).toHaveBeenCalledWith({ meetings: { ...s.meetings, rules: s.meetings.rules.map((r) => (r.app === "Teams" ? { ...r, enabled: false } : r)) } });
    });

    it("saves a trimmed name", async () => {
      const { s, set } = await renderPage();
      const name = group("Zoom").getByLabelText("Name");
      await userEvent.clear(name);
      await userEvent.type(name, "  Zoom Work  {Enter}");
      expect(set).toHaveBeenCalledWith({ meetings: { ...s.meetings, rules: s.meetings.rules.map((r) => (r.app === "Zoom" ? { ...r, app: "Zoom Work" } : r)) } });
    });

    it("replaces the bundle id with the app chosen by Choose…", async () => {
      mocked.callsPickApp.mockResolvedValue({ app: "Discord", bundleId: "com.hnc.Discord" });
      const { set } = await renderPage({ meetings: { rules: [customRule()] } });
      await userEvent.click(group("Slack").getByRole("button", { name: "Choose…" }));
      await vi.waitFor(() => expect(set).toHaveBeenCalled());
      expect(set.mock.lastCall![0].meetings.rules).toEqual([customRule({ bundleId: "com.hnc.Discord" })]);
    });

    it("saves the window title, and null when it is emptied", async () => {
      const { set } = await renderPage({ meetings: { rules: [customRule({ windowTitle: "Huddle" })] } });
      const field = group("Slack").getByLabelText("Window title");
      await userEvent.clear(field);
      await userEvent.tab();
      expect(set.mock.lastCall![0].meetings.rules).toEqual([customRule({ windowTitle: null })]);

      await userEvent.type(field, "  Standup ");
      await userEvent.tab();
      expect(set.mock.lastCall![0].meetings.rules).toEqual([customRule({ windowTitle: "Standup" })]);
    });

    it("moves a rule down and up", async () => {
      const { s, set } = await renderPage();
      const [zoom, teams, meet] = s.meetings.rules;
      await userEvent.click(group("Zoom").getByRole("button", { name: "Move Down" }));
      expect(set.mock.lastCall![0].meetings.rules).toEqual([teams, zoom, meet]);
      await userEvent.click(group("Meet").getByRole("button", { name: "Move Up" }));
      expect(set.mock.lastCall![0].meetings.rules).toEqual([zoom, meet, teams]);
    });

    it("removes a rule with Remove App", async () => {
      const { s, set } = await renderPage();
      await userEvent.click(group("Teams").getByRole("button", { name: "Remove App" }));
      expect(set.mock.lastCall![0].meetings.rules).toEqual(s.meetings.rules.filter((r) => r.app !== "Teams"));
    });
  });

  describe("adding apps", () => {
    it("offers only the built-ins missing from the rules and appends the chosen one", async () => {
      const { s, set } = await renderPage({ meetings: { rules: settings().meetings.rules.filter((r) => r.app !== "Teams") } });
      expect(screen.queryByRole("button", { name: "Add Zoom" })).not.toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "Add Meet" })).not.toBeInTheDocument();
      await userEvent.click(screen.getByRole("button", { name: "Add Teams" }));
      expect(set.mock.lastCall![0].meetings.rules).toEqual([...s.meetings.rules, settings().meetings.rules.find((r) => r.app === "Teams")]);
    });

    it("offers no built-in to add when all are present", async () => {
      await renderPage();
      expect(screen.queryByRole("button", { name: /^Add (Zoom|Teams|Meet)$/ })).not.toBeInTheDocument();
    });

    it("appends an enabled rule for the picked app from Add Other App…", async () => {
      mocked.callsPickApp.mockResolvedValue(SLACK);
      const { s, set } = await renderPage();
      await userEvent.click(screen.getByRole("button", { name: "Add Other App…" }));
      await vi.waitFor(() => expect(set).toHaveBeenCalled());
      const added = set.mock.lastCall![0].meetings.rules.at(-1);
      expect(set.mock.lastCall![0].meetings.rules).toHaveLength(s.meetings.rules.length + 1);
      expect(added).toMatchObject({ enabled: true, app: "Slack", bundleId: SLACK.bundleId, windowTitle: null, slack: { text: "In a Slack call" } });
    });

    it("changes nothing when the pick is cancelled", async () => {
      mocked.callsPickApp.mockResolvedValue(null);
      const { set } = await renderPage();
      await userEvent.click(screen.getByRole("button", { name: "Add Other App…" }));
      await vi.waitFor(() => expect(mocked.callsPickApp).toHaveBeenCalled());
      expect(set).not.toHaveBeenCalled();
    });

    it("changes nothing and shows an error toast when the pick fails", async () => {
      mocked.callsPickApp.mockRejectedValue({ kind: "io", message: "Panel failed", code: null });
      const { set } = await renderPage();
      await userEvent.click(screen.getByRole("button", { name: "Add Other App…" }));
      await vi.waitFor(() => expect(useStore.getState().toasts[0]?.message).toContain("Couldn't use this app"));
      expect(set).not.toHaveBeenCalled();
    });
  });

  describe("permissions", () => {
    it("offers Allow Access… for an enabled custom rule without Accessibility", async () => {
      await renderPage({ meetings: { rules: [customRule()] }, info: { accessibility: false } });
      await userEvent.click(await screen.findByRole("button", { name: "Allow Access…" }));
      expect(mocked.callsRequestAccess).toHaveBeenCalledWith("accessibility");
    });

    it.each([
      ["only built-in rules exist", { accessibility: false }, undefined],
      ["the custom rule is disabled", { accessibility: false }, [customRule({ enabled: false })]],
      ["Accessibility is allowed", { accessibility: true }, [customRule()]],
    ] as const)("hides Allow Access… when %s", async (_, info, rules) => {
      await renderPage({ meetings: rules ? { rules: [...rules] } : {}, info });
      await screen.findByText("No call");
      expect(screen.queryByRole("button", { name: "Allow Access…" })).not.toBeInTheDocument();
    });

    it("offers Open Privacy Settings when Automation is missing", async () => {
      await renderPage({ info: { missing: ["automation"] } });
      await userEvent.click(await screen.findByRole("button", { name: "Open Privacy Settings" }));
      expect(mocked.callsRequestAccess).toHaveBeenCalledWith("automation");
    });

    it("hides Open Privacy Settings when nothing is missing", async () => {
      await renderPage();
      await screen.findByText("No call");
      expect(screen.queryByRole("button", { name: "Open Privacy Settings" })).not.toBeInTheDocument();
    });

    it("shows an error toast when the request fails", async () => {
      mocked.callsRequestAccess.mockRejectedValue({ kind: "io", message: "No prompt", code: null });
      await renderPage({ info: { missing: ["automation"] } });
      await userEvent.click(await screen.findByRole("button", { name: "Open Privacy Settings" }));
      await vi.waitFor(() => expect(useStore.getState().toasts[0]?.message).toContain("Couldn't ask for access"));
    });
  });

  describe("Joining a Call", () => {
    const camo: AppRef = { app: "Camo", bundleId: "com.reincubate.macos.cam" };

    it("lists the Start first apps and removes one", async () => {
      const { set } = await renderPage({ joining: { startFirst: [camo, SLACK] } });
      const list = within(screen.getByRole("list", { name: "Start first" }));
      expect(list.getByText("Camo")).toBeInTheDocument();
      await userEvent.click(list.getByRole("button", { name: "Remove Camo" }));
      expect(set.mock.lastCall![0].joining.startFirst).toEqual([SLACK]);
    });

    it("adds a picked app to Start first", async () => {
      mocked.callsPickApp.mockResolvedValue(SLACK);
      const { set } = await renderPage({ joining: { startFirst: [camo] } });
      await userEvent.click(screen.getByRole("button", { name: "Add App…" }));
      await vi.waitFor(() => expect(set).toHaveBeenCalled());
      expect(set.mock.lastCall![0].joining.startFirst).toEqual([camo, SLACK]);
    });

    it("does not add an app that is already in Start first", async () => {
      mocked.callsPickApp.mockResolvedValue(camo);
      const { set } = await renderPage({ joining: { startFirst: [camo] } });
      await userEvent.click(screen.getByRole("button", { name: "Add App…" }));
      await vi.waitFor(() => expect(mocked.callsPickApp).toHaveBeenCalled());
      expect(set).not.toHaveBeenCalled();
    });

    it("shows the chosen app or Default per service", async () => {
      await renderPage({ joining: { openWith: { Zoom: null, Teams: SLACK, Meet: { app: "Google Meet", bundleId: "x" } } } });
      expect(row("Open Zoom in").getByText("Default")).toBeInTheDocument();
      expect(row("Open Teams in").getByText("Slack")).toBeInTheDocument();
      expect(row("Open Google Meet in").getByText("Google Meet")).toBeInTheDocument();
      expect(row("Open Zoom in").queryByRole("button", { name: "Use Default" })).not.toBeInTheDocument();
    });

    it("sets a service back to the default", async () => {
      const { s, set } = await renderPage({ joining: { openWith: { Zoom: null, Teams: SLACK, Meet: null } } });
      await userEvent.click(row("Open Teams in").getByRole("button", { name: "Use Default" }));
      expect(set).toHaveBeenCalledWith({ joining: { ...s.joining, openWith: { Zoom: null, Teams: null, Meet: null } } });
    });

    it.each(["Meet", "Zoom", "Teams"] as const)("sets the app for %s from Choose…", async (service) => {
      mocked.callsPickApp.mockResolvedValue(SLACK);
      const { set } = await renderPage({ joining: { openWith: { Zoom: null, Teams: null, Meet: null } } });
      await userEvent.click(screen.getByRole("button", { name: `Choose the app for ${service}` }));
      await vi.waitFor(() => expect(set).toHaveBeenCalled());
      expect(set.mock.lastCall![0].joining.openWith).toEqual({ Zoom: null, Teams: null, Meet: null, [service]: SLACK });
    });

    it("leaves the settings alone when the pick fails", async () => {
      mocked.callsPickApp.mockRejectedValue({ kind: "io", message: "Panel failed", code: null });
      const { set } = await renderPage();
      await userEvent.click(screen.getByRole("button", { name: "Choose the app for Zoom" }));
      await vi.waitFor(() => expect(useStore.getState().toasts[0]?.message).toContain("Couldn't use this app"));
      expect(set).not.toHaveBeenCalled();
    });
  });
});

describe("AccessibilityRow", () => {
  beforeEach(resetMocks);

  it("shows Allowed when Accessibility is granted", async () => {
    render(<AccessibilityRow />);
    expect(await screen.findByText("Allowed")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Allow Access…" })).not.toBeInTheDocument();
  });

  it("requests Accessibility from Allow Access… when it is missing", async () => {
    mocked.callsInfo.mockResolvedValue({ ...DEFAULT_INFO, accessibility: false });
    render(<AccessibilityRow />);
    await userEvent.click(await screen.findByRole("button", { name: "Allow Access…" }));
    expect(mocked.callsRequestAccess).toHaveBeenCalledWith("accessibility");
  });
});
