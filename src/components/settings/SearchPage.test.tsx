import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type SearchScope, type Settings } from "../../api";
import { settings } from "../../test/settings";
import { SearchPage } from "./SearchPage";

const mocked = vi.mocked(api, true);
const set = vi.fn<(patch: Partial<Settings>) => void>();

function renderPage(patch: Partial<Settings> = {}) {
  render(<SearchPage s={settings(patch)} set={set} />);
}

const savedSearch = () => set.mock.lastCall![0].search!;
const savedScopes = (): SearchScope[] => savedSearch().scopes;
const names = (scopes: SearchScope[]) => scopes.map((scope) => scope.name);
const nameInput = (i: number) => screen.getAllByRole("textbox", { name: "Scope name" })[i];
const entriesInput = (i: number) => screen.getAllByRole("textbox", { name: "Folders and scopes" })[i];

describe("SearchPage", () => {
  beforeEach(() => vi.clearAllMocks());

  describe("scopes", () => {
    it("shows one row per scope with its name and entries joined by commas", () => {
      renderPage();
      expect(screen.getAllByRole("textbox", { name: "Scope name" }).map((input) => (input as HTMLInputElement).value)).toEqual(["journal", "work", "private", "all"]);
      expect(entriesInput(1)).toHaveValue("2-Work/, journal");
    });

    it("saves a renamed scope as typed, keeping its entries", async () => {
      renderPage();
      await userEvent.clear(nameInput(1));
      await userEvent.type(nameInput(1), "Clients{Enter}");
      expect(savedScopes()[1]).toEqual({ name: "Clients", entries: ["2-Work/", "journal"] });
      expect(names(savedScopes())).toEqual(["journal", "Clients", "private", "all"]);
    });

    it("ignores an emptied name", async () => {
      renderPage();
      await userEvent.clear(nameInput(0));
      await userEvent.tab();
      expect(set).not.toHaveBeenCalled();
      expect(nameInput(0)).toHaveValue("journal");
    });

    it("saves entries split on commas, trimmed, without empty ones", async () => {
      renderPage();
      await userEvent.clear(entriesInput(0));
      await userEvent.type(entriesInput(0), " 1-Journal/ ,, 3-Personal/ , {Enter}");
      expect(savedScopes()[0]).toEqual({ name: "journal", entries: ["1-Journal/", "3-Personal/"] });
    });

    it("saves an emptied entries field as no entries", async () => {
      renderPage();
      await userEvent.clear(entriesInput(0));
      await userEvent.tab();
      expect(savedScopes()[0].entries).toEqual([]);
    });
  });

  describe("list actions", () => {
    it("appends a scope named after the next number without entries", async () => {
      renderPage();
      await userEvent.click(screen.getByRole("button", { name: "Add Scope" }));
      expect(savedScopes()).toHaveLength(5);
      expect(savedScopes()[4]).toEqual({ name: "scope5", entries: [] });
    });

    it("skips a scope name already in use when naming the new scope", async () => {
      renderPage({ search: { ...settings().search, scopes: [{ name: "a", entries: [] }, { name: "scope3", entries: [] }] } });
      await userEvent.click(screen.getByRole("button", { name: "Add Scope" }));
      expect(names(savedScopes())).toEqual(["a", "scope3", "scope4"]);
    });

    it("removes a scope", async () => {
      renderPage();
      await userEvent.click(screen.getAllByRole("button", { name: "Remove Scope" })[1]);
      expect(names(savedScopes())).toEqual(["journal", "private", "all"]);
    });

    it("moves a scope up", async () => {
      renderPage();
      await userEvent.click(screen.getAllByRole("button", { name: "Move Up" })[2]);
      expect(names(savedScopes())).toEqual(["journal", "private", "work", "all"]);
    });

    it("moves a scope down", async () => {
      renderPage();
      await userEvent.click(screen.getAllByRole("button", { name: "Move Down" })[0]);
      expect(names(savedScopes())).toEqual(["work", "journal", "private", "all"]);
    });

    it("disables Move Up on the first scope and Move Down on the last", () => {
      renderPage();
      expect(screen.getAllByRole("button", { name: "Move Up" })[0]).toBeDisabled();
      expect(screen.getAllByRole("button", { name: "Move Down" }).at(-1)).toBeDisabled();
    });
  });

  describe("problems", () => {
    it("shows the problems Rust reports, joined by one space", async () => {
      mocked.searchScopes.mockResolvedValue({ names: ["all"], problems: ["Scope work: unknown folder.", "Scope loop includes itself."] });
      renderPage();
      expect(await screen.findByRole("alert")).toHaveTextContent("Scope work: unknown folder. Scope loop includes itself.");
    });

    it("shows no alert without problems", async () => {
      renderPage();
      await vi.waitFor(() => expect(mocked.searchScopes).toHaveBeenCalled());
      expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    });
  });

  describe("excluded folders", () => {
    it("shows the excluded folders joined by commas", () => {
      renderPage();
      expect(screen.getByRole("textbox", { name: "Excluded folders" })).toHaveValue("_/, .obsidian/, .trash/");
    });

    it("saves them as a list, keeping the scopes", async () => {
      renderPage();
      const input = screen.getByRole("textbox", { name: "Excluded folders" });
      await userEvent.clear(input);
      await userEvent.type(input, "Archive/, , .trash/{Enter}");
      expect(savedSearch().exclude).toEqual(["Archive/", ".trash/"]);
      expect(names(savedScopes())).toEqual(["journal", "work", "private", "all"]);
    });
  });
});
