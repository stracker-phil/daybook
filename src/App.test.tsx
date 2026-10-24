import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("./api", async (orig) => ({ ...(await orig<object>()), api: (await import("./test/apiMock")).createApiMock() }));

import App from "./App";
import { api } from "./api";
import { resetStore, useStore } from "./state/store";
import { settings } from "./test/settings";

const mocked = vi.mocked(api, true);

/** Renders the app and waits until the settings arrived and the commands are installed. */
async function renderApp(patch: Parameters<typeof resetStore>[0] = {}) {
  resetStore(patch);
  render(<App />);
  await vi.waitFor(() => expect(useStore.getState().settings).not.toBeNull());
  await screen.findByRole("region", { name: "Daybook" });
}

const sidebar = () => screen.queryByRole("complementary", { name: "Sidebar" });
const TOGGLE_SIDEBAR = "{Meta>}{Control>}s{/Control}{/Meta}";

describe("App", () => {
  beforeEach(() => vi.clearAllMocks());

  describe("shell", () => {
    it("shows the sidebar with the app name next to an empty main region", async () => {
      await renderApp();
      expect(within(sidebar()!).getByText("Daybook")).toBeInTheDocument();
      expect(screen.getByRole("region", { name: "Daybook" })).toBeEmptyDOMElement();
    });

    it("shows no dialog on start", async () => {
      await renderApp();
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("shows a toast for an error launch event", async () => {
      mocked.takeLaunchEvents.mockResolvedValueOnce([{ type: "error", message: "Could not open the link" }]);
      await renderApp();
      expect(await screen.findByRole("alert")).toHaveTextContent("Could not open the link");
    });

    it("shows an info toast for a notice launch event", async () => {
      mocked.takeLaunchEvents.mockResolvedValueOnce([{ type: "notice", message: "Opened the settings" }]);
      await renderApp();
      const message = await screen.findByText("Opened the settings");
      expect(message.closest('[role="status"]')).not.toBeNull();
      expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    });

    it("shows a toast when the launch events cannot be read", async () => {
      mocked.takeLaunchEvents.mockRejectedValueOnce({ kind: "io", message: "Channel closed", code: null });
      await renderApp();
      expect(await screen.findByRole("alert")).toHaveTextContent("Launch request failed: Channel closed");
    });
  });

  describe("sidebar", () => {
    it("hides with ⌘⌃S and returns with the same keys", async () => {
      await renderApp();
      await userEvent.keyboard(TOGGLE_SIDEBAR);
      expect(sidebar()).not.toBeInTheDocument();
      await userEvent.keyboard(TOGGLE_SIDEBAR);
      expect(sidebar()).toBeInTheDocument();
    });

    it("keeps the main region while the sidebar is hidden", async () => {
      await renderApp();
      await userEvent.keyboard(TOGGLE_SIDEBAR);
      expect(screen.getByRole("region", { name: "Daybook" })).toBeInTheDocument();
    });

    it("starts hidden when the saved layout says so", async () => {
      mocked.getUiState.mockResolvedValueOnce({ sidebarHidden: true, sidebarWidth: 200, paneSizes: {} });
      await renderApp();
      await vi.waitFor(() => expect(sidebar()).not.toBeInTheDocument());
    });
  });

  describe("settings", () => {
    async function openSettingsEvent(page: string | null) {
      await vi.waitFor(() => expect(mocked.onOpenSettings).toHaveBeenCalled());
      const announce = mocked.onOpenSettings.mock.calls[0][0];
      act(() => announce(page));
    }

    it("opens on the named page when Rust asks for it", async () => {
      await renderApp();
      await openSettingsEvent("appearance");
      const dialog = await screen.findByRole("dialog");
      expect(within(dialog).getByRole("heading", { name: "Appearance" })).toBeInTheDocument();
    });

    it("opens on the first page when Rust names none", async () => {
      await renderApp();
      await openSettingsEvent(null);
      const dialog = await screen.findByRole("dialog");
      expect(within(dialog).getByRole("heading", { name: "General" })).toBeInTheDocument();
    });

    it("does not open with ⌘, in the window; the native menu owns that key", async () => {
      await renderApp();
      await userEvent.keyboard("{Meta>},{/Meta}");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("asks Rust to open Settings from the gear in the sidebar", async () => {
      await renderApp();
      await userEvent.click(within(sidebar()!).getByRole("button", { name: "Settings" }));
      expect(mocked.runCommand).toHaveBeenCalledWith("app:settings");
    });

    it("closes with Escape", async () => {
      await renderApp();
      await openSettingsEvent(null);
      await screen.findByRole("dialog");
      await userEvent.keyboard("{Escape}");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });
  });

  describe("backend settings", () => {
    it("uses the settings the backend announces as saved", async () => {
      await renderApp();
      await vi.waitFor(() => expect(mocked.onSettingsChanged).toHaveBeenCalled());
      const announce = mocked.onSettingsChanged.mock.calls[0][0];
      act(() => announce(settings({ accent: "blue", textSize: "large" })));
      expect(useStore.getState().settings).toMatchObject({ accent: "blue", textSize: "large" });
    });
  });

  describe("shortcuts dialog", () => {
    it("opens with ? and closes with it again", async () => {
      await renderApp();
      await userEvent.keyboard("{Shift>}?{/Shift}");
      expect(await screen.findByRole("dialog", { name: "Keyboard shortcuts" })).toBeInTheDocument();
      await userEvent.keyboard("{Shift>}?{/Shift}");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });
  });

  describe("command palette", () => {
    it("does not open in the window with ⌘K; it is an overlay window now", async () => {
      await renderApp();
      await userEvent.keyboard("{Meta>}k{/Meta}");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });
  });
});
