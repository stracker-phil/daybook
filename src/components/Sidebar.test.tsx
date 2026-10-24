import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { installCommands } from "../commands";
import { resetStore, type Store } from "../state/store";
import { settings } from "../test/settings";
import { Dialogs } from "./Dialogs";
import { Sidebar } from "./Sidebar";

function renderSidebar(patch: Partial<Store> = {}) {
  resetStore({ settings: settings(), ...patch });
  installCommands();
  render(
    <>
      <Sidebar />
      <Dialogs />
    </>,
  );
}

const gear = () => screen.getByRole("button", { name: "Settings" });

describe("Sidebar", () => {
  beforeEach(() => vi.clearAllMocks());

  it("shows the app name", () => {
    renderSidebar();
    expect(within(screen.getByRole("complementary", { name: "Sidebar" })).getByText("Daybook")).toBeInTheDocument();
  });

  describe("settings gear", () => {
    it("opens the Settings dialog on its first page", async () => {
      renderSidebar();
      await userEvent.click(gear());
      const dialog = await screen.findByRole("dialog");
      expect(within(dialog).getByRole("heading", { name: "General" })).toBeInTheDocument();
    });

    it("names the shortcut in its title", () => {
      renderSidebar();
      expect(gear()).toHaveAttribute("title", "Settings (⌘,)");
    });

    it("follows a rebinding of the shortcut", () => {
      renderSidebar({ keybindings: { "settings.toggle": "Mod+J" } });
      expect(gear()).toHaveAttribute("title", "Settings (⌘J)");
    });

    it("has a plain title while the shortcut is unbound", () => {
      renderSidebar({ keybindings: { "settings.toggle": null } });
      expect(gear()).toHaveAttribute("title", "Settings");
    });
  });
});
