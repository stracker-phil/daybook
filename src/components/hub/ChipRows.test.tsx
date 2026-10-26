import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type HubView, type JournalProgress, type RoutineProgress } from "../../api";
import { JournalRow, RoutinesRow } from "./ChipRows";

const mocked = vi.mocked(api, true);

const group = (patch: Partial<JournalProgress> = {}): JournalProgress => ({ title: "Dawn", answered: 0, total: 3, hasNote: false, ...patch });

async function renderRow(journal: JournalProgress[], patch: Partial<HubView> = {}) {
  const hub: HubView = { ...(await api.hubView()), journal, ...patch };
  render(<JournalRow hub={hub} />);
}

const journalTool = (shortcut: string | null): HubView["tools"] => [
  { kind: "tool", id: "journal", title: "Journal", command: "journal:open", shortcut, detail: null, accent: false, enabled: true },
];

const routine = (patch: Partial<RoutineProgress> = {}): RoutineProgress => ({ id: "morning", name: "Morning", done: 0, total: 4, ...patch });

async function renderRoutines(routines: RoutineProgress[], patch: Partial<HubView> = {}) {
  const hub: HubView = { ...(await api.hubView()), routines, ...patch };
  render(<RoutinesRow hub={hub} />);
}

describe("RoutinesRow", () => {
  beforeEach(() => vi.clearAllMocks());

  it("renders nothing without routines", async () => {
    await renderRoutines([]);
    expect(screen.queryByRole("region", { name: "Routines" })).not.toBeInTheDocument();
  });

  it("lists one button per routine in a Routines section", async () => {
    await renderRoutines([routine(), routine({ id: "evening", name: "Evening" })]);
    expect(screen.getByRole("region", { name: "Routines" })).toBeInTheDocument();
    expect(screen.getAllByRole("button").map((b) => b.textContent)).toEqual(["Morning", "Evening"]);
  });

  it("shows one dot per item labelled with the done count", async () => {
    await renderRoutines([routine({ done: 1, total: 4 })]);
    expect(screen.getByLabelText("1 of 4").children).toHaveLength(4);
    expect(screen.queryByLabelText("Done")).not.toBeInTheDocument();
  });

  it("shows a check once every item is done", async () => {
    await renderRoutines([routine({ done: 4, total: 4 })]);
    expect(screen.getByLabelText("Done")).toBeInTheDocument();
    expect(screen.queryByLabelText("4 of 4")).not.toBeInTheDocument();
  });

  it("runs the routine's command when its chip is clicked today", async () => {
    await renderRoutines([routine(), routine({ id: "evening", name: "Evening" })]);
    await userEvent.click(screen.getByRole("button", { name: /^Evening/ }));
    expect(mocked.runCommand).toHaveBeenCalledWith("routine:evening");
  });

  it("disables the chips on another day", async () => {
    await renderRoutines([routine()], { isToday: false });
    const chip = screen.getByRole("button", { name: /^Morning/ });
    expect(chip).toBeDisabled();
    await userEvent.click(chip);
    expect(mocked.runCommand).not.toHaveBeenCalled();
  });
});

describe("JournalRow", () => {
  beforeEach(() => vi.clearAllMocks());

  it("renders nothing without journal groups", async () => {
    await renderRow([]);
    expect(screen.queryByRole("region", { name: "Journal" })).not.toBeInTheDocument();
  });

  it("lists one button per group in a Journal section", async () => {
    await renderRow([group({ title: "Dawn" }), group({ title: "Dusk" })]);
    const section = screen.getByRole("region", { name: "Journal" });
    expect(section).toHaveTextContent("Journal");
    expect(screen.getAllByRole("button").map((b) => b.textContent)).toEqual(["Dawn", "Dusk"]);
  });

  describe("progress", () => {
    it("shows one dot per question labelled with the answered count", async () => {
      await renderRow([group({ answered: 1, total: 3 })]);
      const dots = screen.getByLabelText("1 of 3");
      expect(dots.children).toHaveLength(3);
      expect(screen.queryByLabelText("Done")).not.toBeInTheDocument();
    });

    it("shows a check once every question is answered", async () => {
      await renderRow([group({ answered: 3, total: 3 })]);
      expect(screen.getByLabelText("Done")).toBeInTheDocument();
      expect(screen.queryByLabelText("3 of 3")).not.toBeInTheDocument();
    });

    it("shows a check for a free-text group with a note", async () => {
      await renderRow([group({ title: "Health", answered: 0, total: 0, hasNote: true })]);
      expect(screen.getByLabelText("Done")).toBeInTheDocument();
    });

    it("shows nothing for a free-text group without a note", async () => {
      await renderRow([group({ title: "Health", answered: 0, total: 0, hasNote: false })]);
      expect(screen.queryByLabelText("Done")).not.toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Health" })).toBeInTheDocument();
    });
  });

  describe("clicking", () => {
    it("opens the journal on that group today", async () => {
      await renderRow([group({ title: "Dawn" }), group({ title: "Dusk" })]);
      await userEvent.click(screen.getByRole("button", { name: /^Dusk/ }));
      expect(mocked.runCommand).toHaveBeenCalledWith("journal:open:Dusk");
    });

    it("disables the chips on another day", async () => {
      await renderRow([group({ title: "Dawn" })], { isToday: false });
      const chip = screen.getByRole("button", { name: /^Dawn/ });
      expect(chip).toBeDisabled();
      await userEvent.click(chip);
      expect(mocked.runCommand).not.toHaveBeenCalled();
    });
  });

  describe("shortcut", () => {
    it("shows the journal's global shortcut as a key cap in the title", async () => {
      await renderRow([group()], { tools: journalTool("ctrl+alt+cmd+n") });
      expect(screen.getByRole("heading", { name: /Journal/ }).querySelector("kbd")).toHaveTextContent("⌃⌥⌘N");
    });

    it("shows no key cap when the tool has no shortcut", async () => {
      await renderRow([group()], { tools: journalTool(null) });
      expect(screen.getByRole("heading", { name: /Journal/ }).querySelector("kbd")).toBeNull();
    });

    it("shows no key cap without a journal tool", async () => {
      await renderRow([group()], { tools: [] });
      expect(screen.getByRole("heading", { name: /Journal/ }).querySelector("kbd")).toBeNull();
    });
  });
});
