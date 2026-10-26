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

  it("lists a row per topic of the note", async () => {
    await renderNotes({ notes: [{ title: "Planning", level: 2 }, { title: "Ideas", level: 2 }] });
    expect(screen.getByRole("button", { name: "Planning" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Ideas" })).toBeInTheDocument();
  });

  it("opens the note of the shown day at the clicked topic", async () => {
    await renderNotes({ day: "2026-10-20", isToday: false, notes: [{ title: "Planning", level: 2 }] });
    await userEvent.click(screen.getByRole("button", { name: "Planning" }));
    expect(mocked.dailyNoteOpen).toHaveBeenCalledWith("2026-10-20", "Planning");
  });

  it("opens the whole note from Open in Obsidian", async () => {
    await renderNotes({ day: "2026-10-24" });
    await userEvent.click(screen.getByRole("button", { name: "Open in Obsidian" }));
    expect(mocked.dailyNoteOpen).toHaveBeenCalledWith("2026-10-24", undefined);
  });

  it.each([
    [true, "No notes yet today."],
    [false, "No notes."],
  ])("says so when the note has no topics (isToday %s)", async (isToday, text) => {
    await renderNotes({ isToday, notes: [] });
    expect(screen.getByText(text)).toBeInTheDocument();
  });

  it("shows no empty text when there are topics", async () => {
    await renderNotes({ notes: [{ title: "Planning", level: 2 }] });
    expect(screen.queryByText("No notes yet today.")).not.toBeInTheDocument();
  });

  it("toasts when the note cannot be opened", async () => {
    mocked.dailyNoteOpen.mockRejectedValueOnce({ kind: "io", message: "No vault", code: null });
    await renderNotes();
    await userEvent.click(screen.getByRole("button", { name: "Open in Obsidian" }));
    await vi.waitFor(() => expect(useStore.getState().toasts.map((t) => t.message).join()).toContain("No vault"));
  });
});
