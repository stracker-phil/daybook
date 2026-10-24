import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("./api", async (orig) => ({ ...(await orig<object>()), api: (await import("./test/apiMock")).createApiMock() }));

import App from "./App";
import { api } from "./api";
import { resetStore, useStore } from "./state/store";

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

    it("shows no dialog or palette on start", async () => {
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
    it("opens with ⌘, and closes with it again", async () => {
      await renderApp();
      await userEvent.keyboard("{Meta>},{/Meta}");
      const dialog = await screen.findByRole("dialog");
      expect(within(dialog).getByRole("heading", { name: "General" })).toBeInTheDocument();
      await userEvent.keyboard("{Meta>},{/Meta}");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("opens from the gear in the sidebar", async () => {
      await renderApp();
      await userEvent.click(within(sidebar()!).getByRole("button", { name: "Settings" }));
      expect(await screen.findByRole("heading", { name: "General" })).toBeInTheDocument();
    });

    it("opens from the command palette", async () => {
      await renderApp();
      await userEvent.keyboard("{Meta>}k{/Meta}");
      await userEvent.type(await screen.findByLabelText("Search"), "settings");
      await userEvent.click(screen.getByRole("option", { name: /^Settings…/ }));
      expect(await screen.findByRole("heading", { name: "General" })).toBeInTheDocument();
    });

    it("closes with Escape", async () => {
      await renderApp();
      await userEvent.keyboard("{Meta>},{/Meta}");
      await screen.findByRole("dialog");
      await userEvent.keyboard("{Escape}");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
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
    it("opens with ⌘K", async () => {
      await renderApp();
      await userEvent.keyboard("{Meta>}k{/Meta}");
      expect(await screen.findByRole("dialog", { name: "Command palette" })).toBeInTheDocument();
    });

    it("closes with ⌘K again", async () => {
      await renderApp();
      await userEvent.keyboard("{Meta>}k{/Meta}");
      await screen.findByRole("dialog", { name: "Command palette" });
      await userEvent.keyboard("{Meta>}k{/Meta}");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("runs the chosen command on Enter and closes", async () => {
      await renderApp();
      await userEvent.keyboard("{Meta>}k{/Meta}");
      await userEvent.type(await screen.findByLabelText("Search"), ">toggle sidebar{Enter}");
      expect(sidebar()).not.toBeInTheDocument();
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("closes with Escape without running anything", async () => {
      await renderApp();
      await userEvent.keyboard("{Meta>}k{/Meta}");
      await screen.findByRole("dialog");
      await userEvent.keyboard("{Escape}");
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
      expect(sidebar()).toBeInTheDocument();
    });
  });
});
