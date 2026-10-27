import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type RoutineStepView } from "../../api";
import { hintKeys } from "../../test/panelHints";
import { settings } from "../../test/settings";
import { WizardView, linkLabel } from "./WizardView";

const mocked = vi.mocked(api, true);

const step = (index: number, title: string, patch: Partial<RoutineStepView> = {}): RoutineStepView => ({ index, title, link: "", done: false, ...patch });

async function renderWizard(steps: RoutineStepView[], name = "Morning") {
  mocked.routineViewToday.mockResolvedValue({ id: "morning", name, steps });
  const view = render(<WizardView routine="morning" />);
  await screen.findByText(name);
  if (steps.length > 0) await vi.waitFor(() => expect(screen.getByRole("listbox", { name })).toHaveFocus());
  return view;
}

const option = (title: string) => screen.getByRole("option", { name: new RegExp(title) });
const isSelected = (title: string) => option(title).getAttribute("aria-selected") === "true";
const isTicked = (title: string) => option(title).getAttribute("aria-checked") === "true";
const press = (key: string, init: KeyboardEventInit = {}) => fireEvent.keyDown(window, { key, ...init });

describe("WizardView", () => {
  beforeEach(() => vi.clearAllMocks());

  describe("rendering", () => {
    it("loads today's view of the routine and shows its name and items", async () => {
      await renderWizard([step(0, "Plan"), step(1, "Review")]);
      expect(mocked.routineViewToday).toHaveBeenCalledWith("morning");
      expect(screen.getByRole("listbox", { name: "Morning" })).toBeInTheDocument();
      expect(screen.getAllByRole("option")).toHaveLength(2);
    });

    it("shows how many items are done", async () => {
      await renderWizard([step(0, "A", { done: true }), step(1, "B"), step(2, "C")]);
      expect(screen.getByText("1 of 3")).toBeInTheDocument();
    });

    it("shows All done when every item is ticked", async () => {
      await renderWizard([step(0, "A", { done: true }), step(1, "B", { done: true })]);
      expect(screen.getByText("All done")).toBeInTheDocument();
      expect(screen.queryByText("2 of 2")).not.toBeInTheDocument();
    });

    it("says nothing is due when the routine has no items today", async () => {
      await renderWizard([]);
      expect(screen.getByText("Nothing due today.")).toBeInTheDocument();
      expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    });

    it("shows the message when the routine cannot be read", async () => {
      mocked.routineViewToday.mockRejectedValueOnce({ kind: "io", message: "Note is unreadable", code: null });
      render(<WizardView routine="morning" />);
      expect(await screen.findByText("Note is unreadable")).toBeInTheDocument();
    });

    it("marks ticked items as checked", async () => {
      await renderWizard([step(0, "A", { done: true }), step(1, "B")]);
      expect(isTicked("A")).toBe(true);
      expect(isTicked("B")).toBe(false);
    });
  });

  describe("selection", () => {
    it("starts on the first item that is not ticked", async () => {
      await renderWizard([step(0, "A", { done: true }), step(1, "B"), step(2, "C")]);
      expect(isSelected("B")).toBe(true);
      expect(isSelected("A")).toBe(false);
    });

    it("starts on the first item when all are ticked", async () => {
      await renderWizard([step(0, "A", { done: true }), step(1, "B", { done: true })]);
      expect(isSelected("A")).toBe(true);
    });

    it("moves down and up through all items, ticked ones included, wrapping at both ends", async () => {
      await renderWizard([step(0, "A", { done: true }), step(1, "B"), step(2, "C", { done: true })]);
      press("ArrowDown");
      expect(isSelected("C")).toBe(true);
      press("ArrowDown");
      expect(isSelected("A")).toBe(true);
      press("ArrowUp");
      expect(isSelected("C")).toBe(true);
      press("ArrowUp");
      press("ArrowUp");
      expect(isSelected("A")).toBe(true);
    });
  });

  describe("ticking", () => {
    it.each([" ", "Enter"])("ticks the selected item on the key %j with its own index and shows it ticked at once", async (key) => {
      await renderWizard([step(0, "Plan"), step(2, "Review")]);
      press("ArrowDown");
      press(key);
      expect(mocked.routineStep).toHaveBeenCalledWith("morning", 2, true);
      expect(isTicked("Review")).toBe(true);
      expect(isTicked("Plan")).toBe(false);
    });

    it("unticks a ticked item", async () => {
      await renderWizard([step(0, "Plan", { done: true }), step(1, "Review")]);
      press("ArrowUp");
      press(" ");
      expect(mocked.routineStep).toHaveBeenCalledWith("morning", 0, false);
      expect(isTicked("Plan")).toBe(false);
    });

    it("ticks an item when its row is clicked and selects it", async () => {
      await renderWizard([step(0, "Plan"), step(2, "Review")]);
      await userEvent.click(option("Review"));
      expect(mocked.routineStep).toHaveBeenCalledWith("morning", 2, true);
      expect(isTicked("Review")).toBe(true);
      expect(isSelected("Review")).toBe(true);
    });

    it("updates the count as items are ticked", async () => {
      await renderWizard([step(0, "A"), step(1, "B")]);
      press(" ");
      expect(screen.getByText("1 of 2")).toBeInTheDocument();
    });

    it("shows a failed tick in the footer and restores the item from the note", async () => {
      await renderWizard([step(0, "Plan")]);
      mocked.routineStep.mockRejectedValueOnce({ kind: "io", message: "Disk full", code: null });
      press(" ");
      expect(await screen.findByText(/Couldn't update today's note: Disk full/)).toBeInTheDocument();
      await vi.waitFor(() => expect(isTicked("Plan")).toBe(false));
      expect(hintKeys("Close")).toEqual(["esc"]);
    });
  });

  describe("links", () => {
    const linked = [step(0, "Time", { link: "https://time.example.com/x" }), step(1, "Log", { link: "3-Personal/Work-Log.md" }), step(2, "Plain")];

    it("labels the link button with the host of a URL and the note name of a vault path", async () => {
      await renderWizard(linked);
      expect(screen.getByRole("button", { name: "Open time.example.com" })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Open Work-Log" })).toBeInTheDocument();
    });

    it("opens the selected item's link on ⌘O", async () => {
      await renderWizard(linked);
      press("o", { metaKey: true });
      expect(mocked.routineOpenLink).toHaveBeenCalledWith("https://time.example.com/x");
      press("ArrowDown");
      press("o", { metaKey: true });
      expect(mocked.routineOpenLink).toHaveBeenLastCalledWith("3-Personal/Work-Log.md");
    });

    it("does nothing on ⌘O for an item without a link, or on O alone", async () => {
      await renderWizard(linked);
      press("o");
      press("ArrowUp");
      press("o", { metaKey: true });
      expect(mocked.routineOpenLink).not.toHaveBeenCalled();
    });

    it("opens the link of the clicked button without ticking the row", async () => {
      await renderWizard(linked);
      await userEvent.click(screen.getByRole("button", { name: "Open Work-Log" }));
      expect(mocked.routineOpenLink).toHaveBeenCalledWith("3-Personal/Work-Log.md");
      expect(mocked.routineStep).not.toHaveBeenCalled();
      expect(isTicked("Log")).toBe(false);
    });

    it("shows a failed link in the footer", async () => {
      mocked.routineOpenLink.mockRejectedValueOnce({ kind: "io", message: "No app for this link", code: null });
      await renderWizard(linked);
      press("o", { metaKey: true });
      expect(await screen.findByText("No app for this link")).toBeInTheDocument();
    });

    it("lists ⌘O open in the footer only while the selected item has a link", async () => {
      await renderWizard(linked);
      expect(hintKeys("Open")).toEqual(["⌘", "O"]);
      press("ArrowUp");
      expect(hintKeys("Open")).toBeNull();
      expect(hintKeys("Close")).toEqual(["esc"]);
    });
  });

  describe("closing", () => {
    beforeEach(() => vi.useFakeTimers({ shouldAdvanceTime: true }));
    afterEach(() => vi.useRealTimers());

    it("closes the overlay shortly after the last open item is ticked", async () => {
      await renderWizard([step(0, "A", { done: true }), step(1, "B")]);
      press(" ");
      expect(await screen.findByText("All done")).toBeInTheDocument();
      await vi.advanceTimersByTimeAsync(300);
      expect(mocked.panelClose).not.toHaveBeenCalled();
      await vi.advanceTimersByTimeAsync(500);
      expect(mocked.panelClose).toHaveBeenCalled();
    });

    it("stays open while an item is still open", async () => {
      await renderWizard([step(0, "A"), step(1, "B")]);
      press(" ");
      await vi.advanceTimersByTimeAsync(2000);
      expect(mocked.panelClose).not.toHaveBeenCalled();
    });

    it("does not close a routine that was already done when opened", async () => {
      await renderWizard([step(0, "A", { done: true }), step(1, "B", { done: true })]);
      await vi.advanceTimersByTimeAsync(2000);
      expect(mocked.panelClose).not.toHaveBeenCalled();
    });

    it("does not close when the last item is unticked again in time", async () => {
      await renderWizard([step(0, "A")]);
      press(" ");
      await vi.advanceTimersByTimeAsync(300);
      press(" ");
      await vi.advanceTimersByTimeAsync(2000);
      expect(mocked.panelClose).not.toHaveBeenCalled();
    });
  });

  describe("re-reading", () => {
    async function changed(steps: RoutineStepView[], listener: "onNoteChanged" | "onSettingsChanged") {
      mocked.routineViewToday.mockResolvedValue({ id: "morning", name: "Morning", steps });
      const handler = mocked[listener].mock.calls[0][0] as (payload: unknown) => void;
      await act(async () => handler(listener === "onNoteChanged" ? { day: null } : settings()));
    }

    it("shows ticks made in Obsidian when the note changes", async () => {
      await renderWizard([step(0, "A"), step(1, "B")]);
      await changed([step(0, "A", { done: true }), step(1, "B")], "onNoteChanged");
      expect(screen.getByText("1 of 2")).toBeInTheDocument();
      expect(isTicked("A")).toBe(true);
    });

    it("shows the new items when Settings change", async () => {
      await renderWizard([step(0, "A")]);
      await changed([step(0, "A"), step(1, "Added")], "onSettingsChanged");
      expect(screen.getByRole("option", { name: /Added/ })).toBeInTheDocument();
    });
  });
});

describe("linkLabel", () => {
  it.each([
    ["https://time.example.com/x", "time.example.com"],
    ["https://www.example.com/a/b", "example.com"],
    ["http://localhost:3000", "localhost"],
    ["3-Personal/Work-Log.md", "Work-Log"],
    ["Inbox.md", "Inbox"],
    ["3-Personal/Folder", "Folder"],
  ])("labels %s as %s", (link, label) => {
    expect(linkLabel(link)).toBe(label);
  });
});
