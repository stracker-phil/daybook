import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type ActionLogEvent, type ActionLogLine, type DiagnosticRow } from "../../api";
import { resetStore, useStore } from "../../state/store";
import { StatusPage } from "./StatusPage";

const mocked = vi.mocked(api, true);

const row = (patch: Partial<DiagnosticRow> = {}): DiagnosticRow => ({
  label: "Vault",
  value: "Found",
  tone: "ok",
  command: null,
  commandLabel: null,
  ...patch,
});

const line = (patch: Partial<ActionLogLine> = {}): ActionLogLine => ({
  time: "09:00",
  target: "obsidian",
  state: "done",
  text: "Added a todo",
  detail: null,
  count: 1,
  ...patch,
});

const event = (patch: Partial<ActionLogEvent> = {}): ActionLogEvent => ({
  id: "e1",
  day: "2026-10-27",
  dayLabel: "Today",
  time: "09:00",
  trigger: "Routine finished",
  lines: [line()],
  ...patch,
});

const showDiagnostics = (rows: DiagnosticRow[], title = "Connections") =>
  mocked.diagnosticsGet.mockResolvedValue({ groups: [{ title, rows }] });
const showLog = (...events: ActionLogEvent[]) => mocked.actionLogGet.mockResolvedValue({ events });

const renderPage = () => render(<StatusPage />);
const logRegion = async () => screen.findByRole("region", { name: "Action log" });
const lineOf = async (text: string) => {
  const item = (await screen.findByText(text)).closest("li");
  if (!item) throw new Error(`no line for ${text}`);
  return within(item);
};

describe("StatusPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
    mocked.diagnosticsGet.mockResolvedValue({ groups: [] });
    mocked.actionLogGet.mockResolvedValue({ events: [] });
    mocked.actionLogClear.mockResolvedValue(undefined);
    mocked.runCommand.mockResolvedValue(undefined);
    mocked.onActionLogChanged.mockImplementation(() => Promise.resolve(() => {}));
  });

  describe("diagnostics", () => {
    it("shows each group title as a heading with its rows", async () => {
      mocked.diagnosticsGet.mockResolvedValue({
        groups: [
          { title: "Connections", rows: [row({ label: "Vault", value: "Found" })] },
          { title: "Permissions", rows: [row({ label: "Calendar", value: "Allowed" })] },
        ],
      });
      renderPage();
      expect(await screen.findByRole("heading", { name: "Connections", level: 3 })).toBeInTheDocument();
      expect(screen.getByRole("heading", { name: "Permissions", level: 3 })).toBeInTheDocument();
      expect(screen.getByText("Vault")).toBeInTheDocument();
      expect(screen.getByText("Found")).toBeInTheDocument();
      expect(screen.getByText("Calendar")).toBeInTheDocument();
      expect(screen.getByText("Allowed")).toBeInTheDocument();
    });

    it.each([
      ["ok", "OK"],
      ["warn", "Warning"],
      ["error", "Error"],
      ["info", "Info"],
    ] as const)("marks a %s row with the %s icon", async (tone, name) => {
      showDiagnostics([row({ value: "Some value", tone })]);
      renderPage();
      const value = (await screen.findByText("Some value")).parentElement as HTMLElement;
      expect(within(value).getByRole("img", { name })).toBeInTheDocument();
    });

    it("runs the row's command when its button is clicked", async () => {
      showDiagnostics([row({ tone: "warn", command: "calendar.grant", commandLabel: "Allow Access" })]);
      renderPage();
      await userEvent.click(await screen.findByRole("button", { name: "Allow Access" }));
      expect(mocked.runCommand).toHaveBeenCalledWith("calendar.grant");
    });

    it.each([
      ["no command", { command: null, commandLabel: "Allow Access" }],
      ["no label", { command: "calendar.grant", commandLabel: null }],
      ["neither", { command: null, commandLabel: null }],
    ])("shows no button for a row with %s", async (_name, patch) => {
      showDiagnostics([row(patch)]);
      renderPage();
      await screen.findByText("Found");
      await screen.findByRole("button", { name: "Clear Log" });
      expect(screen.getAllByRole("button").map((b) => b.textContent)).toEqual(["Clear Log"]);
    });

    it("toasts an error when the command fails", async () => {
      showDiagnostics([row({ tone: "warn", command: "calendar.grant", commandLabel: "Allow Access" })]);
      mocked.runCommand.mockRejectedValue({ kind: "io", message: "Denied", code: null });
      renderPage();
      await userEvent.click(await screen.findByRole("button", { name: "Allow Access" }));
      await vi.waitFor(() => expect(useStore.getState().toasts[0]?.message).toBe("Couldn't run Allow Access: Denied"));
    });

    it("reloads when the window gets focus again", async () => {
      showDiagnostics([row({ value: "Missing", tone: "error" })]);
      renderPage();
      expect(await screen.findByText("Missing")).toBeInTheDocument();
      showDiagnostics([row({ value: "Found" })]);
      act(() => {
        fireEvent.focus(window);
      });
      expect(await screen.findByText("Found")).toBeInTheDocument();
      expect(screen.queryByText("Missing")).not.toBeInTheDocument();
    });

    it("shows an alert when the diagnostics cannot be read", async () => {
      mocked.diagnosticsGet.mockRejectedValue({ kind: "io", message: "Diagnostics broke", code: null });
      renderPage();
      expect(await screen.findByRole("alert")).toHaveTextContent("Diagnostics broke");
    });
  });

  describe("action log", () => {
    it("shows a day header per day, shared by consecutive events of that day", async () => {
      showLog(
        event({ id: "a", trigger: "Routine finished" }),
        event({ id: "b", trigger: "Status changed" }),
        event({ id: "c", trigger: "Journal closed", day: "2026-10-26", dayLabel: "Yesterday" }),
      );
      renderPage();
      const region = within(await logRegion());
      expect(region.getAllByRole("heading", { level: 4 }).map((h) => h.textContent)).toEqual(["Today", "Yesterday"]);
    });

    it("shows an event as an article named by its trigger, with its time", async () => {
      showLog(event({ trigger: "Routine finished", time: "09:41", lines: [line({ time: "09:41" })] }));
      renderPage();
      const article = within(await screen.findByRole("article", { name: "Routine finished" }));
      expect(article.getByText("09:41")).toBeInTheDocument();
    });

    it.each([
      ["obsidian", "Obsidian"],
      ["slackStatus", "Slack status"],
      ["slackMessage", "Slack message"],
      ["timetracker", "TimeTracker"],
      ["daybook", "Daybook"],
    ] as const)("labels a %s line as %s next to its text", async (target, label) => {
      showLog(event({ lines: [line({ target, text: "Did the thing" })] }));
      renderPage();
      const item = await lineOf("Did the thing");
      expect(item.getByText(label)).toBeInTheDocument();
    });

    it.each([
      ["done", "Done"],
      ["sending", "Sending"],
      ["waiting", "Waiting"],
      ["skipped", "Not sent"],
      ["failed", "Failed"],
    ] as const)("marks a %s line with the %s icon", async (state, name) => {
      showLog(event({ lines: [line({ state, text: "Did the thing" })] }));
      renderPage();
      const item = await lineOf("Did the thing");
      expect(item.getByRole("img", { name })).toBeInTheDocument();
    });

    it("shows the repeat count only for merged lines", async () => {
      showLog(event({ lines: [line({ text: "Merged", count: 3 }), line({ text: "Single", count: 1 })] }));
      renderPage();
      expect((await lineOf("Merged")).getByText("×3")).toBeInTheDocument();
      expect((await lineOf("Single")).queryByText(/×/)).not.toBeInTheDocument();
    });

    it("shows a line's detail", async () => {
      showLog(event({ lines: [line({ text: "Failed send", state: "failed", detail: "Slack returned 500" })] }));
      renderPage();
      expect((await lineOf("Failed send")).getByText("Slack returned 500")).toBeInTheDocument();
    });

    it("shows a line's time only when it differs from the event's", async () => {
      showLog(event({ time: "09:00", lines: [line({ text: "Same time", time: "09:00" }), line({ text: "Later", time: "09:05", detail: "Retried" })] }));
      renderPage();
      expect((await lineOf("Same time")).queryByText("09:00")).not.toBeInTheDocument();
      expect((await lineOf("Later")).getByText("09:05 · Retried")).toBeInTheDocument();
    });

    it("says nothing was sent and disables Clear Log for an empty log", async () => {
      renderPage();
      expect(await screen.findByText("Nothing sent yet.")).toBeInTheDocument();
      expect(within(await logRegion()).getByRole("button", { name: "Clear Log" })).toBeDisabled();
    });

    it("enables Clear Log when there are events", async () => {
      showLog(event());
      renderPage();
      expect(within(await logRegion()).getByRole("button", { name: "Clear Log" })).toBeEnabled();
    });

    it("clears the log and shows the reloaded, empty log", async () => {
      showLog(event());
      renderPage();
      await screen.findByRole("article", { name: "Routine finished" });
      showLog();
      await userEvent.click(screen.getByRole("button", { name: "Clear Log" }));
      expect(mocked.actionLogClear).toHaveBeenCalledWith();
      expect(await screen.findByText("Nothing sent yet.")).toBeInTheDocument();
      expect(screen.queryByRole("article")).not.toBeInTheDocument();
    });

    it("toasts an error when clearing fails", async () => {
      showLog(event());
      mocked.actionLogClear.mockRejectedValue({ kind: "io", message: "Locked", code: null });
      renderPage();
      await userEvent.click(await screen.findByRole("button", { name: "Clear Log" }));
      await vi.waitFor(() => expect(useStore.getState().toasts[0]?.message).toBe("Couldn't clear the action log: Locked"));
    });

    it("reloads when Rust reports a change to the log", async () => {
      let changed: () => void = () => {};
      mocked.onActionLogChanged.mockImplementation((handler) => {
        changed = handler;
        return Promise.resolve(() => {});
      });
      renderPage();
      await screen.findByText("Nothing sent yet.");
      showLog(event({ trigger: "Status changed" }));
      act(() => changed());
      expect(await screen.findByRole("article", { name: "Status changed" })).toBeInTheDocument();
    });

    it("shows an alert with the message when the log cannot be loaded", async () => {
      mocked.actionLogGet.mockRejectedValue({ kind: "io", message: "Log unreadable", code: null });
      renderPage();
      expect(await screen.findByRole("alert")).toHaveTextContent("Log unreadable");
      expect(screen.queryByRole("region", { name: "Action log" })).not.toBeInTheDocument();
    });
  });
});
