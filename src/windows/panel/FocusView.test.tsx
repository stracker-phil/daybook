import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type FocusInfo } from "../../api";
import { FocusView } from "./FocusView";

const mocked = vi.mocked(api, true);

const BODY = "- `[ ]` One";
const PROJECTS = [
  { name: "Nexo", emoji: ":nexo:" },
  { name: "Oscorp", emoji: ":oscorp:" },
];

function info(patch: Partial<FocusInfo> = {}): FocusInfo {
  return { body: BODY, projects: PROJECTS, slack: "edit", channel: "#team", ...patch };
}

async function renderView(patch: Partial<FocusInfo> = {}) {
  mocked.focusInfo.mockResolvedValue(info(patch));
  render(<FocusView />);
  return screen.findByRole("textbox", { name: "Focus list" });
}

const cmd = (key: string) => userEvent.keyboard(`{Meta>}${key}{/Meta}`);
const projectKey = (field: HTMLElement, digit: number) => fireEvent.keyDown(field, { key: String(digit), code: `Digit${digit}`, metaKey: true });

describe("FocusView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.focusSave.mockResolvedValue(undefined);
    mocked.focusSend.mockResolvedValue(undefined);
    mocked.panelKeepOpen.mockResolvedValue(undefined);
  });

  describe("opening", () => {
    it("shows today's list in the focused Focus list field", async () => {
      const field = await renderView();
      expect(field).toHaveValue(BODY);
      await waitFor(() => expect(field).toHaveFocus());
    });

    it("shows the message when the list cannot be read", async () => {
      mocked.focusInfo.mockRejectedValue({ kind: "io", message: "Note is unreadable", code: null });
      render(<FocusView />);
      expect(await screen.findByText("Note is unreadable")).toBeInTheDocument();
      expect(screen.queryByRole("textbox", { name: "Focus list" })).not.toBeInTheDocument();
    });

    it.each<[FocusInfo["slack"], string]>([
      ["edit", "Updates your morning message in #team"],
      ["post", "Posts to #team · no morning message yet"],
      ["sent", "End-of-day list sent · today's note only"],
      ["none", "Today's note only"],
    ])("hints where the list goes when the Slack target is %s", async (slack, hint) => {
      await renderView({ slack });
      expect(screen.getByText(hint)).toBeInTheDocument();
    });

    it("shows the projects with their numbers", async () => {
      await renderView();
      expect(screen.getByText("⌘1 Nexo")).toBeInTheDocument();
      expect(screen.getByText("⌘2 Oscorp")).toBeInTheDocument();
    });

    it("shows no project hints without projects", async () => {
      await renderView({ projects: [] });
      expect(screen.queryByLabelText("Projects")).not.toBeInTheDocument();
    });
  });

  describe("⌘L", () => {
    it("moves the checkbox of the line under the caret to the next state", async () => {
      const field = await renderView();
      await cmd("l");
      expect(field).toHaveValue("- `[/]` One");
    });

    it("makes a plain line an open task", async () => {
      const field = await renderView({ body: "Write report" });
      await cmd("l");
      expect(field).toHaveValue("- `[ ]` Write report");
    });
  });

  describe("⌘1…9", () => {
    it("sets the line's project emoji from the project of that number", async () => {
      const field = await renderView();
      projectKey(field, 2);
      expect(field).toHaveValue("- `[ ]` :oscorp: One");
    });

    it("starts an empty list as a task of the project", async () => {
      const field = await renderView({ body: "" });
      projectKey(field, 1);
      expect(field).toHaveValue("- `[ ]` :nexo: ");
    });

    it("does nothing for a number without a project", async () => {
      const field = await renderView();
      projectKey(field, 3);
      expect(field).toHaveValue(BODY);
    });
  });

  describe("⌘↵", () => {
    it("saves exactly what was typed", async () => {
      await renderView();
      await userEvent.type(screen.getByRole("textbox", { name: "Focus list" }), " more");
      await cmd("{Enter}");
      expect(mocked.focusSave).toHaveBeenCalledWith(`${BODY} more`);
    });

    it("saves the list unchanged when nothing was typed", async () => {
      await renderView();
      await cmd("{Enter}");
      expect(mocked.focusSave).toHaveBeenCalledWith(BODY);
    });

    it("shows the message of a failed save in the footer", async () => {
      mocked.focusSave.mockRejectedValue({ kind: "io", message: "Disk full", code: null });
      await renderView();
      await cmd("{Enter}");
      expect(await screen.findByText("Disk full")).toBeInTheDocument();
    });

    it("drops the error once the list is edited", async () => {
      mocked.focusSave.mockRejectedValue({ kind: "io", message: "Disk full", code: null });
      const field = await renderView();
      await cmd("{Enter}");
      await screen.findByText("Disk full");
      await userEvent.type(field, "!");
      expect(screen.queryByText("Disk full")).not.toBeInTheDocument();
    });
  });

  describe("⌘S", () => {
    it.each<FocusInfo["slack"]>(["edit", "post"])("sends the list as typed to Slack without saving or closing when the target is %s", async (slack) => {
      const field = await renderView({ slack });
      await userEvent.type(field, " more");
      await cmd("s");
      expect(mocked.focusSend).toHaveBeenCalledWith(`${BODY} more`);
      expect(mocked.focusSave).not.toHaveBeenCalled();
      expect(mocked.panelClose).not.toHaveBeenCalled();
      expect(await screen.findByText("Sent to Slack")).toBeInTheDocument();
      expect(field).toHaveValue(`${BODY} more`);
    });

    it("says the list changed once it is edited after sending", async () => {
      const field = await renderView();
      await cmd("s");
      await screen.findByText("Sent to Slack");
      await userEvent.type(field, "!");
      expect(screen.getByText("Changed since sending to Slack")).toBeInTheDocument();
    });

    it("says it is sent again once the edit is sent too", async () => {
      const field = await renderView();
      await cmd("s");
      await screen.findByText("Sent to Slack");
      await userEvent.type(field, "!");
      await cmd("s");
      expect(mocked.focusSend).toHaveBeenLastCalledWith(`${BODY}!`);
      expect(await screen.findByText("Sent to Slack")).toBeInTheDocument();
    });

    it.each<FocusInfo["slack"]>(["none", "sent"])("does nothing when the target is %s", async (slack) => {
      await renderView({ slack });
      await cmd("s");
      expect(mocked.focusSend).not.toHaveBeenCalled();
      expect(screen.queryByText("Sent to Slack")).not.toBeInTheDocument();
      expect(screen.queryByText(/send to Slack/)).not.toBeInTheDocument();
    });

    it("offers the shortcut in the footer when the list can go to Slack", async () => {
      await renderView();
      expect(screen.getByText("⌘S send to Slack")).toBeInTheDocument();
    });

    it("shows the message of a failed send and no sent state", async () => {
      mocked.focusSend.mockRejectedValue({ kind: "io", message: "Slack is unreachable", code: null });
      await renderView();
      await cmd("s");
      expect(await screen.findByText("Slack is unreachable")).toBeInTheDocument();
      expect(screen.queryByText("Sent to Slack")).not.toBeInTheDocument();
    });
  });
});
