import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type DayEvents, type EventRow, type HubView, type TimelineBlock } from "../../api";
import { flushUiState, resetStore, useStore } from "../../state/store";
import { Events } from "./Events";

const mocked = vi.mocked(api, true);

const row = (patch: Partial<EventRow> & Pick<EventRow, "key" | "title">): EventRow => ({
  time: "09:00–09:30",
  calendar: "Work",
  meeting: null,
  past: false,
  ...patch,
});

const block = (key: string, label: string, patch: Partial<TimelineBlock> = {}): TimelineBlock => ({
  key,
  span: { from: 0.1, to: 0.2 },
  label,
  past: false,
  follows: false,
  ...patch,
});

const events = (rows: EventRow[], blocks: TimelineBlock[] = [], now: number | null = null): DayEvents => ({
  timeline: { hours: [], workHours: null, fullDay: [], blocks, now },
  rows,
});

function renderEvents(dayEvents: DayEvents, patch: Partial<HubView> = {}) {
  const hub: HubView = {
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
    events: dayEvents,
    ...patch,
  };
  render(<Events hub={hub} />);
}

const rowOf = (title: string) => screen.getByText(title).closest("[data-highlighted]")!;
const timelineBlock = (label: string) => screen.getByTitle(label);

describe("Events", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
  });

  describe("heading and empty texts", () => {
    it("is titled Today’s events today", () => {
      renderEvents(events([row({ key: "a", title: "Standup" })]));
      expect(screen.getByRole("heading", { name: "Today’s events" })).toBeInTheDocument();
    });

    it("is titled Events on another day", () => {
      renderEvents(events([row({ key: "a", title: "Standup" })]), { isToday: false });
      expect(screen.getByRole("heading", { name: "Events" })).toBeInTheDocument();
    });

    it.each([
      ["today", true, "No events today."],
      ["another day", false, "No events."],
    ])("says there are no events %s", (_name, isToday, text) => {
      renderEvents(events([]), { isToday });
      expect(screen.getByText(text)).toBeInTheDocument();
    });

    it("lists the time, title and calendar of each event", () => {
      renderEvents(events([row({ key: "a", title: "Standup", time: "09:00–09:30", calendar: "Work" })]));
      expect(screen.getByText("09:00–09:30")).toBeInTheDocument();
      expect(screen.getByText("Standup")).toBeInTheDocument();
      expect(screen.getByText("Work")).toBeInTheDocument();
    });
  });

  describe("highlighting", () => {
    const two = () =>
      renderEvents(
        events(
          [row({ key: "a", title: "Standup" }), row({ key: "b", title: "Review" })],
          [block("a", "09:00–09:30 Standup"), block("b", "10:00–11:00 Review")],
        ),
      );

    it("highlights the timeline block of the event row under the pointer", async () => {
      two();
      await userEvent.hover(rowOf("Standup"));
      expect(timelineBlock("09:00–09:30 Standup")).toHaveAttribute("data-highlighted", "true");
      expect(timelineBlock("10:00–11:00 Review")).toHaveAttribute("data-highlighted", "false");
      expect(rowOf("Standup")).toHaveAttribute("data-highlighted", "true");
    });

    it("highlights the event row of the timeline block under the pointer", async () => {
      two();
      await userEvent.hover(timelineBlock("10:00–11:00 Review"));
      expect(rowOf("Review")).toHaveAttribute("data-highlighted", "true");
      expect(rowOf("Standup")).toHaveAttribute("data-highlighted", "false");
    });

    it("clears the highlight when the pointer leaves", async () => {
      two();
      await userEvent.hover(rowOf("Standup"));
      await userEvent.unhover(rowOf("Standup"));
      expect(timelineBlock("09:00–09:30 Standup")).toHaveAttribute("data-highlighted", "false");
    });
  });

  describe("past events", () => {
    const mixed = () =>
      renderEvents(events([row({ key: "a", title: "Standup", past: true }), row({ key: "b", title: "Review" })]));

    it("hides past events from the list and notes how many", async () => {
      mixed();
      await userEvent.click(screen.getByRole("button", { name: "Hide Past Events" }));
      expect(screen.queryByText("Standup")).not.toBeInTheDocument();
      expect(screen.getByText("Review")).toBeInTheDocument();
      expect(screen.getByText("1 past event hidden")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Show Past Events" })).toHaveAttribute("aria-pressed", "true");
    });

    it("pluralizes the note for several hidden events", () => {
      resetStore({ uiState: { ...useStore.getState().uiState, hidePastEvents: true } });
      renderEvents(events([row({ key: "a", title: "A", past: true }), row({ key: "b", title: "B", past: true }), row({ key: "c", title: "C" })]));
      expect(screen.getByText("2 past events hidden")).toBeInTheDocument();
    });

    it("shows them again with the toggle", async () => {
      mixed();
      await userEvent.click(screen.getByRole("button", { name: "Hide Past Events" }));
      await userEvent.click(screen.getByRole("button", { name: "Show Past Events" }));
      expect(screen.getByText("Standup")).toBeInTheDocument();
      expect(screen.queryByText(/past event/)).not.toBeInTheDocument();
    });

    it("saves the choice in the window layout", async () => {
      mixed();
      await userEvent.click(screen.getByRole("button", { name: "Hide Past Events" }));
      flushUiState();
      expect(mocked.setUiState).toHaveBeenCalledWith(expect.objectContaining({ hidePastEvents: true }));
    });

    it("says there are no more events when all of them are past", async () => {
      renderEvents(events([row({ key: "a", title: "Standup", past: true })]));
      await userEvent.click(screen.getByRole("button", { name: "Hide Past Events" }));
      expect(screen.getByText("No more events today.")).toBeInTheDocument();
      expect(screen.queryByText(/past event.* hidden/)).not.toBeInTheDocument();
    });

    it("has no toggle on another day", () => {
      renderEvents(events([row({ key: "a", title: "Standup" })]), { isToday: false });
      expect(screen.queryByRole("button", { name: /Past Events/ })).not.toBeInTheDocument();
    });

    it("has no toggle without events", () => {
      renderEvents(events([]));
      expect(screen.queryByRole("button", { name: /Past Events/ })).not.toBeInTheDocument();
    });

    it("keeps past events on another day even when the choice is to hide them", () => {
      resetStore({ uiState: { ...useStore.getState().uiState, hidePastEvents: true } });
      renderEvents(events([row({ key: "a", title: "Standup", past: true })]), { isToday: false });
      expect(screen.getByText("Standup")).toBeInTheDocument();
    });
  });

  describe("meetings", () => {
    it("joins the call of a meeting row when it is clicked", async () => {
      renderEvents(events([row({ key: "a", title: "Standup", meeting: { app: "Zoom", url: "https://zoom.us/j/1" } })]));
      await userEvent.click(screen.getByTitle("Join the Zoom meeting"));
      expect(mocked.calendarJoin).toHaveBeenCalledWith("https://zoom.us/j/1");
    });

    it("names the call app on the row", () => {
      renderEvents(events([row({ key: "a", title: "Standup", meeting: { app: "Teams", url: "https://teams.example/1" } })]));
      expect(screen.getByTitle("Join the Teams meeting")).toHaveTextContent("Teams");
    });

    it("shows an error toast when joining fails", async () => {
      mocked.calendarJoin.mockRejectedValueOnce({ kind: "io", message: "No app", code: null });
      renderEvents(events([row({ key: "a", title: "Standup", meeting: { app: "Zoom", url: "https://zoom.us/j/1" } })]));
      await userEvent.click(screen.getByTitle("Join the Zoom meeting"));
      await vi.waitFor(() => expect(useStore.getState().toasts[0]?.message).toContain("Couldn't join the meeting"));
    });

    it("renders an event without a call as plain text", () => {
      renderEvents(events([row({ key: "a", title: "Lunch" })]));
      expect(screen.queryByTitle(/Join the/)).not.toBeInTheDocument();
    });
  });

  describe("timeline", () => {
    it("is described as the day’s timeline", () => {
      renderEvents(events([]));
      expect(screen.getByRole("img", { name: "The day’s timeline" })).toBeInTheDocument();
    });

    it("draws the now line only when now is within the shown hours", () => {
      renderEvents(events([], [], 0.4));
      const line = screen.getByRole("img", { name: "The day’s timeline" }).querySelector(".timeline-now");
      expect(line).toHaveStyle({ left: "40%" });
    });

    it("draws no now line when now is null", () => {
      renderEvents(events([], [], null));
      expect(screen.getByRole("img", { name: "The day’s timeline" }).querySelector(".timeline-now")).toBeNull();
    });
  });
});
