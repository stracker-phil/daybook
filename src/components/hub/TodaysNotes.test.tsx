import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type HubView } from "../../api";
import { resetStore, useStore } from "../../state/store";
import { TodaysNotes } from "./TodaysNotes";

const mocked = vi.mocked(api, true);

async function renderNotes(patch: Partial<HubView> = {}) {
  resetStore();
  render(<TodaysNotes hub={{ ...(await api.hubView()), ...patch }} />);
}

describe("TodaysNotes", () => {
  beforeEach(() => vi.clearAllMocks());

  it.each([
    [true, "Today’s notes"],
    [false, "Notes"],
  ])("is titled by the day: isToday %s gives %s", async (isToday, title) => {
    await renderNotes({ isToday });
    expect(screen.getByRole("heading", { name: title })).toBeInTheDocument();
  });

  it("shows the quick-notes shortcut as a key cap beside the title today", async () => {
    await renderNotes({ isToday: true, notesShortcut: "ctrl+alt+cmd+n" });
    expect(screen.getByRole("heading", { name: /Today’s notes/ })).toHaveTextContent("⌃⌥⌘N");
  });

  it("shows no key cap on another day", async () => {
    await renderNotes({ isToday: false, notesShortcut: "ctrl+alt+cmd+n" });
    expect(screen.queryByText("⌃⌥⌘N")).not.toBeInTheDocument();
  });

  it("lists a row per topic of the note", async () => {
    await renderNotes({ notes: [{ title: "Planning", level: 2 }, { title: "Ideas", level: 2 }] });
    expect(screen.getByRole("button", { name: "Planning" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Ideas" })).toBeInTheDocument();
  });

  it("opens today's clicked topic in the quick-notes editor", async () => {
    await renderNotes({ isToday: true, notes: [{ title: "Planning", level: 2 }] });
    await userEvent.click(screen.getByRole("button", { name: "Planning" }));
    expect(mocked.quickNotesOpen).toHaveBeenCalledWith({ title: "Planning", level: 2 }, false);
    expect(mocked.dailyNoteOpen).not.toHaveBeenCalled();
  });

  it("toasts when today's topic cannot be opened in quick notes", async () => {
    mocked.quickNotesOpen.mockRejectedValueOnce({ kind: "io", message: "No editor", code: null });
    await renderNotes({ isToday: true, notes: [{ title: "Planning", level: 2 }] });
    await userEvent.click(screen.getByRole("button", { name: "Planning" }));
    await vi.waitFor(() => expect(useStore.getState().toasts.map((t) => t.message).join()).toContain("Couldn't open quick notes"));
  });

  it("opens another day's note in Obsidian at the clicked topic", async () => {
    await renderNotes({ day: "2026-10-20", isToday: false, notes: [{ title: "Planning", level: 2 }] });
    await userEvent.click(screen.getByRole("button", { name: "Planning" }));
    expect(mocked.dailyNoteOpen).toHaveBeenCalledWith("2026-10-20", "Planning");
    expect(mocked.quickNotesOpen).not.toHaveBeenCalled();
  });

  it("offers to write a note today when there are no topics", async () => {
    await renderNotes({ isToday: true, notes: [] });
    await userEvent.click(screen.getByRole("button", { name: "Write a note…" }));
    expect(mocked.runCommand).toHaveBeenCalledWith("notes:quick");
  });

  it("offers no note to write on another day", async () => {
    await renderNotes({ isToday: false, notes: [] });
    expect(screen.queryByRole("button", { name: "Write a note…" })).not.toBeInTheDocument();
  });

  it("toasts when quick notes cannot be opened", async () => {
    mocked.runCommand.mockRejectedValueOnce({ kind: "io", message: "No editor", code: null });
    await renderNotes({ isToday: true, notes: [] });
    await userEvent.click(screen.getByRole("button", { name: "Write a note…" }));
    await vi.waitFor(() => expect(useStore.getState().toasts.map((t) => t.message).join()).toContain("No editor"));
  });

  it("opens the whole note from Open in Obsidian", async () => {
    await renderNotes({ day: "2026-10-24" });
    await userEvent.click(screen.getByRole("button", { name: "Open in Obsidian" }));
    expect(mocked.dailyNoteOpen).toHaveBeenCalledWith("2026-10-24", undefined);
  });

  it("says so when another day's note has no topics", async () => {
    await renderNotes({ isToday: false, notes: [] });
    expect(screen.getByText("No notes.")).toBeInTheDocument();
  });

  it("shows no empty text when there are topics", async () => {
    await renderNotes({ isToday: false, notes: [{ title: "Planning", level: 2 }] });
    expect(screen.queryByText("No notes.")).not.toBeInTheDocument();
  });

  it("toasts when the note cannot be opened", async () => {
    mocked.dailyNoteOpen.mockRejectedValueOnce({ kind: "io", message: "No vault", code: null });
    await renderNotes();
    await userEvent.click(screen.getByRole("button", { name: "Open in Obsidian" }));
    await vi.waitFor(() => expect(useStore.getState().toasts.map((t) => t.message).join()).toContain("No vault"));
  });
});
