import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type FocusLine, type ReminderLine } from "../../api";
import { resetStore } from "../../state/store";
import { TodayCard } from "./TodayCard";

const mocked = vi.mocked(api, true);

const reminder = (patch: Partial<ReminderLine> = {}): ReminderLine => ({ label: "Routine", text: "Morning routine is due", action: null, ...patch });

interface CardProps {
  reminder?: ReminderLine | null;
  focus?: FocusLine[] | null;
  isToday?: boolean;
  focusShortcut?: string | null;
}

function renderCard({ reminder = null, focus = null, isToday = true, focusShortcut = null }: CardProps = {}) {
  return render(<TodayCard reminder={reminder} focus={focus} isToday={isToday} focusShortcut={focusShortcut} />);
}

const FOCUS: FocusLine[] = [
  { mark: "in-progress", tag: "Nexo", text: "Ship the release" },
  { mark: null, tag: null, text: "Fixed the build" },
  { mark: "cancelled", tag: null, text: "Dropped idea" },
];

describe("TodayCard", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
  });

  it("renders nothing without a reminder and without a focus list", () => {
    const { container } = renderCard();
    expect(container).toBeEmptyDOMElement();
  });

  describe("reminder", () => {
    it("shows the label and the text of the reminder", () => {
      renderCard({ reminder: reminder() });
      expect(screen.getByText("Routine")).toBeInTheDocument();
      expect(screen.getByText("Morning routine is due")).toBeInTheDocument();
    });

    it("is a plain line without a button when the reminder has no action", () => {
      renderCard({ reminder: reminder() });
      expect(screen.queryByRole("button")).not.toBeInTheDocument();
    });

    it("shows no Focus section without a focus list", () => {
      renderCard({ reminder: reminder() });
      expect(screen.queryByRole("region", { name: "Focus" })).not.toBeInTheDocument();
    });

    describe("with an action", () => {
      const command = reminder({ action: { kind: "command", label: "Start", command: "routines:open" } });
      const join = reminder({ label: "Now", text: "Standup", action: { kind: "join", label: "Join", url: "https://zoom.us/j/1" } });

      it("shows the action label inside the button", () => {
        renderCard({ reminder: command });
        expect(screen.getByRole("button")).toHaveTextContent("Start");
      });

      it("runs the command of a command action when clicked", async () => {
        renderCard({ reminder: command });
        await userEvent.click(screen.getByRole("button"));
        expect(mocked.runCommand).toHaveBeenCalledWith("routines:open");
        expect(mocked.calendarJoin).not.toHaveBeenCalled();
      });

      it("joins the meeting of a join action when clicked", async () => {
        renderCard({ reminder: join });
        await userEvent.click(screen.getByRole("button"));
        expect(mocked.calendarJoin).toHaveBeenCalledWith("https://zoom.us/j/1");
        expect(mocked.runCommand).not.toHaveBeenCalled();
      });

      it("takes the focus when it appears, so Enter runs it", async () => {
        renderCard({ reminder: command });
        expect(screen.getByRole("button")).toHaveFocus();
        await userEvent.keyboard("{Enter}");
        expect(mocked.runCommand).toHaveBeenCalledWith("routines:open");
      });

      it("takes the focus again when the window opens", async () => {
        renderCard({ reminder: command });
        act(() => screen.getByRole("button").blur());
        expect(screen.getByRole("button")).not.toHaveFocus();
        await vi.waitFor(() => expect(mocked.onWindowOpened).toHaveBeenCalled());
        const [[opened]] = mocked.onWindowOpened.mock.calls;
        act(() => opened());
        expect(screen.getByRole("button")).toHaveFocus();
      });
    });
  });

  describe("focus list", () => {
    const section = () => screen.getByRole("region", { name: "Focus" });

    it("shows a task with its tag and text", () => {
      renderCard({ focus: FOCUS });
      expect(section()).toHaveTextContent("Nexo");
      expect(screen.getByText("Ship the release").closest("[data-mark]")).toHaveAttribute("data-mark", "in-progress");
    });

    it("marks a cancelled task", () => {
      renderCard({ focus: FOCUS });
      expect(screen.getByText("Dropped idea").closest("[data-mark]")).toHaveAttribute("data-mark", "cancelled");
    });

    it("shows a plain line as a sub-item without a mark", () => {
      renderCard({ focus: FOCUS });
      expect(screen.getByText("Fixed the build").closest("[data-mark]")).toBeNull();
    });

    it("opens the focus overlay when the list is clicked", async () => {
      renderCard({ focus: FOCUS });
      await userEvent.click(screen.getByRole("button", { name: "Edit Today's Focus" }));
      expect(mocked.runCommand).toHaveBeenCalledWith("focus:open");
    });

    it("invites to set today's focus when the list is empty", async () => {
      renderCard({ focus: [] });
      expect(screen.getByText("Set today's focus…")).toBeInTheDocument();
      await userEvent.click(screen.getByRole("button", { name: "Set Today's Focus" }));
      expect(mocked.runCommand).toHaveBeenCalledWith("focus:open");
    });

    it("cannot be edited on another day", async () => {
      renderCard({ focus: FOCUS, isToday: false });
      const button = screen.getByRole("button", { name: "Edit Today's Focus" });
      expect(button).toBeDisabled();
      await userEvent.click(button);
      expect(mocked.runCommand).not.toHaveBeenCalled();
    });

    it("says there is no list on another day when it is empty", () => {
      renderCard({ focus: [], isToday: false });
      expect(screen.getByText("No focus list.")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Set Today's Focus" })).toBeDisabled();
      expect(screen.queryByText("Set today's focus…")).not.toBeInTheDocument();
    });

    it("shows the focus shortcut as a key cap beside the title today", () => {
      renderCard({ focus: FOCUS, focusShortcut: "ctrl+alt+cmd+f" });
      expect(within(screen.getByRole("heading", { name: /Focus/ })).getByText("⌃⌥⌘F")).toBeInTheDocument();
    });

    it("shows no key cap on another day", () => {
      renderCard({ focus: FOCUS, isToday: false, focusShortcut: "ctrl+alt+cmd+f" });
      expect(screen.queryByText("⌃⌥⌘F")).not.toBeInTheDocument();
    });

    it("separates the reminder from the focus list", () => {
      renderCard({ reminder: reminder(), focus: FOCUS });
      expect(screen.getByText("Morning routine is due")).toBeInTheDocument();
      expect(screen.getByRole("separator")).toBeInTheDocument();
      expect(section()).toBeInTheDocument();
    });

    it("shows no separator without a reminder", () => {
      renderCard({ focus: FOCUS });
      expect(screen.queryByRole("separator")).not.toBeInTheDocument();
    });
  });
});
