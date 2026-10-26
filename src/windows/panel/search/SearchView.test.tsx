import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../../test/apiMock")).createApiMock() }));

import { api } from "../../../api";
import { searchResponse, searchResult } from "../../../test/search";
import { SearchView } from "./SearchView";

const mocked = vi.mocked(api, true);

const HINTS = "↵ open · ⌘↵ open preview · ⌘C copy link · esc close";
const DEFAULT_PLACEHOLDER = "Search the whole vault… (@scope to narrow)";

const field = () => screen.getByRole("textbox", { name: "Search" });
const chip = (scope: string) => screen.getByRole("button", { name: `Scope ${scope}. Press Delete to remove it.` });
const queryChip = (scope: string) => screen.queryByRole("button", { name: `Scope ${scope}. Press Delete to remove it.` });
const highlighted = () => screen.getByRole("option", { selected: true });

const PLAN = searchResult("2-Work/Plan.md");
const HOME = searchResult("Home.md");
const LOG = searchResult("3-Work/Log.md");

async function renderView(scope: string | null = null) {
  render(<SearchView scope={scope} />);
  await screen.findByRole("textbox", { name: "Search" });
}

async function showResults(...paths: string[]) {
  mocked.searchQuery.mockResolvedValue(searchResponse(paths.map((path) => searchResult(path))));
  await renderView();
  await screen.findAllByRole("option");
}

describe("SearchView", () => {
  let user: ReturnType<typeof userEvent.setup>;

  beforeEach(() => {
    vi.clearAllMocks();
    user = userEvent.setup();
    mocked.searchQuery.mockResolvedValue(searchResponse([]));
    mocked.searchScopes.mockResolvedValue({ names: ["journal", "work", "all"], problems: [] });
    mocked.searchNote.mockImplementation(async (path: string) => ({ path, name: path, folder: "", markdown: "" }));
    mocked.searchOpen.mockResolvedValue(undefined);
    mocked.searchLink.mockResolvedValue("[[Note]]");
    mocked.searchOpenUrl.mockResolvedValue(undefined);
  });

  describe("scope chips", () => {
    it("asks for the whole vault without a chip", async () => {
      await renderView();
      expect(field()).toHaveAttribute("placeholder", DEFAULT_PLACEHOLDER);
      await user.type(field(), "plan");
      await waitFor(() => expect(mocked.searchQuery).toHaveBeenCalledWith("plan", []));
    });

    it("shows the session's scope as a chip and searches within it", async () => {
      await renderView("work");
      expect(chip("work")).toHaveTextContent("@work");
      expect(field()).toHaveAttribute("placeholder", "Search notes…");
      await user.type(field(), "plan");
      await waitFor(() => expect(mocked.searchQuery).toHaveBeenCalledWith("plan", ["work"]));
    });

    it("turns a typed @name followed by a space into a chip and drops the token", async () => {
      await renderView();
      await user.type(field(), "plan @journal ");
      expect(chip("journal")).toBeInTheDocument();
      expect(field()).toHaveValue("plan ");
      await waitFor(() => expect(mocked.searchQuery).toHaveBeenCalledWith("plan ", ["journal"]));
    });

    it("keeps an unknown @name in the text", async () => {
      await renderView();
      await user.type(field(), "@x ");
      expect(field()).toHaveValue("@x ");
      expect(screen.queryByRole("button", { name: /^Scope/ })).not.toBeInTheDocument();
    });

    it("replaces the default chip with the first chosen scope and appends later ones", async () => {
      await renderView("work");
      await user.type(field(), "@journal ");
      expect(chip("journal")).toBeInTheDocument();
      expect(queryChip("work")).not.toBeInTheDocument();
      await user.type(field(), "@all ");
      expect(chip("journal")).toBeInTheDocument();
      expect(chip("all")).toBeInTheDocument();
    });

    it("removes the last chip with Backspace in an empty field", async () => {
      await renderView();
      await user.type(field(), "@journal @work ");
      expect(chip("work")).toBeInTheDocument();
      await user.keyboard("{Backspace}");
      expect(queryChip("work")).not.toBeInTheDocument();
      expect(chip("journal")).toBeInTheDocument();
      expect(field()).toHaveAttribute("placeholder", "Search notes…");
    });

    it("keeps the chips while Backspace deletes text", async () => {
      await renderView("work");
      await user.type(field(), "ab{Backspace}");
      expect(field()).toHaveValue("a");
      expect(chip("work")).toBeInTheDocument();
    });

    it.each(["Backspace", "Delete"])("removes a focused chip with %s and focuses the field", async (key) => {
      await renderView("work");
      await user.tab({ shift: true });
      expect(chip("work")).toHaveFocus();
      await user.keyboard(`{${key}}`);
      expect(queryChip("work")).not.toBeInTheDocument();
      expect(field()).toHaveFocus();
      expect(field()).toHaveAttribute("placeholder", DEFAULT_PLACEHOLDER);
    });
  });

  describe("scope suggestions", () => {
    it("lists the names matching the typed @token instead of results", async () => {
      mocked.searchQuery.mockResolvedValue(searchResponse([HOME]));
      await renderView();
      await user.type(field(), "@jo");
      const list = screen.getByRole("listbox", { name: "Scopes" });
      expect(list).toHaveTextContent("Scopes");
      expect(screen.getAllByRole("option").map((o) => o.textContent)).toEqual(["journal"]);
      expect(screen.queryByRole("listbox", { name: "Results" })).not.toBeInTheDocument();
    });

    it("offers every name for a bare @, including all, minus the chosen ones", async () => {
      await renderView("work");
      await user.type(field(), "@");
      expect(screen.getAllByRole("option").map((o) => o.textContent)).toEqual(["journal", "all"]);
    });

    it("adds the first suggestion on Tab and drops the token", async () => {
      await renderView();
      await user.type(field(), "plan @jo");
      await user.keyboard("{Tab}");
      expect(chip("journal")).toBeInTheDocument();
      expect(field()).toHaveValue("plan ");
    });

    it("adds the highlighted suggestion on Enter", async () => {
      await renderView();
      await user.type(field(), "@");
      await user.keyboard("{ArrowDown}");
      expect(highlighted()).toHaveTextContent("work");
      await user.keyboard("{Enter}");
      expect(chip("work")).toBeInTheDocument();
      expect(field()).toHaveValue("");
    });

    it("adds a suggestion that is clicked", async () => {
      await renderView();
      await user.type(field(), "@wo");
      await user.click(screen.getByRole("option", { name: "work" }));
      expect(chip("work")).toBeInTheDocument();
      expect(field()).toHaveValue("");
    });
  });

  describe("results", () => {
    it("shows name, folder and snippet with the matched parts marked, and highlights the first", async () => {
      mocked.searchQuery.mockResolvedValue(
        searchResponse([
          searchResult("2-Work/Plan.md", {
            nameParts: [
              { text: "Q3 ", hit: false },
              { text: "Plan", hit: true },
            ],
            lineParts: [
              { text: "the ", hit: false },
              { text: "roadmap", hit: true },
              { text: " for autumn", hit: false },
            ],
          }),
          LOG,
        ]),
      );
      await renderView();
      const first = await screen.findByRole("option", { name: /Q3 Plan/ });
      expect(first).toHaveAttribute("aria-selected", "true");
      expect(first).toHaveTextContent("2-Work");
      expect(first).toHaveTextContent("the roadmap for autumn");
      expect(screen.getByText("Plan", { selector: "mark" })).toBeInTheDocument();
      expect(screen.getByText("roadmap", { selector: "mark" })).toBeInTheDocument();
      expect(screen.getAllByRole("option")[1]).toHaveAttribute("aria-selected", "false");
    });

    it("moves the highlight with the arrows and wraps at both ends", async () => {
      await showResults(PLAN.path, HOME.path, LOG.path);
      await user.keyboard("{ArrowDown}");
      expect(highlighted()).toHaveTextContent("Home");
      await user.keyboard("{ArrowDown}{ArrowDown}");
      expect(highlighted()).toHaveTextContent("Plan");
      await user.keyboard("{ArrowUp}");
      expect(highlighted()).toHaveTextContent("Log");
    });

    it("does not move the highlight when the pointer hovers a row", async () => {
      await showResults(PLAN.path, HOME.path);
      await user.hover(screen.getByRole("option", { name: /Home/ }));
      expect(highlighted()).toHaveTextContent("Plan");
    });

    it("highlights a clicked row and opens the note when the highlighted row is clicked", async () => {
      await showResults(PLAN.path, HOME.path);
      await user.click(screen.getByRole("option", { name: /Home/ }));
      expect(highlighted()).toHaveTextContent("Home");
      expect(mocked.searchOpen).not.toHaveBeenCalled();
      await user.click(screen.getByRole("option", { name: /Home/ }));
      expect(mocked.searchOpen).toHaveBeenCalledWith("Home.md");
    });

    it("opens the highlighted result on Enter", async () => {
      await showResults(PLAN.path, HOME.path);
      await user.keyboard("{ArrowDown}{Enter}");
      expect(mocked.searchOpen).toHaveBeenCalledWith("Home.md");
    });

    it("shows a hint to type before any search and a message when nothing matches", async () => {
      await renderView();
      expect(await screen.findByText("Type to search.")).toBeInTheDocument();
      expect(screen.getByText("Results show up here as you type.")).toBeInTheDocument();
      await user.type(field(), "zzz");
      expect(await screen.findByText("No notes found.")).toBeInTheDocument();
      expect(screen.queryByText("Type to search.")).not.toBeInTheDocument();
    });
  });

  describe("footer", () => {
    it("lists the key hints without back and forward at first", async () => {
      await renderView();
      expect(screen.getByText(HINTS)).toBeInTheDocument();
    });

    it("shows the failure of a search in place of the status", async () => {
      mocked.searchQuery.mockRejectedValue({ kind: "io", message: "Index unreadable", code: null });
      await renderView();
      expect(await screen.findByText("Index unreadable")).toBeInTheDocument();
    });

    it("shows an error from opening a note and clears it when typing", async () => {
      mocked.searchOpen.mockRejectedValue({ kind: "io", message: "Disk full", code: null });
      await showResults(PLAN.path);
      await user.keyboard("{Enter}");
      expect(await screen.findByText("Disk full")).toBeInTheDocument();
      await user.type(field(), "x");
      expect(screen.queryByText("Disk full")).not.toBeInTheDocument();
    });
  });

  describe("indexing", () => {
    afterEach(() => vi.useRealTimers());

    it("shows the progress and asks again every 700 ms until the index is built", async () => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
      mocked.searchQuery
        .mockResolvedValueOnce(searchResponse([], { indexing: true, notes: 12 }))
        .mockResolvedValueOnce(searchResponse([], { indexing: true, notes: 340 }))
        .mockResolvedValue(searchResponse([], { indexing: false, notes: 800 }));
      await renderView();
      expect(await screen.findByText("Indexing… 12 notes so far")).toBeInTheDocument();
      await act(() => vi.advanceTimersByTimeAsync(700));
      expect(await screen.findByText("Indexing… 340 notes so far")).toBeInTheDocument();
      await act(() => vi.advanceTimersByTimeAsync(700));
      await waitFor(() => expect(screen.queryByText(/Indexing…/)).not.toBeInTheDocument());
    });
  });

  describe("previewed note", () => {
    beforeEach(() => {
      mocked.searchNote.mockImplementation(async (path: string) => ({
        path,
        name: path,
        folder: "",
        markdown: path === "Home.md" ? "[Plan](#note=2-Work%2FPlan.md)" : `Body of ${path}`,
      }));
    });

    it("opens the previewed note on ⌘Enter", async () => {
      await showResults(PLAN.path, HOME.path);
      await user.keyboard("{ArrowDown}{Meta>}{Enter}{/Meta}");
      expect(mocked.searchOpen).toHaveBeenCalledWith("Home.md");
    });

    it("opens the linked note on ⌘Enter after following a link", async () => {
      await showResults(HOME.path);
      await user.click(await screen.findByRole("link", { name: "Plan" }));
      await screen.findByText("Body of 2-Work/Plan.md");
      await user.click(field());
      await user.keyboard("{Meta>}{Enter}{/Meta}");
      expect(mocked.searchOpen).toHaveBeenCalledWith("2-Work/Plan.md");
    });

    it("copies the wiki link of the previewed note and confirms it", async () => {
      mocked.searchLink.mockResolvedValue("[[Plan]]");
      await showResults(PLAN.path);
      await user.keyboard("{Meta>}c{/Meta}");
      expect(await screen.findByText("Copied [[Plan]]")).toBeInTheDocument();
      expect(mocked.searchLink).toHaveBeenCalledWith("2-Work/Plan.md");
      expect(await navigator.clipboard.readText()).toBe("[[Plan]]");
    });

    it("leaves ⌘C to the browser while text in the field is selected", async () => {
      await showResults(PLAN.path);
      await user.type(field(), "abc");
      (field() as HTMLInputElement).setSelectionRange(0, 3);
      await user.keyboard("{Meta>}c{/Meta}");
      expect(mocked.searchLink).not.toHaveBeenCalled();
      expect(screen.queryByText(/^Copied/)).not.toBeInTheDocument();
    });

    it("shows an error from copying the link", async () => {
      mocked.searchLink.mockRejectedValue({ kind: "notFound", message: "Note is gone", code: null });
      await showResults(PLAN.path);
      await user.keyboard("{Meta>}c{/Meta}");
      expect(await screen.findByText("Note is gone")).toBeInTheDocument();
    });

    it("goes back and forward through followed links with ⌘[ and ⌘] and shows the matching hints", async () => {
      await showResults(HOME.path);
      await user.click(await screen.findByRole("link", { name: "Plan" }));
      await screen.findByText("Body of 2-Work/Plan.md");
      expect(screen.getByText(HINTS.replace("⌘C copy link", "⌘C copy link · ⌘[ back"))).toBeInTheDocument();

      fireEvent.keyDown(field(), { key: "[", metaKey: true });
      expect(await screen.findByRole("link", { name: "Plan" })).toBeInTheDocument();
      expect(screen.getByText(HINTS.replace("⌘C copy link", "⌘C copy link · ⌘] forward"))).toBeInTheDocument();

      fireEvent.keyDown(field(), { key: "]", metaKey: true });
      expect(await screen.findByText("Body of 2-Work/Plan.md")).toBeInTheDocument();
    });

    it("drops the forward history when a new link is followed after going back", async () => {
      mocked.searchNote.mockImplementation(async (path: string) => ({
        path,
        name: path,
        folder: "",
        markdown: path === "Home.md" ? "[Plan](#note=Plan.md) [Log](#note=Log.md)" : `Body of ${path}`,
      }));
      await showResults(HOME.path);
      await user.click(await screen.findByRole("link", { name: "Plan" }));
      await screen.findByText("Body of Plan.md");
      fireEvent.keyDown(field(), { key: "[", metaKey: true });
      await user.click(await screen.findByRole("link", { name: "Log" }));
      await screen.findByText("Body of Log.md");
      expect(screen.queryByText(/⌘\] forward/)).not.toBeInTheDocument();
      expect(screen.getByText(/⌘\[ back/)).toBeInTheDocument();
    });

    it("starts over when the highlight moves, and shows the result itself on return", async () => {
      await showResults(HOME.path, LOG.path);
      await user.click(await screen.findByRole("link", { name: "Plan" }));
      await screen.findByText("Body of 2-Work/Plan.md");
      await user.click(field());
      await user.keyboard("{ArrowDown}");
      expect(await screen.findByText("Body of 3-Work/Log.md")).toBeInTheDocument();
      expect(screen.queryByText(/⌘\[ back/)).not.toBeInTheDocument();
      await user.keyboard("{ArrowUp}");
      expect(await screen.findByRole("link", { name: "Plan" })).toBeInTheDocument();
    });
  });
});
