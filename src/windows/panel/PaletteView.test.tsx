import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type PaletteEntry, type PaletteResults } from "../../api";
import { PaletteView } from "./PaletteView";

const mocked = vi.mocked(api, true);

const entry = (label: string, command: string, patch: Partial<PaletteEntry> = {}): PaletteEntry => ({
  kind: "command",
  label,
  detail: "",
  shortcut: null,
  command,
  ...patch,
});

const PREFIXES: PaletteResults["prefixes"] = [
  { char: ">", kind: "command", label: "Commands" },
  { char: "@", kind: "link", label: "Links" },
];

const ALL: PaletteResults = {
  prefix: null,
  prefixes: PREFIXES,
  groups: [
    {
      kind: "command",
      heading: "Commands",
      entries: [entry("Open Settings", "app:settings", { shortcut: "cmd+," }), entry("Open Hub", "hub:open")],
    },
    { kind: "link", heading: "Links", entries: [entry("Team wiki", "link:wiki", { kind: "link", detail: "Work" })] },
  ],
};

const COMMANDS_ONLY: PaletteResults = { ...ALL, prefix: PREFIXES[0], groups: [ALL.groups[0]] };

const search = () => screen.getByRole("textbox", { name: "Search" });
const selected = () => screen.getAllByRole("option").find((o) => o.getAttribute("aria-selected") === "true")?.textContent;
const option = (name: RegExp) => screen.findByRole("option", { name });

async function renderPalette() {
  render(<PaletteView />);
  await option(/Open Settings/);
}

describe("PaletteView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.paletteQuery.mockImplementation(async (query) => (query.startsWith(">") ? COMMANDS_ONLY : ALL));
  });

  describe("results", () => {
    it("lists each group under its heading with the entries' labels and details", async () => {
      await renderPalette();
      expect(screen.getByRole("listbox", { name: "Results" })).toBeInTheDocument();
      expect(screen.getByText("Links", { selector: "li" })).toBeInTheDocument();
      expect(screen.getByText("Commands", { selector: "li" })).toBeInTheDocument();
      expect(screen.getAllByRole("option").map((o) => o.textContent)).toEqual(["Open Settings⌘,", "Open Hub", "Team wikiWork"]);
    });

    it("focuses the search field and marks the first entry active", async () => {
      await renderPalette();
      expect(search()).toHaveFocus();
      expect(selected()).toBe("Open Settings⌘,");
    });

    it("shows an entry's shortcut as glyphs", async () => {
      mocked.paletteQuery.mockResolvedValue({
        ...ALL,
        groups: [{ kind: "command", heading: "Commands", entries: [entry("Open Settings", "app:settings", { shortcut: "ctrl+alt+cmd+j" })] }],
      });
      render(<PaletteView />);
      expect(await screen.findByText("⌃⌥⌘J")).toBeInTheDocument();
    });

    it("says so when nothing matches", async () => {
      mocked.paletteQuery.mockResolvedValue({ prefix: null, prefixes: PREFIXES, groups: [] });
      render(<PaletteView />);
      expect(await screen.findByText("No results.")).toBeInTheDocument();
    });

    it("shows the new results after typing", async () => {
      mocked.paletteQuery.mockImplementation(async (query) => (query === "wiki" ? { ...ALL, groups: [ALL.groups[1]] } : ALL));
      await renderPalette();
      await userEvent.type(search(), "wiki");
      await vi.waitFor(() => expect(screen.getAllByRole("option").map((o) => o.textContent)).toEqual(["Team wikiWork"]));
    });
  });

  describe("keyboard", () => {
    it("moves the active entry down with ArrowDown and wraps to the first", async () => {
      await renderPalette();
      await userEvent.keyboard("{ArrowDown}");
      expect(selected()).toBe("Open Hub");
      await userEvent.keyboard("{ArrowDown}{ArrowDown}");
      expect(selected()).toBe("Open Settings⌘,");
    });

    it("moves up with ArrowUp and wraps to the last entry", async () => {
      await renderPalette();
      await userEvent.keyboard("{ArrowUp}");
      expect(selected()).toBe("Team wikiWork");
    });

    it("runs the second entry's command on ArrowDown then Enter", async () => {
      await renderPalette();
      await userEvent.keyboard("{ArrowDown}{Enter}");
      expect(mocked.paletteRun).toHaveBeenCalledWith("hub:open");
    });

    it("runs the first entry on Enter", async () => {
      await renderPalette();
      await userEvent.keyboard("{Enter}");
      expect(mocked.paletteRun).toHaveBeenCalledWith("app:settings");
    });

    it("runs nothing on Enter without results", async () => {
      mocked.paletteQuery.mockResolvedValue({ prefix: null, prefixes: PREFIXES, groups: [] });
      render(<PaletteView />);
      await screen.findByText("No results.");
      await userEvent.keyboard("{Enter}");
      expect(mocked.paletteRun).not.toHaveBeenCalled();
    });
  });

  describe("running an entry", () => {
    it("runs the clicked row's command", async () => {
      await renderPalette();
      await userEvent.click(await option(/Team wiki/));
      expect(mocked.paletteRun).toHaveBeenCalledWith("link:wiki");
    });

    it("shows the message of a rejected run in the footer in place of the prefix buttons", async () => {
      mocked.paletteRun.mockRejectedValueOnce({ kind: "io", message: "Could not open the wiki", code: null });
      await renderPalette();
      expect(screen.getByRole("button", { name: /Commands/ })).toBeInTheDocument();
      await userEvent.click(await option(/Team wiki/));
      expect(await screen.findByText("Could not open the wiki")).toBeInTheDocument();
      expect(screen.queryByRole("button", { name: /Commands/ })).not.toBeInTheDocument();
      expect(screen.queryByRole("button", { name: /Links/ })).not.toBeInTheDocument();
    });
  });

  describe("prefix buttons", () => {
    it("offers one button per prefix", async () => {
      await renderPalette();
      expect(screen.getByRole("button", { name: /Commands/ })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: /Links/ })).toBeInTheDocument();
    });

    it("puts the prefix char in the field and narrows the results", async () => {
      await renderPalette();
      await userEvent.click(screen.getByRole("button", { name: /Commands/ }));
      expect(search()).toHaveValue(">");
      await vi.waitFor(() => expect(screen.getAllByRole("option")).toHaveLength(2));
      expect(search()).toHaveAttribute("placeholder", "Search commands…");
    });

    it("clears the field when the active prefix is clicked again", async () => {
      await renderPalette();
      await userEvent.click(screen.getByRole("button", { name: /Commands/ }));
      await vi.waitFor(() => expect(search()).toHaveAttribute("placeholder", "Search commands…"));
      await userEvent.click(screen.getByRole("button", { name: /Commands/ }));
      expect(search()).toHaveValue("");
    });

    it("puts the char of another prefix in the field", async () => {
      await renderPalette();
      await userEvent.click(screen.getByRole("button", { name: /Links/ }));
      expect(search()).toHaveValue("@");
    });
  });
});
