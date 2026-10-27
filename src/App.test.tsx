import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("./api", async (orig) => ({ ...(await orig<object>()), api: (await import("./test/apiMock")).createApiMock() }));

import App from "./App";
import { api, EMPTY_UI_STATE, type HubView } from "./api";
import { resetStore, useStore } from "./state/store";
import { settings } from "./test/settings";

const mocked = vi.mocked(api, true);

const hub = (patch: Partial<HubView> = {}): HubView => ({
  day: "2026-10-24",
  isToday: true,
  weekday: "Saturday",
  date: "24 October 2026",
  title: "Good morning",
  previousDay: null,
  nextDay: null,
  tools: [],
  status: null,
  slack: null,
  reminder: null,
  focus: null,
  journal: [],
  routines: [],
  notes: [],
  focusShortcut: null,
  notesShortcut: null,
  todos: [],
  events: { timeline: { hours: [], workHours: null, fullDay: [], blocks: [], now: null }, rows: [] },
  timetracker: { command: "timetracker:show", label: "Open in TimeTracker" },
  activityTab: false,
  ...patch,
});

/** Renders the app and waits until the settings arrived and the commands are installed. */
async function renderApp(patch: Parameters<typeof resetStore>[0] = {}) {
  resetStore(patch);
  render(<App />);
  await vi.waitFor(() => expect(useStore.getState().settings).not.toBeNull());
  await screen.findByRole("main");
  await vi.waitFor(() => expect(mocked.takeLaunchEvents).toHaveBeenCalled());
}

/** The handler the app registered with a backend event, called as Rust would. */
async function emit(listen: { mock: { calls: Array<[(...args: never[]) => void]> } }, ...args: unknown[]) {
  await vi.waitFor(() => expect(listen.mock.calls.length).toBeGreaterThan(0));
  act(() => listen.mock.calls.forEach(([handler]) => handler(...(args as never[]))));
}

const sidebar = () => screen.queryByRole("complementary", { name: "Sidebar" });
const inspector = () => screen.queryByRole("complementary", { name: "Inspector" });
const TOGGLE_SIDEBAR = "{Meta>}{Control>}s{/Control}{/Meta}";
const TOGGLE_INSPECTOR = "{Meta>}{Control>}i{/Control}{/Meta}";

describe("App", () => {
  beforeEach(() => vi.clearAllMocks());

  describe("shell", () => {
    it("shows the sidebar, the board and the inspector", async () => {
      await renderApp();
      expect(sidebar()).toBeInTheDocument();
      expect(screen.getByRole("main")).toBeInTheDocument();
      expect(inspector()).toBeInTheDocument();
    });

    it("does not show the app name in the sidebar", async () => {
      await renderApp();
      expect(within(sidebar()!).queryByText("Daybook")).not.toBeInTheDocument();
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

  describe("hub", () => {
    it("shows the day and the tools Rust sends", async () => {
      mocked.hubView.mockResolvedValueOnce(
        hub({ tools: [{ kind: "tool", id: "search", title: "Search vault", command: "search:open", shortcut: "ctrl+alt+cmd+f", detail: null, accent: false, enabled: true }] }),
      );
      await renderApp();
      expect(await screen.findByRole("heading", { name: "Saturday" })).toBeInTheDocument();
      expect(within(screen.getByRole("navigation", { name: "Tools" })).getByRole("button", { name: /Search vault/ })).toBeInTheDocument();
    });

    it("titles the board with the greeting", async () => {
      await renderApp();
      expect(await screen.findByText("Good morning")).toBeInTheDocument();
    });

    it.each([
      ["onContextChanged", mocked.onContextChanged],
      ["onNoteChanged", mocked.onNoteChanged],
      ["onCalendarChanged", mocked.onCalendarChanged],
      ["onCallChanged", mocked.onCallChanged],
      ["onSlackChanged", mocked.onSlackChanged],
      ["onWindowOpened", mocked.onWindowOpened],
      ["onTimerChanged", mocked.onTimerChanged],
      ["onMusicChanged", mocked.onMusicChanged],
    ])("reloads the hub on %s", async (_name, listen) => {
      await renderApp();
      await screen.findByText("Good morning");
      mocked.hubView.mockResolvedValueOnce(hub({ title: "Good evening" }));
      await emit(listen);
      expect(await screen.findByText("Good evening")).toBeInTheDocument();
    });

    it("shows the new status row when Rust announces a status change", async () => {
      const statusRow = (title: string, status: "working" | "break"): HubView =>
        hub({ status, tools: [{ kind: "tool", id: "status", title, command: "status:toggle", shortcut: null, detail: "12:05", accent: false, enabled: true }] });
      mocked.hubView.mockResolvedValueOnce(hub({ tools: [statusRow("No status yet", "working").tools[0]] }));
      await renderApp();
      const tools = () => within(screen.getByRole("navigation", { name: "Tools" }));
      await tools().findByRole("button", { name: /No status yet/ });
      mocked.hubView.mockResolvedValueOnce(statusRow("Taking a break", "break"));
      await emit(mocked.onStatusChanged, { previous: "working", next: "break", first: false, since: "12:05", message: "", endOfDay: null });
      expect(await tools().findByRole("button", { name: /Taking a break/ })).toBeInTheDocument();
    });

    it("reloads the hub when the settings change", async () => {
      await renderApp();
      await screen.findByText("Good morning");
      mocked.hubView.mockResolvedValueOnce(hub({ title: "Good evening" }));
      await emit(mocked.onSettingsChanged, settings({ accent: "blue" }));
      expect(await screen.findByText("Good evening")).toBeInTheDocument();
    });

    it("reloads the hub every minute", async () => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
      try {
        await renderApp();
        await screen.findByText("Good morning");
        mocked.hubView.mockResolvedValueOnce(hub({ title: "Good evening" }));
        await vi.advanceTimersByTimeAsync(60_000);
        expect(await screen.findByText("Good evening")).toBeInTheDocument();
      } finally {
        vi.useRealTimers();
      }
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

    it("keeps the board while the sidebar is hidden", async () => {
      await renderApp();
      await userEvent.keyboard(TOGGLE_SIDEBAR);
      expect(screen.getByRole("main")).toBeInTheDocument();
    });

    it("starts hidden when the saved layout says so", async () => {
      mocked.getUiState.mockResolvedValueOnce({ ...EMPTY_UI_STATE, sidebarHidden: true });
      await renderApp();
      await vi.waitFor(() => expect(sidebar()).not.toBeInTheDocument());
    });
  });

  describe("inspector", () => {
    it("hides with ⌘⌃I and returns with the same keys", async () => {
      await renderApp();
      await userEvent.keyboard(TOGGLE_INSPECTOR);
      expect(inspector()).not.toBeInTheDocument();
      await userEvent.keyboard(TOGGLE_INSPECTOR);
      expect(inspector()).toBeInTheDocument();
    });

    it("hides and shows with the toolbar button, which names the next action", async () => {
      await renderApp();
      await userEvent.click(screen.getByRole("button", { name: "Hide Inspector" }));
      expect(inspector()).not.toBeInTheDocument();
      await userEvent.click(screen.getByRole("button", { name: "Show Inspector" }));
      expect(inspector()).toBeInTheDocument();
    });

    it("starts hidden when the saved layout says so", async () => {
      mocked.getUiState.mockResolvedValueOnce({ ...EMPTY_UI_STATE, inspectorHidden: true });
      await renderApp();
      await vi.waitFor(() => expect(inspector()).not.toBeInTheDocument());
      expect(screen.getByRole("button", { name: "Show Inspector" })).toHaveAttribute("aria-pressed", "false");
    });

    it("keeps the board while the inspector is hidden", async () => {
      await renderApp();
      await userEvent.keyboard(TOGGLE_INSPECTOR);
      expect(screen.getByRole("main")).toBeInTheDocument();
    });

    it("toggles when Rust announces toggle_inspector", async () => {
      await renderApp();
      await emit(mocked.onToggleInspector);
      expect(inspector()).not.toBeInTheDocument();
      await emit(mocked.onToggleInspector);
      expect(inspector()).toBeInTheDocument();
    });

    it("opens a hidden inspector on the Activity tab when Rust announces show_activity_tab", async () => {
      mocked.hubView.mockResolvedValueOnce(hub({ activityTab: true }));
      mocked.getUiState.mockResolvedValueOnce({ ...EMPTY_UI_STATE, inspectorHidden: true });
      await renderApp();
      await vi.waitFor(() => expect(inspector()).not.toBeInTheDocument());
      await emit(mocked.onShowActivityTab);
      expect(inspector()).toBeInTheDocument();
      expect(within(inspector()!).getByRole("button", { name: "Activity" })).toHaveAttribute("aria-pressed", "true");
    });

    it("answers to the new key when the keybindings change", async () => {
      await renderApp();
      await emit(mocked.onKeybindingsChanged, { "inspector.toggle": "Mod+Shift+J" });
      await userEvent.keyboard(TOGGLE_INSPECTOR);
      expect(inspector()).toBeInTheDocument();
      await userEvent.keyboard("{Meta>}{Shift>}j{/Shift}{/Meta}");
      expect(inspector()).not.toBeInTheDocument();
    });
  });

  describe("closing the window", () => {
    it("asks Rust to close the window on Escape", async () => {
      await renderApp();
      await userEvent.keyboard("{Escape}");
      expect(mocked.runCommand).toHaveBeenCalledWith("window:close");
    });

    it("leaves Escape to a focused text input", async () => {
      await renderApp();
      const input = document.createElement("input");
      document.body.append(input);
      input.focus();
      await userEvent.keyboard("{Escape}");
      input.remove();
      expect(mocked.runCommand).not.toHaveBeenCalledWith("window:close");
    });
  });

  describe("settings", () => {
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
  });

  describe("backend settings", () => {
    it("uses the settings the backend announces as saved", async () => {
      await renderApp();
      await emit(mocked.onSettingsChanged, settings({ accent: "blue", textSize: "large" }));
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
