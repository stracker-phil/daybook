import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Routine, Settings } from "../../api";
import { settings } from "../../test/settings";
import { RoutinesPage } from "./RoutinesPage";

const set = vi.fn<(patch: Partial<Settings>) => void>();

function renderPage(patch: Partial<Settings> = {}) {
  render(<RoutinesPage s={settings(patch)} set={set} />);
}

const card = (title: string) => within(screen.getByRole("heading", { name: title }).closest("section")!);
const morning = () => card("Morning");
const evening = () => card("Evening");

const saved = (): Routine[] => set.mock.lastCall![0].routines!;
const savedRoutine = (id: string) => saved().find((r) => r.id === id)!;
const last = (within_: ReturnType<typeof card>, name: string) => within_.getAllByRole("button", { name }).at(-1)!;

describe("RoutinesPage", () => {
  beforeEach(() => vi.clearAllMocks());

  describe("cards", () => {
    it("shows one card per routine titled by its name, in order", () => {
      renderPage();
      expect(screen.getAllByRole("heading", { level: 3 }).map((h) => h.textContent)).toEqual(["Morning", "Evening"]);
    });

    it("fills a card with the routine's name and items", () => {
      renderPage();
      expect(morning().getByRole("textbox", { name: "Name" })).toHaveValue("Morning");
      expect(morning().getAllByRole("textbox", { name: "Item" })).toHaveLength(4);
      expect(morning().getAllByRole("textbox", { name: "Item" })[0]).toHaveValue("Plan day (Slack)");
      expect(evening().getAllByRole("textbox", { name: "Link" })[0]).toHaveValue("https://time.example.com");
    });

    it("shows only the add buttons without routines", () => {
      renderPage({ routines: [] });
      expect(screen.queryByRole("heading", { level: 3 })).not.toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Add Routine" })).toBeInTheDocument();
    });
  });

  describe("routine fields", () => {
    it("saves a renamed routine on Enter, trimmed, keeping its id and leaving the other routine alone", async () => {
      renderPage();
      const name = morning().getByRole("textbox", { name: "Name" });
      await userEvent.clear(name);
      await userEvent.type(name, " Start {Enter}");
      expect(saved().map((r) => [r.id, r.name])).toEqual([
        ["morning", "Start"],
        ["evening", "Evening"],
      ]);
    });

    it("keeps the name when it is emptied", async () => {
      renderPage();
      await userEvent.clear(morning().getByRole("textbox", { name: "Name" }));
      await userEvent.tab();
      expect(set).not.toHaveBeenCalled();
    });
  });

  describe("contexts", () => {
    it("lists the routine's contexts by their label and name", () => {
      renderPage();
      expect(within(morning().getByRole("list", { name: "Contexts" })).getByText("[W] Work")).toBeInTheDocument();
    });

    it("adds a context found by typing part of its name", async () => {
      renderPage();
      await userEvent.type(morning().getByRole("textbox", { name: "Add to Contexts" }), "priv{Enter}");
      expect(savedRoutine("morning").contexts).toEqual(["work", "private"]);
    });

    it("removes a context", async () => {
      renderPage();
      await userEvent.click(morning().getByRole("button", { name: "Remove [W] Work" }));
      expect(savedRoutine("morning").contexts).toEqual([]);
    });

    it("hides the chooser when there is only one context", () => {
      const [work] = settings().contexts;
      renderPage({ contexts: [work] });
      expect(screen.queryByRole("textbox", { name: "Add to Contexts" })).not.toBeInTheDocument();
    });
  });

  describe("items", () => {
    it("appends a New item without link or weekdays", async () => {
      renderPage();
      await userEvent.click(evening().getByRole("button", { name: "Add Item" }));
      expect(savedRoutine("evening").steps.at(-1)).toEqual({ title: "New item", link: "", days: [] });
      expect(savedRoutine("evening").steps).toHaveLength(3);
    });

    it("saves an edited item title, trimmed, on Enter", async () => {
      renderPage();
      const title = evening().getAllByRole("textbox", { name: "Item" })[1];
      await userEvent.clear(title);
      await userEvent.type(title, " Brag doc {Enter}");
      expect(savedRoutine("evening").steps.map((st) => st.title)).toEqual(["Time recording", "Brag doc"]);
    });

    it("keeps the item when its title is emptied", async () => {
      renderPage();
      await userEvent.clear(evening().getAllByRole("textbox", { name: "Item" })[0]);
      await userEvent.tab();
      expect(set).not.toHaveBeenCalled();
    });

    it("saves an edited link on blur", async () => {
      renderPage();
      const link = evening().getAllByRole("textbox", { name: "Link" })[1];
      await userEvent.clear(link);
      await userEvent.type(link, "3-Personal/Log.md");
      await userEvent.tab();
      expect(savedRoutine("evening").steps[1]).toEqual(expect.objectContaining({ title: "Brag doc: anything worth logging today?", link: "3-Personal/Log.md" }));
    });

    it("saves an emptied link", async () => {
      renderPage();
      await userEvent.clear(evening().getAllByRole("textbox", { name: "Link" })[0]);
      await userEvent.tab();
      expect(savedRoutine("evening").steps[0].link).toBe("");
    });

    it("saves the weekdays an item is due on", async () => {
      renderPage();
      const days = within(evening().getByRole("group", { name: "Brag doc: anything worth logging today?: due on" }));
      await userEvent.click(days.getByRole("button", { name: "Fr" }));
      expect(savedRoutine("evening").steps[1].days).toEqual([0, 1, 2, 3, 4, 6]);
    });

    it("removes an item", async () => {
      renderPage();
      await userEvent.click(evening().getAllByRole("button", { name: "Remove Item" })[0]);
      expect(savedRoutine("evening").steps.map((st) => st.title)).toEqual(["Brag doc: anything worth logging today?"]);
    });

    it("moves an item down", async () => {
      renderPage();
      await userEvent.click(evening().getAllByRole("button", { name: "Move Down" })[0]);
      expect(savedRoutine("evening").steps.map((st) => st.title)).toEqual(["Brag doc: anything worth logging today?", "Time recording"]);
    });

    it("disables Move Up on the first item and Move Down on the last", () => {
      renderPage();
      expect(evening().getAllByRole("button", { name: "Move Up" })[0]).toBeDisabled();
      expect(evening().getAllByRole("button", { name: "Move Down" })[1]).toBeDisabled();
    });
  });

  describe("list actions", () => {
    it("appends a New routine without items, contexts or timing", async () => {
      renderPage();
      await userEvent.click(screen.getByRole("button", { name: "Add Routine" }));
      expect(saved()).toHaveLength(3);
      expect(saved()[2]).toEqual({ id: "", name: "New routine", contexts: [], steps: [], timing: { due: null, openWhenDue: false, openOn: [] } });
    });

    it("removes a routine", async () => {
      renderPage();
      await userEvent.click(last(evening(), "Remove Routine"));
      expect(saved().map((r) => r.id)).toEqual(["morning"]);
    });

    it("moves a routine up", async () => {
      renderPage();
      await userEvent.click(last(evening(), "Move Up"));
      expect(saved().map((r) => r.id)).toEqual(["evening", "morning"]);
    });

    it("moves a routine down", async () => {
      renderPage();
      await userEvent.click(last(morning(), "Move Down"));
      expect(saved().map((r) => r.id)).toEqual(["evening", "morning"]);
    });

    it("disables Move Up on the first routine and Move Down on the last", () => {
      renderPage();
      expect(last(morning(), "Move Up")).toBeDisabled();
      expect(last(evening(), "Move Down")).toBeDisabled();
    });
  });

  describe("timing", () => {
    it("adds an opening event to the existing ones", async () => {
      renderPage();
      await userEvent.click(morning().getByRole("checkbox", { name: "On “Done for today”" }));
      expect(savedRoutine("morning").timing.openOn).toEqual(["start", "off"]);
    });

    it("removes an opening event", async () => {
      renderPage();
      await userEvent.click(evening().getByRole("checkbox", { name: "On “Done for today”" }));
      expect(savedRoutine("evening").timing.openOn).toEqual([]);
    });

    it("names the context switch as when switching to one of its contexts", () => {
      renderPage();
      expect(morning().getByRole("checkbox", { name: "When switching to one of its contexts" })).not.toBeChecked();
    });

    it("clears the due time and opening when Due is switched off", async () => {
      renderPage();
      await userEvent.click(morning().getByRole("switch", { name: "Due" }));
      expect(savedRoutine("morning").timing).toEqual({ due: null, openWhenDue: false, openOn: ["start"] });
    });
  });
});
