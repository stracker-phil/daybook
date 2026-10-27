import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type EndOfDay, type Settings } from "../../api";
import { resetStore } from "../../state/store";
import { settings } from "../../test/settings";
import { SettingsWindow } from "./SettingsWindow";

const mocked = vi.mocked(api, true);

function renderPage(patch: Partial<Settings> = {}) {
  resetStore({ settings: settings(patch) });
  render(<SettingsWindow page="workflows" onPage={() => {}} />);
}

const lastSaved = () => mocked.updateSettings.mock.lastCall![0];
const channelMessages = () => screen.getAllByRole("textbox", { name: "Channel message" });
const field = (name: string) => screen.getByRole("textbox", { name });

async function replace(input: HTMLElement, text: string) {
  await userEvent.clear(input);
  if (text) await userEvent.type(input, text);
}

describe("WorkflowsPage", () => {
  beforeEach(() => vi.clearAllMocks());

  describe("status workflows", () => {
    it("shows each status switch from the saved workflows", () => {
      const base = settings();
      renderPage({ slack: { ...base.slack, manualWorkflows: { working: true, break: false, off: true } } });
      expect(screen.getByRole("switch", { name: "Working and present" })).toBeChecked();
      expect(screen.getByRole("switch", { name: "Taking a break" })).not.toBeChecked();
      expect(screen.getByRole("switch", { name: "Done for today" })).toBeChecked();
    });

    it.each([
      ["Working and present", { working: false, break: true, off: true }],
      ["Taking a break", { working: true, break: false, off: true }],
      ["Done for today", { working: true, break: true, off: false }],
    ])("saves %s switched off on that status only", async (label, expected) => {
      renderPage();
      await userEvent.click(screen.getByRole("switch", { name: label }));
      expect(lastSaved().slack.manualWorkflows).toEqual(expected);
    });

    it("keeps the rest of the Slack settings when a workflow changes", async () => {
      renderPage();
      await userEvent.click(screen.getByRole("switch", { name: "Taking a break" }));
      expect(lastSaved().slack.statuses).toEqual(settings().slack.statuses);
    });
  });

  describe("status timer", () => {
    it("shows the timer switch on by default", () => {
      renderPage();
      expect(screen.getByRole("switch", { name: "Start and stop the timer" })).toBeChecked();
    });

    it("explains in the Finished calls row that matching calls wait as suggestions", () => {
      renderPage();
      expect(screen.getByText("Finished calls")).toBeInTheDocument();
      expect(screen.getByText(/suggestion to approve/)).toBeInTheDocument();
    });

    it("saves the switch off as timetracker.statusTimer and keeps the other timetracker fields", async () => {
      renderPage();
      await userEvent.click(screen.getByRole("switch", { name: "Start and stop the timer" }));
      expect(lastSaved().timetracker).toEqual({ ...settings().timetracker, statusTimer: false });
    });
  });

  describe("Slack channel", () => {
    it("saves the trimmed channel as slack.channel", async () => {
      renderPage();
      await replace(field("Channel"), "  #team-status ");
      await userEvent.keyboard("{Enter}");
      expect(lastSaved().slack).toEqual({ ...settings().slack, channel: "#team-status" });
    });

    it("saves a cleared channel as empty, so nothing is posted", async () => {
      const base = settings();
      renderPage({ slack: { ...base.slack, channel: "#team-status" } });
      expect(field("Channel")).toHaveValue("#team-status");
      await replace(field("Channel"), "");
      await userEvent.tab();
      expect(lastSaved().slack.channel).toBe("");
    });
  });

  describe("Slack status of a workflow", () => {
    const statusFields = () => screen.getAllByRole("button", { name: "Edit Slack status" });

    it.each([
      [0, "working"],
      [1, "break"],
      [2, "off"],
    ] as const)("saves the edited text of status field %i as slack.statuses.%s only", async (index, key) => {
      renderPage();
      await userEvent.click(statusFields()[index]);
      await replace(screen.getByLabelText("Text"), "Heads down");
      await userEvent.keyboard("{Enter}");
      const base = settings().slack;
      expect(lastSaved().slack).toEqual({ ...base, statuses: { ...base.statuses, [key]: { ...base.statuses[key], text: "Heads down" } } });
    });

    it("saves a changed presence of the Break status", async () => {
      renderPage();
      await userEvent.click(statusFields()[1]);
      await userEvent.click(within(screen.getByRole("group", { name: "Presence" })).getByRole("button", { name: "No change" }));
      expect(lastSaved().slack.statuses.break.presence).toBe("keep");
    });
  });

  describe("channel messages", () => {
    it("shows the saved message of each status", () => {
      renderPage();
      expect(channelMessages().map((input) => (input as HTMLInputElement).value)).toEqual(["re", "afk", "off for today :wave:"]);
    });

    it.each([
      [0, "working"],
      [1, "break"],
      [2, "off"],
    ] as const)("saves the edited message of field %i as statusMessages.%s on Enter", async (index, key) => {
      renderPage();
      await replace(channelMessages()[index], "new text");
      await userEvent.keyboard("{Enter}");
      expect(lastSaved().statusMessages).toEqual({ ...settings().statusMessages, [key]: "new text" });
    });

    it("saves an edited message on blur", async () => {
      renderPage();
      await replace(channelMessages()[1], "lunch");
      await userEvent.tab();
      expect(lastSaved().statusMessages.break).toBe("lunch");
    });

    it("saves a cleared message as empty, so nothing is posted", async () => {
      renderPage();
      await replace(channelMessages()[0], "");
      await userEvent.tab();
      expect(lastSaved().statusMessages.working).toBe("");
    });

    it("saves nothing while the field is only being typed in", async () => {
      renderPage();
      await replace(channelMessages()[0], "half");
      expect(mocked.updateSettings).not.toHaveBeenCalled();
    });

    it("saves the first message of the day as statusMessages.firstWorking", async () => {
      renderPage();
      await replace(field("First of the day"), "good morning");
      await userEvent.keyboard("{Enter}");
      expect(lastSaved().statusMessages).toEqual({ ...settings().statusMessages, firstWorking: "good morning" });
    });

    it("hints the Working message in the first-of-the-day placeholder", () => {
      renderPage();
      expect(field("First of the day")).toHaveAttribute("placeholder", "Same as above: re");
    });
  });

  describe("ask first", () => {
    it("turns the end-of-day check off and says Done for today is set right away", async () => {
      renderPage();
      expect(screen.getByRole("switch", { name: "Ask first" })).toBeChecked();
      await userEvent.click(screen.getByRole("switch", { name: "Ask first" }));
      expect(lastSaved().endOfDay.enabled).toBe(false);
      expect(await screen.findByText("Done for today is set right away.")).toBeInTheDocument();
    });

    it("turns the check back on", async () => {
      renderPage({ endOfDay: { ...settings().endOfDay, enabled: false } });
      expect(screen.getByRole("switch", { name: "Ask first" })).not.toBeChecked();
      await userEvent.click(screen.getByRole("switch", { name: "Ask first" }));
      expect(lastSaved().endOfDay.enabled).toBe(true);
    });
  });

  describe("end-of-day check", () => {
    const withReminder = () => renderPage({ endOfDay: { ...settings().endOfDay, reminder: "Book your hours" } });

    it("shows the saved check", () => {
      withReminder();
      expect(field("Reminder")).toHaveValue("Book your hours");
      expect(field("Question")).toHaveValue("Is today fully tracked?");
      expect(field("Link")).toHaveValue("https://example.com/timesheets");
      expect(field("Message if yes")).toHaveValue("Tracked: Done | _{message}_");
      expect(field("Message if not yet")).toHaveValue("Tracked: Incomplete | _{message}_");
    });

    it.each<[string, keyof EndOfDay]>([
      ["Reminder", "reminder"],
      ["Question", "question"],
      ["Link", "url"],
      ["Message if yes", "done"],
      ["Message if not yet", "incomplete"],
    ])("saves an edited %s", async (label, key) => {
      renderPage();
      await replace(field(label), "changed");
      await userEvent.tab();
      expect(lastSaved().endOfDay).toEqual({ ...settings().endOfDay, [key]: "changed" });
    });

    it.each(["Question", "Message if yes", "Message if not yet"])("saves nothing when the required %s is cleared", async (label) => {
      renderPage();
      await replace(field(label), "");
      await userEvent.tab();
      expect(mocked.updateSettings).not.toHaveBeenCalled();
    });

    it("saves nothing when a required field is only whitespace", async () => {
      renderPage();
      await replace(field("Question"), "   ");
      await userEvent.tab();
      expect(mocked.updateSettings).not.toHaveBeenCalled();
    });

    it.each<[string, keyof EndOfDay]>([
      ["Reminder", "reminder"],
      ["Link", "url"],
    ])("saves a cleared %s as empty", async (label, key) => {
      withReminder();
      await replace(field(label), "");
      await userEvent.tab();
      expect(lastSaved().endOfDay[key]).toBe("");
    });
  });

  describe("daily focus", () => {
    it("shows the saved focus settings", () => {
      renderPage({ focus: { enabled: false, endOfDayTitle: "Wrap-up" } });
      expect(screen.getByRole("switch", { name: "Daily focus" })).not.toBeChecked();
      expect(field("End-of-day title")).toHaveValue("Wrap-up");
    });

    it("saves the switch as focus.enabled only", async () => {
      renderPage();
      expect(screen.getByRole("switch", { name: "Daily focus" })).toBeChecked();
      await userEvent.click(screen.getByRole("switch", { name: "Daily focus" }));
      expect(lastSaved().focus).toEqual({ ...settings().focus, enabled: false });
    });

    it("saves the trimmed end-of-day title", async () => {
      renderPage();
      await replace(field("End-of-day title"), "  Wrap-up ");
      await userEvent.keyboard("{Enter}");
      expect(lastSaved().focus).toEqual({ ...settings().focus, endOfDayTitle: "Wrap-up" });
    });

    it("saves a cleared title as empty, so the reply is just the list", async () => {
      renderPage();
      await replace(field("End-of-day title"), "");
      await userEvent.tab();
      expect(lastSaved().focus.endOfDayTitle).toBe("");
    });
  });

  describe("before an absence", () => {
    const rules = () => settings().statusRules;
    const card = (title: string) => within(screen.getByRole("heading", { name: title }).closest("section")!);

    it("shows the saved absence settings", () => {
      renderPage();
      expect(field("Absence titles")).toHaveValue("Out of office");
      expect(screen.getByRole("spinbutton", { name: "Busy blocks from" })).toHaveValue(8);
      expect(field("Time zone label")).toHaveValue("");
    });

    it("saves the comma list of absence titles", async () => {
      renderPage();
      await replace(field("Absence titles"), "Out of office, Vacation ,, Sick");
      await userEvent.keyboard("{Enter}");
      expect(lastSaved().statusRules).toEqual({ ...rules(), absenceTitles: ["Out of office", "Vacation", "Sick"] });
    });

    it("saves the minimum busy hours", async () => {
      renderPage();
      await replace(screen.getByRole("spinbutton", { name: "Busy blocks from" }), "6");
      await userEvent.keyboard("{Enter}");
      expect(lastSaved().statusRules.minBusyHours).toBe(6);
    });

    it("saves the trimmed time zone label", async () => {
      renderPage();
      await replace(field("Time zone label"), "  CEST ");
      await userEvent.keyboard("{Enter}");
      expect(lastSaved().statusRules.tzLabel).toBe("CEST");
    });

    it("shows one card per rule titled by its hours", () => {
      renderPage();
      expect(screen.getByRole("heading", { name: "Absence starts within 8 hours" })).toBeInTheDocument();
      expect(screen.getByRole("heading", { name: "Absence starts within 168 hours" })).toBeInTheDocument();
    });

    it("saves a rule switched off", async () => {
      renderPage();
      await userEvent.click(card("Absence starts within 168 hours").getByRole("switch", { name: "Workflow on" }));
      expect(lastSaved().statusRules.rules).toEqual([rules().rules[0], { ...rules().rules[1], enabled: false }]);
    });

    it("saves the hours of a rule", async () => {
      renderPage();
      await replace(card("Absence starts within 8 hours").getByRole("spinbutton", { name: "Hours before" }), "12");
      await userEvent.keyboard("{Enter}");
      expect(lastSaved().statusRules.rules).toEqual([{ ...rules().rules[0], hours: 12 }, rules().rules[1]]);
    });

    it("saves the Slack status of a rule", async () => {
      renderPage();
      const rule = card("Absence starts within 8 hours");
      await userEvent.click(rule.getByRole("button", { name: "Edit Slack status" }));
      await replace(rule.getByLabelText("Text"), "Out soon");
      await userEvent.keyboard("{Enter}");
      expect(lastSaved().statusRules.rules).toEqual([{ ...rules().rules[0], slack: { ...rules().rules[0].slack, text: "Out soon" } }, rules().rules[1]]);
    });

    it("removes only the rule whose Remove Rule is clicked", async () => {
      renderPage();
      await userEvent.click(card("Absence starts within 8 hours").getByRole("button", { name: "Remove Rule" }));
      expect(lastSaved().statusRules.rules).toEqual([rules().rules[1]]);
    });

    it("appends a 24 hour rule with a default status from Add Rule", async () => {
      renderPage();
      await userEvent.click(screen.getByRole("button", { name: "Add Rule" }));
      expect(lastSaved().statusRules.rules).toEqual([
        ...rules().rules,
        { hours: 24, enabled: true, slack: { keepStatus: false, emoji: ":information_source:", text: "Away {when}", presence: "keep" } },
      ]);
    });
  });
});
