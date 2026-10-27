import { EditorView } from "@codemirror/view";
import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type QuickNote } from "../../api";
import { installCommands } from "../../commands";
import { QuickNotesApp } from "./QuickNotesApp";

const mocked = vi.mocked(api, true);

const PLANNING: QuickNote = { title: "Planning", level: 3, body: "Hello" };
const TOPIC = { title: "Planning", level: 3 };

const editor = () => screen.getByRole("textbox", { name: "Note" });
const titleInput = () => screen.getByRole("textbox", { name: "Topic title" });
const view = () => EditorView.findFromDOM(editor())!;

async function renderEditor(note: QuickNote | null = PLANNING) {
  mocked.quickNoteGet.mockResolvedValue(note);
  render(<QuickNotesApp />);
  if (note) await screen.findByRole("textbox", { name: "Note" });
  else await screen.findByText(/Pick a topic/);
}

/** Types into the editor the way the user would: at the end, with the caret after the new text. */
function typeBody(text: string) {
  act(() => {
    const end = view().state.doc.length;
    view().dispatch({ changes: { from: end, insert: text }, selection: { anchor: end + text.length } });
  });
}

function placeCaret(anchor: number, head = anchor) {
  act(() => view().dispatch({ selection: { anchor, head } }));
}

async function advance(ms: number) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

const SAVE_DELAY = 800;

// jsdom has no layout; CodeMirror measures text with Range rects.
const emptyRects = () => Object.assign([], { item: () => null }) as unknown as DOMRectList;
const zeroRect = () => new DOMRect();
Range.prototype.getClientRects ??= emptyRects;
Range.prototype.getBoundingClientRect ??= zeroRect;

/** Calls the handler the page registered with `listener`, as Rust's event would. */
async function fire<A extends unknown[]>(listener: { mock: { calls: [(...args: A) => void][] } }, ...args: A) {
  const handler = listener.mock.calls.at(-1)![0];
  await act(async () => {
    handler(...args);
    await vi.advanceTimersByTimeAsync(0);
  });
}

describe("QuickNotesApp", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers({ shouldAdvanceTime: true });
  });

  afterEach(() => vi.useRealTimers());

  describe("loading", () => {
    it("points to the picker while no topic is chosen", async () => {
      await renderEditor(null);
      expect(screen.getByText(/Pick a topic with Quick Notes/)).toBeInTheDocument();
      expect(screen.queryByRole("textbox", { name: "Note" })).not.toBeInTheDocument();
      expect(titleInput()).toBeDisabled();
    });

    it("shows the topic's title, heading level and text", async () => {
      await renderEditor({ title: "Retro", level: 4, body: "What went well" });
      expect(titleInput()).toHaveValue("Retro");
      expect(screen.getByText("####")).toBeInTheDocument();
      expect(editor()).toHaveTextContent("What went well");
      expect(screen.getByText("Saved to today’s note")).toBeInTheDocument();
    });

    it("loads the topic Rust picks next, saving the unsaved text of the current one first", async () => {
      await renderEditor();
      typeBody(" world");
      mocked.quickNoteGet.mockResolvedValue({ title: "Ideas", level: 3, body: "Fresh" });
      await fire(mocked.onQuickNoteTarget);
      expect(mocked.quickNoteSave).toHaveBeenCalledWith(TOPIC, "Planning", "Hello world", "Hello");
      expect(titleInput()).toHaveValue("Ideas");
      expect(editor()).toHaveTextContent("Fresh");
    });
  });

  describe("autosave", () => {
    it("sends the edited title and the exact typed text with the loaded text as base", async () => {
      await renderEditor();
      await userEvent.setup({ advanceTimers: vi.advanceTimersByTime }).type(titleInput(), " 2");
      typeBody(" world\n\n- [ ] a **b**");
      expect(screen.getByText("Saving…")).toBeInTheDocument();
      await advance(SAVE_DELAY);
      expect(mocked.quickNoteSave).toHaveBeenCalledWith(TOPIC, "Planning 2", "Hello world\n\n- [ ] a **b**", "Hello");
      expect(screen.getByText("Saved to today’s note")).toBeInTheDocument();
    });

    it("waits for the typing to pause before saving", async () => {
      await renderEditor();
      typeBody(" a");
      await advance(SAVE_DELAY - 100);
      typeBody("b");
      await advance(SAVE_DELAY - 100);
      expect(mocked.quickNoteSave).not.toHaveBeenCalled();
      await advance(100);
      expect(mocked.quickNoteSave).toHaveBeenCalledWith(TOPIC, "Planning", "Hello ab", "Hello");
    });

    it("builds the next save on the renamed topic and the text saved before", async () => {
      await renderEditor();
      await userEvent.setup({ advanceTimers: vi.advanceTimersByTime }).type(titleInput(), "!");
      await advance(SAVE_DELAY);
      typeBody(" more");
      await advance(SAVE_DELAY);
      expect(mocked.quickNoteSave).toHaveBeenLastCalledWith({ title: "Planning!", level: 3 }, "Planning!", "Hello more", "Hello");
    });

    it("saves the pending text when the window loses focus", async () => {
      await renderEditor();
      typeBody(" now");
      fireEvent.blur(window);
      await advance(0);
      expect(mocked.quickNoteSave).toHaveBeenCalledWith(TOPIC, "Planning", "Hello now", "Hello");
    });

    it("shows a failed save and keeps the text", async () => {
      mocked.quickNoteSave.mockRejectedValue({ kind: "io", message: "Disk full", code: null });
      await renderEditor();
      typeBody(" more");
      await advance(SAVE_DELAY);
      expect(screen.getByText("Not saved: Disk full")).toBeInTheDocument();
      expect(editor()).toHaveTextContent("Hello more");
    });

    it("moves from the title to the text with Enter", async () => {
      await renderEditor();
      await userEvent.setup({ advanceTimers: vi.advanceTimersByTime }).type(titleInput(), "{Enter}");
      expect(editor()).toHaveFocus();
      expect(titleInput()).toHaveValue("Planning");
    });
  });

  describe("the note changing meanwhile", () => {
    it("shows the note's new text when nothing is unsaved", async () => {
      await renderEditor();
      mocked.quickNoteGet.mockResolvedValue({ ...PLANNING, body: "Edited in Obsidian" });
      await fire(mocked.onNoteChanged, { day: "2026-10-24" });
      expect(editor()).toHaveTextContent("Edited in Obsidian");
    });

    it("keeps unsaved text when the note changes", async () => {
      await renderEditor();
      typeBody(" typed");
      mocked.quickNoteGet.mockResolvedValue({ ...PLANNING, body: "Edited in Obsidian" });
      await fire(mocked.onNoteChanged, { day: "2026-10-24" });
      expect(editor()).toHaveTextContent("Hello typed");
      expect(editor()).not.toHaveTextContent("Edited in Obsidian");
    });

    describe("when the save is refused", () => {
      const other: QuickNote = { ...PLANNING, body: "Edited in Obsidian" };

      async function refuseSave() {
        mocked.quickNoteSave.mockResolvedValue({ kind: "changed", note: other });
        await renderEditor();
        typeBody(" typed");
        await advance(SAVE_DELAY);
      }

      it("says the text isn't saved and keeps what was typed", async () => {
        await refuseSave();
        expect(screen.getByText("Changed in the note meanwhile: your text isn’t saved.")).toBeInTheDocument();
        expect(editor()).toHaveTextContent("Hello typed");
      });

      it("takes the note's text on Load the note, after copying the typed text", async () => {
        await refuseSave();
        const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
        await user.click(screen.getByRole("button", { name: "Load the note" }));
        expect(await navigator.clipboard.readText()).toBe("Hello typed");
        expect(editor()).toHaveTextContent("Edited in Obsidian");
        expect(screen.getByText("Saved to today’s note")).toBeInTheDocument();
        expect(screen.queryByRole("button", { name: "Load the note" })).not.toBeInTheDocument();
      });

      it("does not replace the typed text when the note changes again", async () => {
        await refuseSave();
        mocked.quickNoteGet.mockResolvedValue({ ...PLANNING, body: "Edited again" });
        await fire(mocked.onNoteChanged, { day: "2026-10-24" });
        expect(editor()).toHaveTextContent("Hello typed");
      });
    });
  });

  describe("Escape", () => {
    it("saves the pending text, then closes the window", async () => {
      await renderEditor();
      typeBody(" last");
      await act(async () => {
        await userEvent.setup({ advanceTimers: vi.advanceTimersByTime }).keyboard("{Escape}");
      });
      await vi.waitFor(() => expect(mocked.quickNotesClose).toHaveBeenCalled());
      expect(mocked.quickNoteSave).toHaveBeenCalledWith(TOPIC, "Planning", "Hello last", "Hello");
      expect(mocked.quickNoteSave.mock.invocationCallOrder[0]).toBeLessThan(mocked.quickNotesClose.mock.invocationCallOrder[0]);
    });

    it("closes without saving when nothing changed", async () => {
      await renderEditor();
      await act(async () => {
        await userEvent.setup({ advanceTimers: vi.advanceTimersByTime }).keyboard("{Escape}");
      });
      await vi.waitFor(() => expect(mocked.quickNotesClose).toHaveBeenCalled());
      expect(mocked.quickNoteSave).not.toHaveBeenCalled();
    });
  });

  describe("presenting", () => {
    const present = (presenting: boolean) => fire(mocked.onQuickNotePresentation, { presenting });

    it("shows only the editable text, without the title and the save status", async () => {
      await renderEditor();
      await present(true);
      expect(screen.queryByRole("textbox", { name: "Topic title" })).not.toBeInTheDocument();
      expect(screen.queryByText("Saved to today’s note")).not.toBeInTheDocument();
      expect(editor()).toHaveTextContent("Hello");
    });

    it("starts presenting when Rust already presents on load", async () => {
      mocked.quickNotesPresentation.mockResolvedValue({ presenting: true });
      await renderEditor();
      await vi.waitFor(() => expect(screen.queryByRole("textbox", { name: "Topic title" })).not.toBeInTheDocument());
      expect(editor()).toHaveTextContent("Hello");
    });

    it("saves exactly the typed text", async () => {
      await renderEditor();
      await present(true);
      typeBody(" world");
      await advance(SAVE_DELAY);
      expect(mocked.quickNoteSave).toHaveBeenCalledWith(TOPIC, "Planning", "Hello world", "Hello");
    });

    it("brings the title back when the presentation ends", async () => {
      await renderEditor();
      await present(true);
      await present(false);
      expect(titleInput()).toHaveValue("Planning");
      expect(screen.getByText("Saved to today’s note")).toBeInTheDocument();
    });

    it("shows a failed save", async () => {
      mocked.quickNoteSave.mockRejectedValue({ kind: "io", message: "Disk full", code: null });
      await renderEditor();
      await present(true);
      typeBody(" more");
      await advance(SAVE_DELAY);
      expect(screen.getByText("Not saved: Disk full")).toBeInTheDocument();
      expect(screen.queryByRole("textbox", { name: "Topic title" })).not.toBeInTheDocument();
    });

    it("shows the notice and Load the note when the note changed meanwhile", async () => {
      mocked.quickNoteSave.mockResolvedValue({ kind: "changed", note: { ...PLANNING, body: "Edited in Obsidian" } });
      await renderEditor();
      await present(true);
      typeBody(" typed");
      await advance(SAVE_DELAY);
      expect(screen.getByText("Changed in the note meanwhile: your text isn’t saved.")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Load the note" })).toBeInTheDocument();
    });

    it("ends the presentation on Escape instead of closing the window", async () => {
      await renderEditor();
      await present(true);
      await act(async () => {
        await userEvent.setup({ advanceTimers: vi.advanceTimersByTime }).keyboard("{Escape}");
      });
      expect(mocked.runCommand).toHaveBeenCalledWith("notes:endPresentation");
      expect(mocked.quickNotesClose).not.toHaveBeenCalled();
    });

    it("starts the presentation on ⌘⌥P", async () => {
      installCommands("quicknotes");
      await renderEditor();
      fireEvent.keyDown(editor(), { key: "π", code: "KeyP", metaKey: true, altKey: true });
      expect(mocked.runCommand).toHaveBeenCalledWith("notes:present");
    });
  });

  describe("Markdown shortcuts", () => {
    it("continues a list with a new bullet on Enter", async () => {
      await renderEditor({ ...PLANNING, body: "- one" });
      placeCaret(5);
      fireEvent.keyDown(editor(), { key: "Enter", keyCode: 13 });
      await advance(SAVE_DELAY);
      expect(mocked.quickNoteSave).toHaveBeenCalledWith(TOPIC, "Planning", "- one\n- ", "- one");
    });

    it("turns the line into a todo on ⌘L", async () => {
      await renderEditor({ ...PLANNING, body: "buy milk" });
      placeCaret(3);
      fireEvent.keyDown(editor(), { key: "l", keyCode: 76, metaKey: true });
      await advance(SAVE_DELAY);
      expect(mocked.quickNoteSave).toHaveBeenCalledWith(TOPIC, "Planning", "- [ ] buy milk", "buy milk");
    });

    it("bolds the selection on ⌘B", async () => {
      await renderEditor({ ...PLANNING, body: "say hi" });
      placeCaret(4, 6);
      fireEvent.keyDown(editor(), { key: "b", keyCode: 66, metaKey: true });
      await advance(SAVE_DELAY);
      expect(mocked.quickNoteSave).toHaveBeenCalledWith(TOPIC, "Planning", "say **hi**", "say hi");
    });
  });
});
