import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { DayActivityRow } from "../../api";
import { resetStore, useStore } from "../../state/store";
import { ActivityTimeline } from "./ActivityTimeline";

const row = (patch: Partial<DayActivityRow> = {}): DayActivityRow => ({
  kind: "work",
  start: "09:00",
  end: "10:25",
  duration: "1h 25m",
  durationMs: 5_100_000,
  title: "Apollo · Fix sync",
  project: "Apollo",
  client: null,
  note: "Fix sync",
  tag: null,
  color: "blue",
  shade: 4,
  suggestedBy: null,
  copyText: "Fix sync",
  recordingId: "r1",
  ...patch,
});

const toastMessages = () => useStore.getState().toasts.map((t) => t.message);

describe("ActivityTimeline", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
  });

  it("lists each span under its start time with project, note and duration, closed by the end label", () => {
    render(<ActivityTimeline rows={[row(), row({ start: "11:00", project: "Home", note: "Tidy", copyText: "Tidy", duration: "30m", recordingId: "r2" })]} end="11:30" />);
    const list = screen.getByRole("list", { name: "Timeline" });
    expect(within(list).getByText("09:00")).toBeInTheDocument();
    expect(within(list).getByRole("button", { name: /Apollo · Fix sync/ })).toBeInTheDocument();
    expect(within(list).getByTitle("Copy 1h 25m")).toBeInTheDocument();
    expect(within(list).getByRole("button", { name: /Home · Tidy/ })).toBeInTheDocument();
    expect(within(list).getByText("11:30")).toBeInTheDocument();
  });

  it("shows no closing time without an end label", () => {
    render(<ActivityTimeline rows={[row()]} end={null} />);
    expect(screen.getAllByRole("listitem")).toHaveLength(1);
  });

  it("shows the title of a gap as quiet text that cannot be copied", () => {
    render(<ActivityTimeline rows={[row({ kind: "gap", title: "Nothing recorded", project: null, note: "", copyText: null, color: null, recordingId: null })]} end={null} />);
    expect(screen.getByText("Nothing recorded")).toBeInTheDocument();
    expect(screen.queryByTitle("Copy the note")).not.toBeInTheDocument();
  });

  it("shows who suggested a suggestion", () => {
    render(<ActivityTimeline rows={[row({ kind: "suggestion", suggestedBy: "Suggested by Slack: Oscorp" })]} end={null} />);
    expect(screen.getByText("Suggested by Slack: Oscorp")).toBeInTheDocument();
  });

  it("shows the title when a row has no project", () => {
    render(<ActivityTimeline rows={[row({ project: null, title: "Unlisted work", note: "", copyText: "Unlisted work" })]} end={null} />);
    expect(screen.getByRole("button", { name: "Unlisted work" })).toBeInTheDocument();
  });

  describe("copying", () => {
    it("copies a duration and confirms with a toast", async () => {
      const user = userEvent.setup();
      render(<ActivityTimeline rows={[row()]} end={null} />);
      await user.click(screen.getByTitle("Copy 1h 25m"));
      expect(await navigator.clipboard.readText()).toBe("1h 25m");
      expect(toastMessages()).toContain("Copied 1h 25m");
    });

    it("copies the copy text of a row when its note is clicked", async () => {
      const user = userEvent.setup();
      render(<ActivityTimeline rows={[row({ copyText: "Fix sync (QRS-1)" })]} end={null} />);
      await user.click(screen.getByTitle("Copy the note"));
      expect(await navigator.clipboard.readText()).toBe("Fix sync (QRS-1)");
      expect(toastMessages()).toEqual(["Copied the note"]);
    });

    it.each(["work", "running"] as const)("copies the note of a %s row when its time is clicked", async (kind) => {
      const user = userEvent.setup();
      render(<ActivityTimeline rows={[row({ kind, copyText: "The note" })]} end={null} />);
      await user.click(screen.getByText("09:00"));
      expect(await navigator.clipboard.readText()).toBe("The note");
      expect(toastMessages()).toEqual(["Copied the note"]);
    });

    it.each([
      ["gap", { kind: "gap", copyText: null, project: null, title: "Nothing recorded" }],
      ["suggestion", { kind: "suggestion", suggestedBy: "Suggested by Slack: Oscorp" }],
    ] as const)("copies nothing when the time of a %s row is clicked", async (_name, patch) => {
      const user = userEvent.setup();
      render(<ActivityTimeline rows={[row(patch)]} end={null} />);
      await user.click(screen.getByText("09:00"));
      expect(toastMessages()).toEqual([]);
    });

    it("copies once when the note button inside a work row is clicked", async () => {
      const user = userEvent.setup();
      render(<ActivityTimeline rows={[row()]} end={null} />);
      await user.click(screen.getByTitle("Copy the note"));
      expect(useStore.getState().toasts).toMatchObject([{ message: "Copied the note", count: 1 }]);
    });
  });
});
