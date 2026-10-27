import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type AnswerKind, type JournalAnswer, type JournalGroupView, type JournalInfo, type JournalQuestionView } from "../../api";
import { hintKeys } from "../../test/panelHints";
import { settings } from "../../test/settings";
import { JournalView } from "./JournalView";

const mocked = vi.mocked(api, true);

function question(name: string, kind: AnswerKind, answer: Partial<JournalAnswer> = {}, hint = ""): JournalQuestionView {
  return { question: name, hint, kind, answer: { items: [], tasks: [], text: "", ...answer }, answered: false };
}

function group(title: string, questions: JournalQuestionView[], patch: Partial<JournalGroupView> = {}): JournalGroupView {
  return { title, questions, answered: 0, total: questions.length, note: "", ...patch };
}

function info(groups: JournalGroupView[], noteExists = true): JournalInfo {
  return { groups, noteExists };
}

const MORNING = group("Morning", [question("Grateful for", "list"), question("Plan", "task")]);
const EVENING = group("Evening", [question("Highlight", "text")]);

async function renderView(groups: JournalGroupView[], asked: string | null = null, noteExists = true) {
  mocked.journalInfo.mockResolvedValue(info(groups, noteExists));
  const view = render(<JournalView group={asked} />);
  await screen.findByText("Journal");
  return view;
}

const pressed = (name: string) => screen.getByRole("button", { name }).getAttribute("aria-pressed") === "true";
const cmdDigit = (digit: number) => fireEvent.keyDown(window, { key: String(digit), code: `Digit${digit}`, metaKey: true });
const saved = () => mocked.journalSave.mock.calls;

async function reread(next: JournalInfo) {
  mocked.journalInfo.mockResolvedValue(next);
  const handler = mocked.onNoteChanged.mock.calls[0][0];
  await act(async () => handler({ day: null }));
}

describe("JournalView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.journalSave.mockResolvedValue(undefined);
  });

  describe("choosing the group", () => {
    const done = group("Done", [question("A", "list")], { answered: 1, total: 1 });
    const open = group("Open", [question("B", "list")], { answered: 1, total: 2 });

    it("shows the asked group", async () => {
      await renderView([open, done], "Done");
      expect(pressed("Done")).toBe(true);
      expect(screen.getByRole("textbox", { name: "A 1" })).toBeInTheDocument();
    });

    it("shows the first group with open questions when none is asked for", async () => {
      await renderView([done, open]);
      expect(pressed("Open")).toBe(true);
    });

    it("shows the first group when all questions are answered", async () => {
      const alsoDone = group("Also done", [question("C", "list")], { answered: 1, total: 1 });
      await renderView([done, alsoDone]);
      expect(pressed("Done")).toBe(true);
    });

    it("shows the first group with open questions when the asked group does not exist", async () => {
      await renderView([done, open], "Missing");
      expect(pressed("Open")).toBe(true);
    });

    it("lists the groups as a segmented control only when there is more than one", async () => {
      const view = await renderView([MORNING, EVENING], "Morning");
      expect(screen.getByRole("group", { name: "Group" })).toBeInTheDocument();
      view.unmount();
      await renderView([MORNING], "Morning");
      expect(screen.queryByRole("group", { name: "Group" })).not.toBeInTheDocument();
    });

    it("switches to the group when its button is clicked", async () => {
      await renderView([MORNING, EVENING], "Morning");
      await userEvent.click(screen.getByRole("button", { name: "Evening" }));
      expect(screen.getByRole("textbox", { name: "Highlight" })).toBeInTheDocument();
      expect(screen.queryByRole("textbox", { name: "Grateful for 1" })).not.toBeInTheDocument();
    });

    it("switches to the group numbered by the pressed ⌘ digit", async () => {
      await renderView([MORNING, EVENING], "Morning");
      cmdDigit(2);
      await waitFor(() => expect(pressed("Evening")).toBe(true));
      cmdDigit(1);
      await waitFor(() => expect(pressed("Morning")).toBe(true));
    });

    it("ignores a digit without ⌘ and a digit beyond the last group", async () => {
      await renderView([MORNING, EVENING], "Morning");
      fireEvent.keyDown(window, { key: "2", code: "Digit2" });
      cmdDigit(3);
      expect(pressed("Morning")).toBe(true);
    });
  });

  describe("fields", () => {
    it("numbers list rows and adds one empty trailing row", async () => {
      await renderView([group("Morning", [question("Grateful for", "list", { items: ["Sunshine", "Kaffee"] })])]);
      expect(screen.getByRole("textbox", { name: "Grateful for 1" })).toHaveValue("Sunshine");
      expect(screen.getByRole("textbox", { name: "Grateful for 2" })).toHaveValue("Kaffee");
      expect(screen.getByRole("textbox", { name: "Grateful for 3" })).toHaveValue("");
      expect(screen.queryByRole("textbox", { name: "Grateful for 4" })).not.toBeInTheDocument();
      expect(screen.getByText("3.")).toBeInTheDocument();
    });

    it("shows the hint of a question", async () => {
      await renderView([group("Morning", [question("Grateful for", "list", {}, "Three small things")])]);
      expect(screen.getByText("Three small things")).toBeInTheDocument();
    });

    it("shows a task with a checkbox that is disabled while the row is empty", async () => {
      await renderView([group("Morning", [question("Plan", "task", { tasks: [{ text: "Write", done: true }] })])]);
      const [first, trailing] = screen.getAllByRole("checkbox", { name: "Done" });
      expect(first).toBeChecked();
      expect(first).toBeEnabled();
      expect(trailing).toBeDisabled();
      await userEvent.type(screen.getByRole("textbox", { name: "Plan 2" }), "Read");
      expect(screen.getAllByRole("checkbox", { name: "Done" })[1]).toBeEnabled();
    });

    it("shows a text question as a textarea labelled by the question", async () => {
      await renderView([group("Evening", [question("Highlight", "text", { text: "A walk" })])]);
      expect(screen.getByRole("textbox", { name: "Highlight" })).toHaveValue("A walk");
    });

    it("shows a group without questions as one free text field labelled by the group title", async () => {
      await renderView([group("Evening", [], { note: "Quiet day" })]);
      expect(screen.getByRole("textbox", { name: "Evening" })).toHaveValue("Quiet day");
    });

    it("focuses the first field", async () => {
      await renderView([MORNING]);
      await waitFor(() => expect(screen.getByRole("textbox", { name: "Grateful for 1" })).toHaveFocus());
    });
  });

  describe("keys in a row", () => {
    const list = group("Morning", [question("Q", "list", { items: ["a", "", "c"] })]);

    it("moves the caret to the next row on ↵", async () => {
      await renderView([list]);
      await userEvent.click(screen.getByRole("textbox", { name: "Q 1" }));
      await userEvent.keyboard("{Enter}");
      await waitFor(() => expect(screen.getByRole("textbox", { name: "Q 2" })).toHaveFocus());
    });

    it("moves between rows on ↓ and ↑", async () => {
      await renderView([list]);
      await userEvent.click(screen.getByRole("textbox", { name: "Q 1" }));
      await userEvent.keyboard("{ArrowDown}");
      expect(screen.getByRole("textbox", { name: "Q 2" })).toHaveFocus();
      await userEvent.keyboard("{ArrowUp}");
      expect(screen.getByRole("textbox", { name: "Q 1" })).toHaveFocus();
    });

    it("removes an empty middle row on ⌫ and moves up", async () => {
      await renderView([list]);
      await userEvent.click(screen.getByRole("textbox", { name: "Q 2" }));
      await userEvent.keyboard("{Backspace}");
      expect(screen.getByRole("textbox", { name: "Q 2" })).toHaveValue("c");
      expect(screen.getByRole("textbox", { name: "Q 3" })).toHaveValue("");
      expect(screen.queryByRole("textbox", { name: "Q 4" })).not.toBeInTheDocument();
      expect(screen.getByRole("textbox", { name: "Q 1" })).toHaveFocus();
    });

    it("keeps the trailing empty row on ⌫", async () => {
      await renderView([list]);
      await userEvent.click(screen.getByRole("textbox", { name: "Q 4" }));
      await userEvent.keyboard("{Backspace}");
      expect(screen.getByRole("textbox", { name: "Q 4" })).toBeInTheDocument();
    });
  });

  describe("saving", () => {
    beforeEach(() => vi.useFakeTimers({ shouldAdvanceTime: true }));
    afterEach(() => vi.useRealTimers());

    const typer = () => userEvent.setup({ advanceTimers: vi.advanceTimersByTime });

    it("saves the answers 800 ms after the last change, without the trailing empty row", async () => {
      await renderView([group("Morning", [question("Grateful for", "list"), question("Highlight", "text", { text: "Walk" })], { note: "n" })]);
      await typer().type(screen.getByRole("textbox", { name: "Grateful for 1" }), "Sunshine");
      await vi.advanceTimersByTimeAsync(500);
      expect(saved()).toHaveLength(0);
      await vi.advanceTimersByTimeAsync(400);
      expect(mocked.journalSave).toHaveBeenCalledWith("Morning", {
        answers: {
          "Grateful for": { items: ["Sunshine"], tasks: [], text: "" },
          Highlight: { items: [], tasks: [], text: "Walk" },
        },
        note: "n",
      });
    });

    it("saves a ticked task with its text", async () => {
      await renderView([group("Morning", [question("Plan", "task")])]);
      const user = typer();
      await user.type(screen.getByRole("textbox", { name: "Plan 1" }), "Write");
      await user.click(screen.getAllByRole("checkbox", { name: "Done" })[0]);
      await vi.advanceTimersByTimeAsync(900);
      expect(mocked.journalSave).toHaveBeenLastCalledWith("Morning", {
        answers: { Plan: { items: [], tasks: [{ text: "Write", done: true }], text: "" } },
        note: "",
      });
    });

    it("saves free text of a group without questions as its note", async () => {
      await renderView([group("Evening", [])]);
      await typer().type(screen.getByRole("textbox", { name: "Evening" }), "Calm");
      await vi.advanceTimersByTimeAsync(900);
      expect(mocked.journalSave).toHaveBeenCalledWith("Evening", { answers: {}, note: "Calm" });
    });

    it("saves pending changes at once when switching to another group", async () => {
      await renderView([MORNING, EVENING], "Morning");
      await typer().type(screen.getByRole("textbox", { name: "Grateful for 1" }), "Sunshine");
      await userEvent.click(screen.getByRole("button", { name: "Evening" }));
      await waitFor(() => expect(mocked.journalSave).toHaveBeenCalledWith("Morning", expect.objectContaining({ note: "" })));
      expect(saved()[0][1].answers["Grateful for"].items).toEqual(["Sunshine"]);
    });

    it("saves pending changes at once when the view goes away", async () => {
      const view = await renderView([MORNING]);
      await typer().type(screen.getByRole("textbox", { name: "Grateful for 1" }), "Sunshine");
      view.unmount();
      await waitFor(() => expect(saved()).toHaveLength(1));
      expect(saved()[0][1].answers["Grateful for"].items).toEqual(["Sunshine"]);
    });

    it("shows the message of a failed save in the footer", async () => {
      mocked.journalSave.mockRejectedValue({ kind: "io", message: "Disk full", code: null });
      await renderView([MORNING]);
      await typer().type(screen.getByRole("textbox", { name: "Grateful for 1" }), "Sunshine");
      await vi.advanceTimersByTimeAsync(900);
      expect(await screen.findByText("Disk full")).toBeInTheDocument();
      expect(hintKeys("Close")).toEqual(["esc"]);
    });
  });

  describe("re-reading the note", () => {
    const morning = (items: string[]) => group("Morning", [question("Q", "list", { items })]);

    it("re-reads when the note changes and replaces the draft with answers edited in Obsidian", async () => {
      await renderView([morning(["Sunshine"])]);
      await reread(info([morning(["Regen"])]));
      expect(screen.getByRole("textbox", { name: "Q 1" })).toHaveValue("Regen");
    });

    it("re-reads when Settings change", async () => {
      await renderView([morning(["Sunshine"])]);
      mocked.journalInfo.mockResolvedValue(info([morning(["Sunshine"]), group("Evening", [])]));
      const handler = mocked.onSettingsChanged.mock.calls[0][0];
      await act(async () => handler(settings()));
      expect(screen.getByRole("button", { name: "Evening" })).toBeInTheDocument();
    });

    it("keeps the field as typed when the note only echoes our own save", async () => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
      try {
        await renderView([morning([])]);
        await userEvent.setup({ advanceTimers: vi.advanceTimersByTime }).type(screen.getByRole("textbox", { name: "Q 1" }), "Sunshine ");
        await vi.advanceTimersByTimeAsync(900);
        expect(saved()).toHaveLength(1);
        await reread(info([morning(["Sunshine"])]));
        expect(screen.getByRole("textbox", { name: "Q 1" })).toHaveValue("Sunshine ");
      } finally {
        vi.useRealTimers();
      }
    });

    it("keeps a draft that is not saved yet when the note changes", async () => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
      try {
        await renderView([morning([])]);
        await userEvent.setup({ advanceTimers: vi.advanceTimersByTime }).type(screen.getByRole("textbox", { name: "Q 1" }), "Mine");
        await reread(info([morning(["Theirs"])]));
        expect(screen.getByRole("textbox", { name: "Q 1" })).toHaveValue("Mine");
      } finally {
        vi.useRealTimers();
      }
    });
  });

  describe("footer", () => {
    it("lists the group shortcut range, esc and no waiting hint", async () => {
      await renderView([MORNING, EVENING]);
      expect(hintKeys("Group")).toEqual(["⌘", "1–2"]);
      expect(hintKeys("Close")).toEqual(["esc"]);
      expect(screen.queryByText("Saved once today's note exists")).not.toBeInTheDocument();
    });

    it("omits the group shortcut with a single group", async () => {
      await renderView([MORNING]);
      expect(hintKeys("Group")).toBeNull();
      expect(hintKeys("Close")).toEqual(["esc"]);
    });

    it("says answers are saved once today's note exists when it does not", async () => {
      await renderView([MORNING], null, false);
      expect(screen.getByText("Saved once today's note exists")).toBeInTheDocument();
    });
  });

  describe("empty and failing", () => {
    it("points to Settings when there are no journal questions", async () => {
      await renderView([]);
      expect(screen.getByText("No journal questions yet. Add them in Settings → Journal.")).toBeInTheDocument();
    });

    it("shows the message when the journal cannot be read", async () => {
      mocked.journalInfo.mockRejectedValue({ kind: "io", message: "Note is unreadable", code: null });
      render(<JournalView group={null} />);
      expect(await screen.findByText("Note is unreadable")).toBeInTheDocument();
    });
  });
});
