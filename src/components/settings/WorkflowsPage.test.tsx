import { render, screen } from "@testing-library/react";
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
});
