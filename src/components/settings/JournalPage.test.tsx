import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type Settings } from "../../api";
import { resetStore } from "../../state/store";
import { settings } from "../../test/settings";
import { SettingsWindow } from "./SettingsWindow";

const mocked = vi.mocked(api, true);

function renderPage(patch: Partial<Settings> = {}) {
  resetStore({ settings: settings(patch) });
  render(<SettingsWindow page="journal" onPage={() => {}} />);
}

const card = (title: string) => within(screen.getByRole("heading", { name: title }).closest("section")!);
const morgen = () => card("Dawn");
const gesundheit = () => card("Health");

const saved = () => mocked.updateSettings.mock.lastCall![0].journal;
const savedGroup = (title: string) => saved().find((g) => g.title === title)!;
const groupButton = (within_: ReturnType<typeof card>, name: string) => within_.getAllByRole("button", { name }).at(-1)!;

describe("JournalPage", () => {
  beforeEach(() => vi.clearAllMocks());

  describe("cards", () => {
    it("shows one card per group in order", () => {
      renderPage();
      expect(screen.getAllByRole("heading", { level: 3 }).map((h) => h.textContent)).toEqual(["Dawn", "Dusk", "Health"]);
    });

    it("says a group without questions is a free-text entry", () => {
      renderPage();
      expect(gesundheit().getByText("No questions: a free-text entry.")).toBeInTheDocument();
      expect(morgen().queryByText("No questions: a free-text entry.")).not.toBeInTheDocument();
    });

    it("fills a card with the group's values", () => {
      renderPage();
      expect(morgen().getByRole("textbox", { name: "Name" })).toHaveValue("Dawn");
      expect(morgen().getByRole("textbox", { name: "Callout type" })).toHaveValue("NOTE");
      expect(morgen().getByRole("switch", { name: "Folded" })).not.toBeChecked();
      expect(gesundheit().getByRole("textbox", { name: "Callout type" })).toHaveValue("HEART");
      expect(gesundheit().getByRole("switch", { name: "Folded" })).toBeChecked();
    });
  });

  describe("group fields", () => {
    it("saves a renamed group on blur, trimmed", async () => {
      renderPage();
      await userEvent.clear(morgen().getByRole("textbox", { name: "Name" }));
      await userEvent.type(morgen().getByRole("textbox", { name: "Name" }), " Start ");
      await userEvent.tab();
      expect(saved().map((g) => g.title)).toEqual(["Start", "Dusk", "Health"]);
    });

    it("saves the name on Enter", async () => {
      renderPage();
      await userEvent.type(morgen().getByRole("textbox", { name: "Name" }), "x{Enter}");
      expect(saved()[0].title).toBe("Dawnx");
    });

    it("keeps the name when it is emptied", async () => {
      renderPage();
      await userEvent.clear(morgen().getByRole("textbox", { name: "Name" }));
      await userEvent.tab();
      expect(mocked.updateSettings).not.toHaveBeenCalled();
    });

    it("saves the callout type in upper case", async () => {
      renderPage();
      await userEvent.clear(morgen().getByRole("textbox", { name: "Callout type" }));
      await userEvent.type(morgen().getByRole("textbox", { name: "Callout type" }), "tip{Enter}");
      expect(savedGroup("Dawn").callout).toBe("TIP");
    });

    it("keeps the callout type when it is emptied", async () => {
      renderPage();
      await userEvent.clear(morgen().getByRole("textbox", { name: "Callout type" }));
      await userEvent.tab();
      expect(mocked.updateSettings).not.toHaveBeenCalled();
    });

    it("saves the Folded switch", async () => {
      renderPage();
      await userEvent.click(morgen().getByRole("switch", { name: "Folded" }));
      expect(savedGroup("Dawn").folded).toBe(true);
      expect(savedGroup("Health").folded).toBe(true);
    });
  });

  describe("list actions", () => {
    it("disables Move Up on the first group and Move Down on the last", () => {
      renderPage();
      expect(groupButton(morgen(), "Move Up")).toBeDisabled();
      expect(groupButton(gesundheit(), "Move Down")).toBeDisabled();
    });

    it("saves the order after Move Down", async () => {
      renderPage();
      await userEvent.click(groupButton(morgen(), "Move Down"));
      expect(saved().map((g) => g.title)).toEqual(["Dusk", "Dawn", "Health"]);
    });

    it("saves the order after Move Up", async () => {
      renderPage();
      await userEvent.click(gesundheit().getByRole("button", { name: "Move Up" }));
      expect(saved().map((g) => g.title)).toEqual(["Dawn", "Health", "Dusk"]);
    });

    it("saves the list without the removed group", async () => {
      renderPage();
      await userEvent.click(gesundheit().getByRole("button", { name: "Remove Group" }));
      expect(saved().map((g) => g.title)).toEqual(["Dawn", "Dusk"]);
    });

    it("appends a New group without questions or due time", async () => {
      renderPage();
      await userEvent.click(screen.getByRole("button", { name: "Add Group" }));
      expect(saved()[3]).toEqual({ title: "New group", callout: "NOTE", folded: false, questions: [], timing: { due: null, openWhenDue: false, openOn: [] } });
      expect(await screen.findByRole("heading", { name: "New group" })).toBeInTheDocument();
    });
  });

  describe("questions", () => {
    it("fills a question's fields", () => {
      renderPage();
      expect(morgen().getAllByRole("textbox", { name: "Question" }).map((i) => (i as HTMLInputElement).value)).toEqual([
        "What am I grateful for?",
        "My current goal",
        "The \"One Thing\"",
      ]);
      expect(morgen().getAllByRole("combobox", { name: "Answer" }).map((s) => (s as HTMLSelectElement).value)).toEqual(["list", "text", "task"]);
      expect(morgen().getAllByRole("textbox", { name: "Hint" })[1]).toHaveValue("Where do I want to be in a year?");
    });

    it("gives each question line its answer select and Move Up, Move Down and Remove Question icons", () => {
      renderPage();
      expect(morgen().getAllByRole("textbox", { name: "Question" })).toHaveLength(3);
      expect(morgen().getAllByRole("combobox", { name: "Answer" })).toHaveLength(3);
      expect(morgen().getAllByRole("button", { name: "Remove Question" })).toHaveLength(3);
      expect(morgen().getAllByRole("button", { name: "Move Up" })).toHaveLength(4);
    });

    it("saves an edited question text", async () => {
      renderPage();
      const input = morgen().getAllByRole("textbox", { name: "Question" })[1];
      await userEvent.clear(input);
      await userEvent.type(input, "Ziel{Enter}");
      expect(savedGroup("Dawn").questions.map((q) => q.question)).toEqual(["What am I grateful for?", "Ziel", "The \"One Thing\""]);
    });

    it("keeps the question when its text is emptied", async () => {
      renderPage();
      await userEvent.clear(morgen().getAllByRole("textbox", { name: "Question" })[0]);
      await userEvent.tab();
      expect(mocked.updateSettings).not.toHaveBeenCalled();
    });

    it("saves an emptied hint", async () => {
      renderPage();
      await userEvent.clear(morgen().getAllByRole("textbox", { name: "Hint" })[0]);
      await userEvent.tab();
      expect(savedGroup("Dawn").questions[0].hint).toBe("");
    });

    it.each([
      ["List (1, 2, 3 …)", "list"],
      ["Task (optional)", "task"],
    ])("saves the answer %s as kind %s", async (option, kind) => {
      renderPage();
      await userEvent.selectOptions(morgen().getAllByRole("combobox", { name: "Answer" })[1], option);
      expect(savedGroup("Dawn").questions[1].kind).toBe(kind);
    });

    it("saves the answer Text as kind text", async () => {
      renderPage();
      await userEvent.selectOptions(morgen().getAllByRole("combobox", { name: "Answer" })[0], "Text");
      expect(savedGroup("Dawn").questions[0].kind).toBe("text");
    });

    it("appends a New question as a text question", async () => {
      renderPage();
      await userEvent.click(gesundheit().getByRole("button", { name: "Add Question" }));
      expect(savedGroup("Health").questions).toEqual([{ question: "New question", hint: "", kind: "text" }]);
    });

    it("removes a question", async () => {
      renderPage();
      await userEvent.click(morgen().getAllByRole("button", { name: "Remove Question" })[0]);
      expect(savedGroup("Dawn").questions.map((q) => q.question)).toEqual(["My current goal", "The \"One Thing\""]);
    });

    it("moves a question down", async () => {
      renderPage();
      await userEvent.click(morgen().getAllByRole("button", { name: "Move Down" })[0]);
      expect(savedGroup("Dawn").questions.map((q) => q.question)).toEqual(["My current goal", "What am I grateful for?", "The \"One Thing\""]);
    });
  });

  describe("timing", () => {
    const eventBoxes: [string, string][] = [
      ["On the first Working of the day", "start"],
      ["When switching contexts", "context"],
      ["When a call starts", "meeting-start"],
      ["When a call ends", "meeting-end"],
      ["On “Done for today”", "off"],
    ];

    it("shows the due times of a group with a due time", () => {
      renderPage();
      expect(morgen().getByRole("switch", { name: "Due" })).toBeChecked();
      expect(morgen().getByLabelText("Due from")).toHaveValue("10:00");
      expect(morgen().getByLabelText("Due until")).toHaveValue("13:00");
    });

    it("hides the times of a group without a due time", () => {
      renderPage();
      expect(gesundheit().getByRole("switch", { name: "Due" })).not.toBeChecked();
      expect(gesundheit().queryByLabelText("Due from")).not.toBeInTheDocument();
    });

    it("switches Due on with 09:00 and no end", async () => {
      renderPage();
      await userEvent.click(gesundheit().getByRole("switch", { name: "Due" }));
      expect(savedGroup("Health").timing.due).toEqual({ from: "09:00", to: "" });
    });

    it("clears the due time and opening when Due is switched off", async () => {
      renderPage();
      await userEvent.click(morgen().getByRole("switch", { name: "Due" }));
      expect(savedGroup("Dawn").timing).toEqual({ due: null, openWhenDue: false, openOn: [] });
    });

    it("brings back the last times when Due is switched off and on again", async () => {
      renderPage();
      await userEvent.click(morgen().getByRole("switch", { name: "Due" }));
      await waitFor(() => expect(morgen().getByRole("switch", { name: "Due" })).not.toBeChecked());
      await userEvent.click(morgen().getByRole("switch", { name: "Due" }));
      expect(savedGroup("Dawn").timing.due).toEqual({ from: "10:00", to: "13:00" });
    });

    it("saves an edited due time", async () => {
      renderPage();
      const from = morgen().getByLabelText("Due from");
      await userEvent.click(from);
      fireEvent.change(from, { target: { value: "11:00" } });
      await userEvent.tab();
      expect(savedGroup("Dawn").timing.due).toEqual({ from: "11:00", to: "13:00" });
    });

    it("disables opening when due without a due time", () => {
      renderPage();
      expect(gesundheit().getByRole("checkbox", { name: "When it's due (once a day)" })).toBeDisabled();
      expect(morgen().getByRole("checkbox", { name: "When it's due (once a day)" })).toBeEnabled();
    });

    it("saves opening when due", async () => {
      renderPage();
      await userEvent.click(morgen().getByRole("checkbox", { name: "When it's due (once a day)" }));
      expect(savedGroup("Dawn").timing.openWhenDue).toBe(false);
    });

    it.each(eventBoxes)("adds %s to the opening events and removes it again", async (label, event) => {
      renderPage();
      await userEvent.click(gesundheit().getByRole("checkbox", { name: label }));
      expect(savedGroup("Health").timing.openOn).toEqual([event]);
      await waitFor(() => expect(gesundheit().getByRole("checkbox", { name: label })).toBeChecked());
      await userEvent.click(gesundheit().getByRole("checkbox", { name: label }));
      expect(savedGroup("Health").timing.openOn).toEqual([]);
    });
  });
});
