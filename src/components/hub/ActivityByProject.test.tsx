import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { DayActivityProject, DayActivityRecording } from "../../api";
import { resetStore, useStore } from "../../state/store";
import { ActivityByProject } from "./ActivityByProject";

const recording = (patch: Partial<DayActivityRecording> = {}): DayActivityRecording => ({
  recordingId: "r1",
  note: "Fix sync",
  tag: null,
  spans: "09:00–10:25",
  duration: "1h 25m",
  durationMs: 5_100_000,
  running: false,
  ...patch,
});

const project = (patch: Partial<DayActivityProject> = {}): DayActivityProject => ({
  projectId: "p1",
  project: "Apollo",
  client: "Acme",
  color: "blue",
  shade: 4,
  total: "2h 00m",
  totalMs: 7_200_000,
  recordings: [recording()],
  ...patch,
});

describe("ActivityByProject", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
  });

  it("shows a section per project with its name, client and total", () => {
    render(<ActivityByProject projects={[project(), project({ projectId: "p2", project: "Home", client: null, total: "30m", recordings: [] })]} />);
    const apollo = screen.getByRole("region", { name: "Apollo" });
    expect(within(apollo).getByText("Acme")).toBeInTheDocument();
    expect(within(apollo).getByTitle("Copy 2h 00m")).toBeInTheDocument();
    const home = screen.getByRole("region", { name: "Home" });
    expect(within(home).getByTitle("Copy 30m")).toBeInTheDocument();
    expect(within(home).queryByText("Acme")).not.toBeInTheDocument();
  });

  it("lists a recording with its note, tag, spans and duration", () => {
    render(<ActivityByProject projects={[project({ recordings: [recording({ tag: "Review" })] })]} />);
    const section = screen.getByRole("region", { name: "Apollo" });
    expect(within(section).getByRole("button", { name: "Fix sync" })).toBeInTheDocument();
    expect(within(section).getByText("Review · 09:00–10:25")).toBeInTheDocument();
    expect(within(section).getByTitle("Copy 1h 25m")).toBeInTheDocument();
  });

  it("shows only the spans when a recording has no tag", () => {
    render(<ActivityByProject projects={[project()]} />);
    expect(screen.getByText("09:00–10:25")).toBeInTheDocument();
  });

  it("says No description for a recording without a note", () => {
    render(<ActivityByProject projects={[project({ recordings: [recording({ note: "" })] })]} />);
    expect(screen.getByRole("button", { name: "No description" })).toBeInTheDocument();
  });

  it("copies the note of a recording and confirms with a toast", async () => {
    const user = userEvent.setup();
    render(<ActivityByProject projects={[project()]} />);
    await user.click(screen.getByRole("button", { name: "Fix sync" }));
    expect(await navigator.clipboard.readText()).toBe("Fix sync");
    expect(useStore.getState().toasts.map((t) => t.message)).toContain("Copied the note");
  });

  it("copies the project name for a recording without a note", async () => {
    const user = userEvent.setup();
    render(<ActivityByProject projects={[project({ recordings: [recording({ note: "" })] })]} />);
    await user.click(screen.getByRole("button", { name: "No description" }));
    expect(await navigator.clipboard.readText()).toBe("Apollo");
  });

  it("copies a project total", async () => {
    const user = userEvent.setup();
    render(<ActivityByProject projects={[project()]} />);
    await user.click(screen.getByTitle("Copy 2h 00m"));
    expect(await navigator.clipboard.readText()).toBe("2h 00m");
    expect(useStore.getState().toasts.map((t) => t.message)).toContain("Copied 2h 00m");
  });
});
