import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type ReminderRule, type Settings } from "../../api";
import { resetStore } from "../../state/store";
import { settings } from "../../test/settings";
import { SettingsWindow } from "./SettingsWindow";

const mocked = vi.mocked(api, true);

function renderPage(patch: Partial<Settings> = {}) {
  resetStore({ settings: settings(patch) });
  render(<SettingsWindow page="reminders" onPage={() => {}} />);
}

const withTodosOn = (patch: Partial<Extract<ReminderRule, { kind: "todos" }>> = {}) => ({
  reminders: settings().reminders.map((rule) => (rule.kind === "todos" ? { ...rule, enabled: true, ...patch } : rule)),
});

const group = (title: string) => within(screen.getByRole("switch", { name: title }).closest(".lead-block") as HTMLElement);
const switchOf = (title: string) => screen.getByRole("switch", { name: title });
const saved = () => mocked.updateSettings.mock.lastCall![0].reminders;

describe("RemindersPage", () => {
  beforeEach(() => vi.clearAllMocks());

  it.each(["Upcoming meeting", "Routine: Morning", "Routine: Evening", "Journal: Dawn", "Journal: Dusk", "Journal: Health", "Open todos"])(
    "has a block with a switch named %s",
    (title) => {
      renderPage();
      expect(switchOf(title)).toBeInTheDocument();
    },
  );

  describe("due text", () => {
    it.each([
      ["Routine: Morning", "While due, 09:45 to 13:00, and not done."],
      ["Routine: Evening", "While due, 18:00 to the end of the day, and not done."],
      ["Journal: Dusk", "While due, 17:30 to the end of the day, and not done."],
      ["Journal: Health", "Never due: set a due time in Settings → Journal."],
    ])("explains %s", (title, text) => {
      renderPage();
      expect(group(title).getByText(text)).toBeInTheDocument();
    });

    it("sends a routine without a due time to Settings → Routines", () => {
      const s = settings();
      renderPage({ routines: s.routines.map((r) => (r.id === "morning" ? { ...r, timing: { ...r.timing, due: null } } : r)) });
      expect(group("Routine: Morning").getByText("Never due: set a due time in Settings → Routines.")).toBeInTheDocument();
    });
  });

  describe("switch", () => {
    it("reflects whether the rule is enabled", () => {
      renderPage();
      expect(switchOf("Upcoming meeting")).toBeChecked();
      expect(switchOf("Open todos")).not.toBeChecked();
    });

    it("hides the fields of a disabled rule", () => {
      renderPage();
      expect(group("Open todos").queryByRole("combobox", { name: "Todos from" })).not.toBeInTheDocument();
      expect(group("Open todos").queryByLabelText("Reminder from")).not.toBeInTheDocument();
    });

    it("saves the rule enabled and shows its fields", async () => {
      renderPage();
      await userEvent.click(switchOf("Open todos"));
      expect(saved().at(-1)).toEqual({ kind: "todos", enabled: true, span: "today", from: "16:00", to: "" });
      expect(await group("Open todos").findByLabelText("Reminder from")).toHaveValue("16:00");
    });

    it("saves the rule disabled and hides its fields", async () => {
      renderPage();
      await userEvent.click(switchOf("Upcoming meeting"));
      expect(saved()[0]).toEqual({ kind: "meeting", enabled: false, minutesBefore: 60 });
      await vi.waitFor(() => expect(group("Upcoming meeting").queryByRole("spinbutton", { name: "Minutes before" })).not.toBeInTheDocument());
    });
  });

  describe("fields", () => {
    it("saves the minutes before a meeting on blur", async () => {
      renderPage();
      const field = group("Upcoming meeting").getByRole("spinbutton", { name: "Minutes before" });
      expect(field).toHaveValue(60);
      await userEvent.clear(field);
      await userEvent.type(field, "15");
      await userEvent.tab();
      expect(saved()[0]).toEqual({ kind: "meeting", enabled: true, minutesBefore: 15 });
    });

    it("offers the todo spans and saves the chosen one", async () => {
      renderPage(withTodosOn());
      const select = group("Open todos").getByRole("combobox", { name: "Todos from" });
      expect(within(select).getAllByRole("option").map((o) => o.textContent)).toEqual(["from today", "from this week", "from this month", "from any day"]);
      await userEvent.selectOptions(select, "week");
      expect(saved().at(-1)).toEqual(expect.objectContaining({ kind: "todos", span: "week" }));
    });

    it("saves a changed start time and keeps the end", async () => {
      renderPage(withTodosOn({ to: "20:00" }));
      const from = group("Open todos").getByLabelText("Reminder from");
      await userEvent.click(from);
      fireEvent.change(from, { target: { value: "15:00" } });
      await userEvent.tab();
      expect(saved().at(-1)).toEqual(expect.objectContaining({ from: "15:00", to: "20:00" }));
    });

    it("saves an empty end so the reminder runs to the end of the day", async () => {
      renderPage(withTodosOn({ to: "20:00" }));
      const until = group("Open todos").getByLabelText("Reminder until");
      expect(until).toHaveValue("20:00");
      await userEvent.click(until);
      fireEvent.change(until, { target: { value: "" } });
      await userEvent.tab();
      expect(saved().at(-1)).toEqual(expect.objectContaining({ from: "16:00", to: "" }));
    });
  });

  describe("list actions", () => {
    it("disables Move Up on the first rule and Move Down on the last", () => {
      renderPage();
      expect(group("Upcoming meeting").getByRole("button", { name: "Move Up" })).toBeDisabled();
      expect(group("Open todos").getByRole("button", { name: "Move Down" })).toBeDisabled();
    });

    it("saves the order after Move Down", async () => {
      renderPage();
      await userEvent.click(group("Upcoming meeting").getByRole("button", { name: "Move Down" }));
      expect(saved().slice(0, 2).map((r) => r.kind)).toEqual(["routine", "meeting"]);
    });

    it("saves the order after Move Up", async () => {
      renderPage();
      await userEvent.click(group("Routine: Evening").getByRole("button", { name: "Move Up" }));
      const routines = saved().flatMap((r) => (r.kind === "routine" ? [r.routine] : []));
      expect(routines).toEqual(["evening", "morning"]);
    });

    it.each(["Upcoming meeting", "Open todos"])("removes %s", async (title) => {
      renderPage();
      await userEvent.click(group(title).getByRole("button", { name: "Remove Reminder" }));
      expect(saved()).toHaveLength(6);
      expect(saved().some((r) => r.kind === (title === "Open todos" ? "todos" : "meeting"))).toBe(false);
    });

    it.each(["Routine: Morning", "Journal: Dawn"])("offers no Remove Reminder on %s", (title) => {
      renderPage();
      expect(group(title).queryByRole("button", { name: "Remove Reminder" })).not.toBeInTheDocument();
    });

    it("appends an Upcoming meeting rule one hour ahead", async () => {
      renderPage();
      await userEvent.click(screen.getByRole("button", { name: "Add Upcoming meeting" }));
      expect(saved()).toHaveLength(8);
      expect(saved().at(-1)).toEqual({ kind: "meeting", enabled: true, minutesBefore: 60 });
    });

    it("appends an Open todos rule for today from 16:00 with an open end", async () => {
      renderPage();
      await userEvent.click(screen.getByRole("button", { name: "Add Open todos" }));
      expect(saved()).toHaveLength(8);
      expect(saved().at(-1)).toEqual({ kind: "todos", enabled: true, span: "today", from: "16:00", to: "" });
    });
  });
});
