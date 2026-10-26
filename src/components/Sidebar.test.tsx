import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api, type HubTool, type HubView, type ToolRow } from "../api";
import { resetStore, useStore } from "../state/store";
import { settings } from "../test/settings";
import { Sidebar } from "./Sidebar";

const mocked = vi.mocked(api, true);

const tool = (patch: Partial<ToolRow> & Pick<ToolRow, "id" | "title">): HubTool => ({
  kind: "tool",
  command: `tool:${patch.id}`,
  shortcut: null,
  detail: null,
  accent: false,
  enabled: true,
  ...patch,
});

const separator: HubTool = { kind: "separator" };

const hubView = (patch: Partial<HubView> = {}): HubView => ({
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
  events: { timeline: { hours: [], workHours: null, fullDay: [], blocks: [], now: null }, rows: [] },
  ...patch,
});

function renderSidebar(patch: Partial<HubView> = {}) {
  resetStore({ settings: settings(), hub: hubView(patch) });
  render(<Sidebar />);
}

const tools = () => within(screen.getByRole("navigation", { name: "Tools" }));
const row = (title: string) => tools().getByRole("button", { name: new RegExp(`^${title}`) });

describe("Sidebar", () => {
  beforeEach(() => vi.clearAllMocks());

  describe("day header", () => {
    it("shows the weekday and the date of the day", () => {
      renderSidebar();
      expect(screen.getByRole("heading", { name: "Saturday" })).toBeInTheDocument();
      expect(screen.getByText("24 October 2026")).toBeInTheDocument();
    });

    it("shows no header before the hub arrived", () => {
      resetStore({ settings: settings(), hub: null });
      render(<Sidebar />);
      expect(screen.queryByRole("heading")).not.toBeInTheDocument();
    });

    it("opens today's note through the command bus", async () => {
      renderSidebar();
      await userEvent.click(screen.getByRole("button", { name: /Saturday/ }));
      expect(mocked.runCommand).toHaveBeenCalledWith("daily:open");
    });

    it("opens the note of another day by its date", async () => {
      renderSidebar({ day: "2026-10-20", isToday: false, weekday: "Tuesday", date: "20 October 2026" });
      await userEvent.click(screen.getByRole("button", { name: /Tuesday/ }));
      expect(mocked.dailyNoteOpen).toHaveBeenCalledWith("2026-10-20");
    });

    it("enables the day arrows only where a neighbouring note exists", () => {
      renderSidebar({ previousDay: "2026-10-22", nextDay: null });
      expect(screen.getByRole("button", { name: "Previous Day with a Note" })).toBeEnabled();
      expect(screen.getByRole("button", { name: "Next Day with a Note" })).toBeDisabled();
    });
  });

  describe("tools", () => {
    it("lists the tool rows in the given order", () => {
      renderSidebar({ tools: [tool({ id: "search", title: "Search vault" }), tool({ id: "status", title: "Status" }), tool({ id: "links", title: "Links" })] });
      expect(tools().getAllByRole("button").map((b) => b.textContent)).toEqual(["Search vault", "Status", "Links"]);
    });

    it("shows a key cap from the row's global shortcut", () => {
      renderSidebar({ tools: [tool({ id: "search", title: "Search vault", shortcut: "ctrl+alt+cmd+f" })] });
      expect(within(row("Search vault")).getByText("⌃⌥⌘F")).toBeInTheDocument();
    });

    it("shows no key cap for a row without a shortcut", () => {
      renderSidebar({ tools: [tool({ id: "status", title: "Status" })] });
      expect(row("Status").querySelector("kbd")).toBeNull();
    });

    it("draws separators between the rows", () => {
      renderSidebar({ tools: [tool({ id: "status", title: "Status" }), separator, tool({ id: "links", title: "Links" })] });
      expect(tools().getAllByRole("separator")).toHaveLength(1);
      expect(tools().getAllByRole("button")).toHaveLength(2);
    });

    it("runs the command of the clicked row", async () => {
      renderSidebar({ tools: [tool({ id: "search", title: "Search vault", command: "search:open" })] });
      await userEvent.click(row("Search vault"));
      expect(mocked.runCommand).toHaveBeenCalledWith("search:open");
    });

    it("disables a row without a command", () => {
      renderSidebar({ tools: [tool({ id: "call", title: "Call", command: null })] });
      expect(row("Call")).toBeDisabled();
    });

    it("disables a row that is not enabled", () => {
      renderSidebar({ tools: [tool({ id: "jira", title: "Open Jira issue", enabled: false })] });
      expect(row("Open Jira issue")).toBeDisabled();
    });

    describe("status row", () => {
      const statusTool = (patch: Partial<ToolRow> = {}) =>
        tool({ id: "status", title: "Taking a break", command: "status:toggle", detail: "12:05", ...patch });

      it("shows the status title and the time it was set", () => {
        renderSidebar({ status: "break", tools: [statusTool()] });
        expect(row("Taking a break")).toHaveTextContent("Taking a break");
        expect(within(row("Taking a break")).getByText("12:05")).toBeInTheDocument();
      });

      it("runs status:toggle when clicked", async () => {
        renderSidebar({ status: "break", tools: [statusTool()] });
        await userEvent.click(row("Taking a break"));
        expect(mocked.runCommand).toHaveBeenCalledWith("status:toggle");
      });

      it("shows no time for a row without a detail", () => {
        renderSidebar({ tools: [statusTool({ title: "No status yet", detail: null })] });
        expect(row("No status yet").textContent).toBe("No status yet");
      });
    });

    it("toasts when a row's command fails", async () => {
      mocked.runCommand.mockRejectedValueOnce({ kind: "io", message: "Not available", code: null });
      renderSidebar({ tools: [tool({ id: "search", title: "Search vault" })] });
      await userEvent.click(row("Search vault"));
      await vi.waitFor(() => expect(useStore.getState().toasts.map((t) => t.message).join()).toContain("Not available"));
    });
  });

  describe("Slack badge", () => {
    const shown = (patch: Partial<NonNullable<HubView["slack"]>> = {}) => ({ emoji: ":white_check_mark:", text: "9:00 - 18:00", presence: null, expiresAt: null, ...patch });

    it.each([
      ["auto", "Slack status: 9:00 - 18:00 · Active"],
      ["away", "Slack status: 9:00 - 18:00 · Away"],
      ["keep", "Slack status: 9:00 - 18:00"],
      [null, "Slack status: 9:00 - 18:00"],
    ] as const)("labels what Slack shows for presence %s", (presence, label) => {
      renderSidebar({ slack: shown({ presence }) });
      expect(screen.getByRole("status", { name: label })).toBeInTheDocument();
    });

    it("shows the emoji glyph and the text", () => {
      renderSidebar({ slack: shown() });
      const badge = screen.getByRole("status");
      expect(badge).toHaveTextContent("✅");
      expect(badge).toHaveTextContent("9:00 - 18:00");
    });

    it("says No status for an empty text", () => {
      renderSidebar({ slack: shown({ text: "", emoji: "" }) });
      expect(screen.getByRole("status", { name: "Slack status: No status" })).toBeInTheDocument();
    });

    it("shows no badge without a Slack status", () => {
      renderSidebar({ slack: null });
      expect(screen.queryByRole("status")).not.toBeInTheDocument();
    });
  });

  describe("settings gear", () => {
    it("runs the app:settings command of the Rust command bus", async () => {
      renderSidebar();
      await userEvent.click(screen.getByRole("button", { name: "Settings" }));
      expect(mocked.runCommand).toHaveBeenCalledWith("app:settings");
    });

    it("opens no dialog by itself", async () => {
      renderSidebar();
      await userEvent.click(screen.getByRole("button", { name: "Settings" }));
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });
  });
});
