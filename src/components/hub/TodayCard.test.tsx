import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type ReminderLine } from "../../api";
import { resetStore } from "../../state/store";
import { TodayCard } from "./TodayCard";

const mocked = vi.mocked(api, true);

const reminder = (patch: Partial<ReminderLine> = {}): ReminderLine => ({ label: "Routine", text: "Morning routine is due", action: null, ...patch });

describe("TodayCard", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
  });

  it("renders nothing without a reminder", () => {
    const { container } = render(<TodayCard reminder={null} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("shows the label and the text of the reminder", () => {
    render(<TodayCard reminder={reminder()} />);
    expect(screen.getByText("Routine")).toBeInTheDocument();
    expect(screen.getByText("Morning routine is due")).toBeInTheDocument();
  });

  it("is a plain line without a button when the reminder has no action", () => {
    render(<TodayCard reminder={reminder()} />);
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });

  describe("with an action", () => {
    const command = reminder({ action: { kind: "command", label: "Start", command: "routines:open" } });
    const join = reminder({ label: "Now", text: "Standup", action: { kind: "join", label: "Join", url: "https://zoom.us/j/1" } });

    it("shows the action label inside the button", () => {
      render(<TodayCard reminder={command} />);
      expect(screen.getByRole("button")).toHaveTextContent("Start");
    });

    it("runs the command of a command action when clicked", async () => {
      render(<TodayCard reminder={command} />);
      await userEvent.click(screen.getByRole("button"));
      expect(mocked.runCommand).toHaveBeenCalledWith("routines:open");
      expect(mocked.calendarJoin).not.toHaveBeenCalled();
    });

    it("joins the meeting of a join action when clicked", async () => {
      render(<TodayCard reminder={join} />);
      await userEvent.click(screen.getByRole("button"));
      expect(mocked.calendarJoin).toHaveBeenCalledWith("https://zoom.us/j/1");
      expect(mocked.runCommand).not.toHaveBeenCalled();
    });

    it("takes the focus when it appears, so Enter runs it", async () => {
      render(<TodayCard reminder={command} />);
      expect(screen.getByRole("button")).toHaveFocus();
      await userEvent.keyboard("{Enter}");
      expect(mocked.runCommand).toHaveBeenCalledWith("routines:open");
    });

    it("takes the focus again when the window opens", async () => {
      render(<TodayCard reminder={command} />);
      act(() => screen.getByRole("button").blur());
      expect(screen.getByRole("button")).not.toHaveFocus();
      await vi.waitFor(() => expect(mocked.onWindowOpened).toHaveBeenCalled());
      const [[opened]] = mocked.onWindowOpened.mock.calls;
      act(() => opened());
      expect(screen.getByRole("button")).toHaveFocus();
    });
  });
});
