import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { installCommands } from "../../commands";
import { resetStore } from "../../state/store";
import { settings } from "../../test/settings";
import { SettingsDialog } from "./SettingsDialog";

const mocked = vi.mocked(api, true);

function openPage(name: string) {
  return userEvent.click(within(screen.getByRole("navigation", { name: "Settings pages" })).getByRole("button", { name }));
}

function renderDialog(patch: Parameters<typeof resetStore>[0] = {}) {
  resetStore({ settings: settings(), dialog: { type: "settings" }, ...patch });
  installCommands();
  return render(<SettingsDialog />);
}

describe("SettingsDialog", () => {
  beforeEach(() => vi.clearAllMocks());

  it("opens on General, the first page", () => {
    renderDialog();
    expect(screen.getByRole("heading", { name: "General" })).toBeInTheDocument();
  });

  it("switches the shown page from the navigation", async () => {
    renderDialog();
    await openPage("Appearance");
    expect(screen.getByRole("heading", { name: "Appearance" })).toBeInTheDocument();
  });

  describe("navigation", () => {
    const nav = () => screen.getByRole("navigation", { name: "Settings pages" });

    it("lists the pages in order", () => {
      renderDialog();
      expect(within(nav()).getAllByRole("button").map((b) => b.textContent?.trim())).toEqual(["General", "Appearance", "Shortcuts"]);
    });

    it("groups them under App", () => {
      renderDialog();
      expect(nav().textContent).toMatch(/^\s*App\s*General\s*Appearance\s*Shortcuts\s*$/);
    });

    it("has no Logging page", () => {
      renderDialog();
      expect(within(nav()).queryByRole("button", { name: "Logging" })).not.toBeInTheDocument();
    });
  });

  describe("Appearance", () => {
    const schemes = (name: string) => within(screen.getByRole("group", { name }));
    const renderAppearance = (patch: Parameters<typeof settings>[0] = {}) =>
      renderDialog({ settings: settings(patch), dialog: { type: "settings", page: "appearance" } });

    it("has no Mode switcher", () => {
      renderAppearance();
      expect(screen.queryByRole("group", { name: "Mode" })).not.toBeInTheDocument();
    });

    it.each(["Light mode scheme", "Dark mode scheme"])("offers %s as Default, Light and Dark", (name) => {
      renderAppearance();
      expect(schemes(name).getAllByRole("button").map((b) => b.textContent)).toEqual(["Default", "Light", "Dark"]);
    });

    it("presses the saved scheme in each group", () => {
      renderAppearance({ lightScheme: "light", darkScheme: "default" });
      expect(schemes("Light mode scheme").getByRole("button", { name: "Light" })).toHaveAttribute("aria-pressed", "true");
      expect(schemes("Light mode scheme").getByRole("button", { name: "Default" })).toHaveAttribute("aria-pressed", "false");
      expect(schemes("Dark mode scheme").getByRole("button", { name: "Default" })).toHaveAttribute("aria-pressed", "true");
      expect(schemes("Dark mode scheme").getByRole("button", { name: "Light" })).toHaveAttribute("aria-pressed", "false");
    });

    it("saves the clicked tile as the light scheme and leaves the dark one unchanged", async () => {
      renderAppearance({ lightScheme: "default", darkScheme: "dark" });
      await userEvent.click(schemes("Light mode scheme").getByRole("button", { name: "Dark" }));
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ lightScheme: "dark", darkScheme: "dark" }));
    });

    it("saves the clicked tile as the dark scheme and leaves the light one unchanged", async () => {
      renderAppearance({ lightScheme: "default", darkScheme: "dark" });
      await userEvent.click(schemes("Dark mode scheme").getByRole("button", { name: "Light" }));
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ lightScheme: "default", darkScheme: "light" }));
    });

    describe("Text size", () => {
      const sizes = () => schemes("Text size");

      it("offers Small, Medium and Large", () => {
        renderAppearance();
        expect(sizes().getAllByRole("button").map((b) => b.textContent)).toEqual(["Small", "Medium", "Large"]);
      });

      it("presses the saved size", () => {
        renderAppearance({ textSize: "large" });
        expect(sizes().getByRole("button", { name: "Large" })).toHaveAttribute("aria-pressed", "true");
        expect(sizes().getByRole("button", { name: "Medium" })).toHaveAttribute("aria-pressed", "false");
      });

      it("saves the clicked size", async () => {
        renderAppearance();
        await userEvent.click(sizes().getByRole("button", { name: "Small" }));
        expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ textSize: "small" }));
      });

      it("shows no pixel sizes", () => {
        renderAppearance();
        expect(document.body.textContent).not.toMatch(/\d\s*px|\bpx\b/i);
      });

      it.each(["UI size", "Data size", "UI font", "Mono font"])("has no %s field", (label) => {
        renderAppearance();
        expect(screen.queryByLabelText(label)).not.toBeInTheDocument();
      });
    });

    it("saves the accent of the clicked swatch", async () => {
      renderAppearance();
      await userEvent.click(screen.getByRole("button", { name: "Accent teal" }));
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ accent: "teal" }));
    });

    it("saves the system accent when the System swatch is clicked", async () => {
      renderAppearance();
      await userEvent.click(screen.getByRole("button", { name: "Accent system" }));
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ accent: "system" }));
    });

    it("marks the System swatch as pressed while the accent is system", () => {
      renderAppearance({ accent: "system" });
      expect(screen.getByRole("button", { name: "Accent system" })).toHaveAttribute("aria-pressed", "true");
      expect(screen.getByRole("button", { name: "Accent orange" })).toHaveAttribute("aria-pressed", "false");
    });

    it("marks the current accent as pressed", () => {
      renderAppearance();
      expect(screen.getByRole("button", { name: "Accent orange" })).toHaveAttribute("aria-pressed", "true");
      expect(screen.getByRole("button", { name: "Accent teal" })).toHaveAttribute("aria-pressed", "false");
    });
  });

  describe("General", () => {
    it("installs launch at login when the switch is turned on", async () => {
      renderDialog();
      await openPage("General");
      await userEvent.click(screen.getByRole("switch", { name: "Launch at login" }));
      expect(mocked.autostartSet).toHaveBeenLastCalledWith(true);
      await vi.waitFor(() => expect(screen.getByRole("switch", { name: "Launch at login" })).toBeChecked());
    });

    it("shows the launch at login switch on when the LaunchAgent is installed", async () => {
      mocked.autostartStatus.mockResolvedValueOnce(true);
      renderDialog();
      await openPage("General");
      await vi.waitFor(() => expect(screen.getByRole("switch", { name: "Launch at login" })).toBeChecked());
    });
  });

  describe("Shortcuts", () => {
    async function openShortcuts() {
      await openPage("Shortcuts");
      await userEvent.type(screen.getByRole("textbox", { name: "Search shortcuts" }), "Toggle sidebar");
    }

    const commandField = () => screen.getByRole("button", { name: /Record shortcut|Press the new shortcut/ });

    describe("removing a shortcut", () => {
      it("saves a null override for an in-app command", async () => {
        renderDialog();
        await openShortcuts();
        await userEvent.click(screen.getByRole("button", { name: "Remove shortcut for Toggle sidebar" }));
        expect(mocked.setKeybindings).toHaveBeenLastCalledWith({ "sidebar.toggle": null });
        await vi.waitFor(() => expect(commandField()).toHaveTextContent("unbound"));
        expect(screen.queryByRole("button", { name: "Remove shortcut for Toggle sidebar" })).not.toBeInTheDocument();
      });
    });

    it("groups the in-app commands under In Daybook", async () => {
      renderDialog();
      await openPage("Shortcuts");
      expect(screen.getByText("In Daybook")).toBeInTheDocument();
    });

    it("lists the commands with their bindings", async () => {
      renderDialog();
      await openShortcuts();
      expect(commandField()).toHaveTextContent("⌘⌃S");
    });

    it("says so when no command matches the search", async () => {
      renderDialog();
      await openPage("Shortcuts");
      await userEvent.type(screen.getByRole("textbox", { name: "Search shortcuts" }), "nothing like this");
      expect(screen.getByText("No matching commands.")).toBeInTheDocument();
    });

    it("saves the keys pressed while recording as the command's binding", async () => {
      renderDialog();
      await openShortcuts();
      await userEvent.click(commandField());
      await userEvent.keyboard("{Meta>}{Shift>}j{/Shift}{/Meta}");
      expect(mocked.setKeybindings).toHaveBeenLastCalledWith({ "sidebar.toggle": "Mod+Shift+J" });
      await vi.waitFor(() => expect(commandField()).toHaveTextContent("⌘⇧J"));
    });

    it("unbinds the command when Backspace is pressed while recording", async () => {
      renderDialog();
      await openShortcuts();
      await userEvent.click(commandField());
      await userEvent.keyboard("{Backspace}");
      expect(mocked.setKeybindings).toHaveBeenLastCalledWith({ "sidebar.toggle": null });
      await vi.waitFor(() => expect(commandField()).toHaveTextContent("unbound"));
    });

    it("keeps the binding when Escape cancels the recording", async () => {
      renderDialog();
      await openShortcuts();
      await userEvent.click(commandField());
      await userEvent.keyboard("{Escape}");
      expect(mocked.setKeybindings).not.toHaveBeenCalled();
      expect(commandField()).toHaveTextContent("⌘⌃S");
    });

    it("removes the override when the reset button is clicked", async () => {
      renderDialog({ keybindings: { "sidebar.toggle": "Mod+Shift+J" } });
      await openShortcuts();
      expect(commandField()).toHaveTextContent("⌘⇧J");
      await userEvent.click(screen.getByRole("button", { name: "Reset Toggle sidebar" }));
      expect(mocked.setKeybindings).toHaveBeenLastCalledWith({});
      await vi.waitFor(() => expect(commandField()).toHaveTextContent("⌘⌃S"));
    });

    it("warns when two commands share a binding", async () => {
      renderDialog({ keybindings: { "zoom.in": "Mod+Ctrl+S" } });
      await openPage("Shortcuts");
      expect(screen.getByRole("alert")).toHaveTextContent("⌘⌃S is used by both “Toggle sidebar” and “Larger text”.");
    });
  });
});
