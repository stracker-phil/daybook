import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type QuickNoteTopics, type TopicRow } from "../../api";
import { QuickNotesView } from "./QuickNotesView";

const mocked = vi.mocked(api, true);

const existing = (title: string, level = 3): TopicRow => ({ topic: { title, level }, create: false });
const created = (title: string): TopicRow => ({ topic: { title, level: 3 }, create: true });
const result = (rows: TopicRow[], hasTopics = true): QuickNoteTopics => ({ rows, hasTopics, notesHeading: "## Notes" });

const ALL = [existing("Planning"), existing("Retro", 4), existing("Ideas")];

/** The list answers per typed query, as Rust does. */
function answer(byQuery: Record<string, QuickNoteTopics>) {
  mocked.quickNotesList.mockImplementation(async (query) => byQuery[query] ?? result([]));
}

async function renderView(byQuery: Record<string, QuickNoteTopics> = { "": result(ALL) }) {
  answer(byQuery);
  render(<QuickNotesView />);
  await screen.findByRole("textbox", { name: "Topic" });
}

const input = () => screen.getByRole("textbox", { name: "Topic" });
const optionNames = () => screen.queryAllByRole("option").map((o) => o.textContent);
const selected = () => screen.getAllByRole("option").find((o) => o.getAttribute("aria-selected") === "true")?.textContent;

describe("QuickNotesView", () => {
  beforeEach(() => vi.clearAllMocks());

  it("asks for the topics with an empty query on mount and lists them with their heading level", async () => {
    await renderView();
    expect(await screen.findByRole("option", { name: /Planning/ })).toBeInTheDocument();
    expect(mocked.quickNotesList).toHaveBeenCalledWith("");
    expect(optionNames()).toEqual(["###Planning", "####Retro", "###Ideas"]);
  });

  it("names the notes heading in the placeholder", async () => {
    await renderView();
    await screen.findByRole("option", { name: /Planning/ });
    expect(input()).toHaveAttribute("placeholder", "Pick a topic under ## Notes or type a new title…");
  });

  describe("typing", () => {
    it("asks again with the typed query and shows exactly the rows returned", async () => {
      await renderView({ "": result(ALL), Plan: result([existing("Planning"), created("Plan")]) });
      await userEvent.type(input(), "Plan");
      expect(await screen.findByRole("option", { name: "New topic “Plan”" })).toBeInTheDocument();
      expect(mocked.quickNotesList).toHaveBeenLastCalledWith("Plan");
      expect(optionNames()).toEqual(["###Planning", "New topic “Plan”"]);
    });

    it("selects the first row again when new results arrive", async () => {
      await renderView({ "": result(ALL), R: result([existing("Retro", 4), existing("Review")]) });
      await screen.findByRole("option", { name: /Planning/ });
      await userEvent.type(input(), "{ArrowDown}{ArrowDown}R");
      await screen.findByRole("option", { name: /Review/ });
      expect(selected()).toBe("####Retro");
    });
  });

  describe("opening a row", () => {
    it("opens the first topic with Enter without creating it", async () => {
      await renderView();
      await screen.findByRole("option", { name: /Planning/ });
      await userEvent.type(input(), "{Enter}");
      expect(mocked.quickNotesOpen).toHaveBeenCalledWith({ title: "Planning", level: 3 }, false);
    });

    it("opens the topic chosen with the arrow keys", async () => {
      await renderView();
      await screen.findByRole("option", { name: /Planning/ });
      await userEvent.type(input(), "{ArrowDown}{Enter}");
      expect(mocked.quickNotesOpen).toHaveBeenCalledWith({ title: "Retro", level: 4 }, false);
    });

    it("wraps from the first row up to the last and from the last down to the first", async () => {
      await renderView();
      await screen.findByRole("option", { name: /Planning/ });
      await userEvent.type(input(), "{ArrowUp}");
      expect(selected()).toBe("###Ideas");
      await userEvent.type(input(), "{ArrowDown}");
      expect(selected()).toBe("###Planning");
    });

    it("opens a clicked topic", async () => {
      await renderView();
      await userEvent.click(await screen.findByRole("option", { name: /Ideas/ }));
      expect(mocked.quickNotesOpen).toHaveBeenCalledWith({ title: "Ideas", level: 3 }, false);
    });

    it("opens a new-topic row with create set", async () => {
      await renderView({ "": result(ALL), Fresh: result([created("Fresh")]) });
      await userEvent.type(input(), "Fresh");
      await userEvent.click(await screen.findByRole("option", { name: "New topic “Fresh”" }));
      expect(mocked.quickNotesOpen).toHaveBeenCalledWith({ title: "Fresh", level: 3 }, true);
    });

    it("opens a new-topic row with Enter", async () => {
      await renderView({ "": result(ALL), Fresh: result([created("Fresh")]) });
      await userEvent.type(input(), "Fresh");
      await screen.findByRole("option", { name: "New topic “Fresh”" });
      await userEvent.type(input(), "{Enter}");
      expect(mocked.quickNotesOpen).toHaveBeenCalledWith({ title: "Fresh", level: 3 }, true);
    });

    it("opens nothing when Enter is pressed without any row", async () => {
      await renderView({ "": result([], false) });
      await screen.findByText("No topics today yet. Type a title to start one.");
      await userEvent.type(input(), "{Enter}");
      expect(mocked.quickNotesOpen).not.toHaveBeenCalled();
    });
  });

  describe("empty state", () => {
    it("invites to start a topic when today has none", async () => {
      await renderView({ "": result([], false) });
      expect(await screen.findByText("No topics today yet. Type a title to start one.")).toBeInTheDocument();
    });

    it("does not say today has no topics when a query matches none of them", async () => {
      await renderView({ "": result(ALL), zzz: result([], true) });
      await userEvent.type(input(), "zzz");
      await vi.waitFor(() => expect(optionNames()).toEqual([]));
      expect(screen.queryByText("No topics today yet. Type a title to start one.")).not.toBeInTheDocument();
    });
  });

  describe("errors", () => {
    it("shows a failed topic list in the footer", async () => {
      mocked.quickNotesList.mockRejectedValue({ kind: "io", message: "No vault", code: null });
      render(<QuickNotesView />);
      expect(await screen.findByText("No vault")).toBeInTheDocument();
    });

    it("shows a failed open in the footer", async () => {
      mocked.quickNotesOpen.mockRejectedValue({ kind: "io", message: "Note is locked", code: null });
      await renderView();
      await screen.findByRole("option", { name: /Planning/ });
      await userEvent.type(input(), "{Enter}");
      expect(await screen.findByText("Note is locked")).toBeInTheDocument();
    });

    it("shows the key hints in the footer while nothing failed", async () => {
      await renderView();
      expect(screen.getByText("↵ open · esc close")).toBeInTheDocument();
    });
  });
});
