import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api } from "../api";
import { installCommands } from "../commands";
import { resetStore, type Store } from "../state/store";
import { settings } from "../test/settings";
import { Dialogs } from "./Dialogs";
import { Sidebar } from "./Sidebar";

const mocked = vi.mocked(api, true);

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
    it("runs the app:settings command of the Rust command bus", async () => {
      renderSidebar();
      await userEvent.click(gear());
      expect(mocked.runCommand).toHaveBeenCalledWith("app:settings");
    });

    it("opens no dialog by itself", async () => {
      renderSidebar();
      await userEvent.click(gear());
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("names the native menu shortcut in its title", () => {
      renderSidebar();
      expect(gear()).toHaveAttribute("title", "Settings (⌘,)");
    });

    it("keeps its title when the keybindings change", () => {
      renderSidebar({ keybindings: { "sidebar.toggle": null } });
      expect(gear()).toHaveAttribute("title", "Settings (⌘,)");
    });
  });
});
