import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { installCommands } from "../commands";
import { resetStore, useStore, type Store } from "../state/store";
import { settings } from "../test/settings";
import { CommandPalette } from "./CommandPalette";
import { Dialogs } from "./Dialogs";

async function openPalette(query = "", patch: Partial<Store> = {}) {
  resetStore({ settings: settings(), paletteOpen: true, ...patch });
  installCommands();
  render(
    <>
      <CommandPalette />
      <Dialogs />
    </>,
  );
  const input = screen.getByLabelText("Search");
  if (query) await userEvent.type(input, query);
}

const options = () => screen.queryAllByRole("option").map((o) => o.textContent ?? "");
const option = (name: RegExp) => screen.queryByRole("option", { name });

describe("CommandPalette", () => {
  beforeEach(() => vi.clearAllMocks());

  describe("with nothing typed", () => {
    it("lists only the commands, not the settings pages", async () => {
      await openPalette();
      expect(screen.getByText("Commands")).toBeInTheDocument();
      expect(options()).toEqual(expect.arrayContaining([expect.stringMatching(/^Toggle sidebar/), expect.stringMatching(/^Settings…/)]));
      expect(options().some((t) => t.startsWith("Settings: "))).toBe(false);
    });

    it("leaves out the palette's own command", async () => {
      await openPalette();
      expect(option(/^Command palette/)).not.toBeInTheDocument();
    });

    it("shows the key cap of a bound command", async () => {
      await openPalette();
      expect(option(/^Settings…/)).toHaveTextContent("⌘,");
    });

    it("hints at the commands-only prefix in the footer", async () => {
      await openPalette();
      expect(screen.getByText("> commands only")).toBeInTheDocument();
    });

    it("shows the keys to open and close in the footer", async () => {
      await openPalette();
      expect(screen.getByText("↵ open · esc close")).toBeInTheDocument();
    });

    it("selects the first row", async () => {
      await openPalette();
      expect(screen.getAllByRole("option")[0]).toHaveAttribute("aria-selected", "true");
    });
  });

  describe("search", () => {
    it("narrows the settings pages and commands to the typed text", async () => {
      await openPalette("appear");
      expect(options()).toEqual([expect.stringMatching(/^Settings: Appearance/)]);
    });

    it("lists a command by a word inside its title", async () => {
      await openPalette("text");
      expect(options()).toEqual([expect.stringMatching(/^Larger text/), expect.stringMatching(/^Smaller text/), expect.stringMatching(/^Default text size/)]);
    });

    it("shows only commands behind the > prefix", async () => {
      await openPalette(">settings");
      expect(options()).toEqual([expect.stringMatching(/^Settings…/)]);
      expect(screen.queryByText("Settings: General")).not.toBeInTheDocument();
    });

    it("opens seeded with the prefix", async () => {
      await openPalette("", { paletteSeed: ">" });
      expect(screen.getByLabelText("Search")).toHaveValue(">");
      expect(options().some((t) => t.startsWith("Settings: "))).toBe(false);
      expect(option(/^Toggle sidebar/)).toBeInTheDocument();
    });

    it("drops the prefix hint once something is typed", async () => {
      await openPalette("a");
      expect(screen.queryByText("> commands only")).not.toBeInTheDocument();
      expect(screen.getByText("↵ open · esc close")).toBeInTheDocument();
    });

    it("says so when nothing matches", async () => {
      await openPalette("qqq");
      expect(screen.getByText("No matches.")).toBeInTheDocument();
      expect(screen.queryAllByRole("option")).toEqual([]);
    });
  });

  describe("commands that depend on the state", () => {
    it("offers Launch at login once the settings are loaded", async () => {
      await openPalette("launch");
      expect(options()).toEqual([expect.stringMatching(/^Launch at login/)]);
    });

    it("hides Launch at login while the settings are not loaded", async () => {
      await openPalette("launch", { settings: null });
      expect(screen.getByText("No matches.")).toBeInTheDocument();
    });
  });

  describe("choosing", () => {
    it("moves the selection with the arrow keys", async () => {
      await openPalette(">text");
      await userEvent.keyboard("{ArrowDown}");
      expect(screen.getByRole("option", { name: /^Smaller text/ })).toHaveAttribute("aria-selected", "true");
      await userEvent.keyboard("{ArrowUp}");
      expect(screen.getByRole("option", { name: /^Larger text/ })).toHaveAttribute("aria-selected", "true");
    });

    it("keeps the selection on the last row when moving past it", async () => {
      await openPalette(">text");
      await userEvent.keyboard("{ArrowDown}{ArrowDown}{ArrowDown}{ArrowDown}");
      expect(screen.getByRole("option", { name: /^Default text size/ })).toHaveAttribute("aria-selected", "true");
    });

    it("runs the selected command on Enter and closes the palette", async () => {
      await openPalette(">toggle sidebar");
      await userEvent.keyboard("{Enter}");
      expect(useStore.getState().uiState.sidebarHidden).toBe(true);
      expect(screen.queryByRole("dialog", { name: "Command palette" })).not.toBeInTheDocument();
    });

    it("runs a clicked command", async () => {
      await openPalette(">toggle sidebar");
      await userEvent.click(screen.getByRole("option", { name: /^Toggle sidebar/ }));
      expect(useStore.getState().uiState.sidebarHidden).toBe(true);
    });

    it("opens the chosen settings page and closes the palette", async () => {
      await openPalette("appear");
      await userEvent.click(screen.getByRole("option", { name: /^Settings: Appearance/ }));
      const dialog = await screen.findByRole("dialog");
      expect(within(dialog).getByRole("heading", { name: "Appearance" })).toBeInTheDocument();
      expect(screen.queryByLabelText("Search")).not.toBeInTheDocument();
    });

    it("does nothing on Enter when nothing matches", async () => {
      await openPalette("qqq");
      await userEvent.keyboard("{Enter}");
      expect(screen.getByRole("dialog", { name: "Command palette" })).toBeInTheDocument();
    });

    it("closes with Escape without running anything", async () => {
      await openPalette();
      await userEvent.keyboard("{Escape}");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      expect(useStore.getState().uiState.sidebarHidden).toBe(false);
    });
  });
});
