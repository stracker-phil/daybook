import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type ShortcutOverview } from "../../api";
import { installCommands } from "../../commands";
import { resetStore } from "../../state/store";
import { settings } from "../../test/settings";
import { SettingsWindow } from "./SettingsWindow";

const mocked = vi.mocked(api, true);

const OVERVIEW: ShortcutOverview = {
  registered: true,
  global: [
    { action: "journal", command: "journal:open", title: "Open journal", help: "Answer today's journal." },
    { action: "search", command: "search:open", title: "Search vault", help: "Find notes." },
    { action: "hub", command: "hub:open", title: "Open Daybook", help: "Show the window." },
  ],
  menu: [
    { command: "palette:open", title: "Command palette", shortcut: "cmd+k" },
    { command: "window:close", title: "Close window", shortcut: "cmd+w" },
  ],
};

async function renderPage(overview: Partial<ShortcutOverview> = {}, hotkeys: Record<string, string> = {}, keybindings = {}) {
  mocked.shortcutOverview.mockResolvedValue({ ...OVERVIEW, ...overview });
  const base = settings();
  resetStore({
    settings: { ...base, hotkeys: { ...base.hotkeys, journal: "ctrl+alt+cmd+n", search: "ctrl+alt+cmd+f", hub: "", ...hotkeys } },
    keybindings,
  });
  installCommands();
  render(<SettingsWindow page="shortcuts" onPage={() => {}} />);
  await screen.findByText("Global shortcuts");
}

const field = (title: string) => screen.getByRole("button", { name: new RegExp(`shortcut for ${title}$`) });
const savedHotkeys = () => mocked.updateSettings.mock.lastCall![0].hotkeys;
const rowOf = (title: string) => within(screen.getByText(title).closest<HTMLElement>(".settings-row")!);

describe("ShortcutsPage", () => {
  beforeEach(() => vi.clearAllMocks());

  describe("global shortcuts", () => {
    it("lists each one with its Global marker, help and shortcut", async () => {
      await renderPage();
      expect(screen.getAllByText("Global")).toHaveLength(3);
      expect(screen.getByText("Find notes.")).toBeInTheDocument();
      expect(field("Open journal")).toHaveTextContent("⌃⌥⌘N");
      expect(field("Search vault")).toHaveTextContent("⌃⌥⌘F");
    });

    it("shows a cleared shortcut as unbound", async () => {
      await renderPage();
      expect(field("Open Daybook")).toHaveTextContent("unbound");
    });

    it("saves the recorded keys in Settings notation and keeps the other shortcuts", async () => {
      await renderPage();
      await userEvent.click(field("Open journal"));
      await userEvent.keyboard("{Control>}{Alt>}{Meta>}j{/Meta}{/Alt}{/Control}");
      expect(savedHotkeys()).toEqual(expect.objectContaining({ journal: "ctrl+alt+cmd+j", search: "ctrl+alt+cmd+f", hub: "" }));
      await vi.waitFor(() => expect(field("Open journal")).toHaveTextContent("⌃⌥⌘J"));
    });

    it("records a shortcut for one that was unbound", async () => {
      await renderPage();
      await userEvent.click(field("Open Daybook"));
      await userEvent.keyboard("{Meta>}{Shift>}h{/Shift}{/Meta}");
      expect(savedHotkeys().hub).toBe("shift+cmd+h");
    });

    it("clears the shortcut with Backspace by saving an empty string", async () => {
      await renderPage();
      await userEvent.click(field("Search vault"));
      await userEvent.keyboard("{Backspace}");
      expect(savedHotkeys().search).toBe("");
      await vi.waitFor(() => expect(field("Search vault")).toHaveTextContent("unbound"));
    });

    it("keeps the shortcut when Escape cancels the recording", async () => {
      await renderPage();
      await userEvent.click(field("Search vault"));
      await userEvent.keyboard("{Escape}");
      expect(mocked.updateSettings).not.toHaveBeenCalled();
      expect(field("Search vault")).toHaveTextContent("⌃⌥⌘F");
    });

    it("ignores a plain letter, which would fire while typing", async () => {
      await renderPage();
      await userEvent.click(field("Search vault"));
      await userEvent.keyboard("j");
      expect(mocked.updateSettings).not.toHaveBeenCalled();
      expect(field("Search vault")).toHaveTextContent("Press keys…");
    });

    it("accepts a function key without modifiers", async () => {
      await renderPage();
      await userEvent.click(field("Open Daybook"));
      fireEvent.keyDown(field("Open Daybook"), { key: "F5", code: "F5" });
      expect(savedHotkeys().hub).toBe("f5");
    });

    describe("conflicts", () => {
      it("names another global shortcut that uses the same keys, on both rows", async () => {
        await renderPage({}, { search: "ctrl+alt+cmd+n" });
        expect(rowOf("Search vault").getByText("Also used by Open journal.")).toBeInTheDocument();
        expect(rowOf("Open journal").getByText("Also used by Search vault.")).toBeInTheDocument();
      });

      it("names a macOS shortcut", async () => {
        await renderPage({}, { hub: "cmd+space" });
        expect(rowOf("Open Daybook").getByText("Also used by Spotlight.")).toBeInTheDocument();
      });

      it("names an app menu key", async () => {
        await renderPage({}, { hub: "cmd+k" });
        expect(rowOf("Open Daybook").getByText("Also used by Command palette.")).toBeInTheDocument();
      });

      it("shows the help text again without a conflict", async () => {
        await renderPage();
        expect(rowOf("Search vault").getByText("Find notes.")).toBeInTheDocument();
        expect(screen.queryByText(/Also used by/)).not.toBeInTheDocument();
      });
    });

    it("notes that this build registers none when they are not registered", async () => {
      await renderPage({ registered: false });
      expect(screen.getByText(/registers none while the prototype runs/)).toBeInTheDocument();
    });

    it("has no such note when they are registered", async () => {
      await renderPage();
      expect(screen.queryByText(/registers none/)).not.toBeInTheDocument();
    });
  });

  describe("In Daybook", () => {
    it("lists the app menu's keys as key caps, without a way to record them", async () => {
      await renderPage();
      const group = within(screen.getByRole("heading", { name: "In Daybook" }).closest<HTMLElement>("section")!);
      expect(group.getByText("Command palette")).toBeInTheDocument();
      expect(group.getByText("⌘K")).toBeInTheDocument();
      expect(group.getByText("⌘W")).toBeInTheDocument();
      expect(group.queryByRole("button")).not.toBeInTheDocument();
    });

    it("is left out when the menu has no keys", async () => {
      await renderPage({ menu: [] });
      expect(screen.queryByRole("heading", { name: "In Daybook" })).not.toBeInTheDocument();
    });
  });

  describe("shortcuts in the windows", () => {
    const search = (query: string) => userEvent.type(screen.getByRole("textbox", { name: "Search shortcuts" }), query);
    const commandField = () => screen.getByRole("button", { name: "Record shortcut for Toggle sidebar" });

    it("records a page command's binding in page notation", async () => {
      await renderPage();
      await search("Toggle sidebar");
      await userEvent.click(commandField());
      await userEvent.keyboard("{Meta>}{Shift>}j{/Shift}{/Meta}");
      expect(mocked.setKeybindings).toHaveBeenLastCalledWith({ "sidebar.toggle": "Mod+Shift+J" });
    });

    it("removes a binding with a null override", async () => {
      await renderPage();
      await search("Toggle sidebar");
      await userEvent.click(screen.getByRole("button", { name: "Remove shortcut for Toggle sidebar" }));
      expect(mocked.setKeybindings).toHaveBeenLastCalledWith({ "sidebar.toggle": null });
    });

    it("resets an overridden binding by dropping the override", async () => {
      await renderPage({}, {}, { "sidebar.toggle": "Mod+Shift+J" });
      await search("Toggle sidebar");
      await userEvent.click(screen.getByRole("button", { name: "Reset Toggle sidebar" }));
      expect(mocked.setKeybindings).toHaveBeenLastCalledWith({});
    });
  });
});
