import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type Choice, type RailPoint, type RailTipContent } from "../../api";
import { RailTipApp } from "./RailTipApp";

const mocked = vi.mocked(api, true);

function content(patch: Partial<RailTipContent> = {}): RailTipContent {
  return {
    tip: { label: "Quick Notes", detail: null, shortcut: null },
    side: "right",
    theme: "system",
    highlight: "tinted",
    sequence: 1,
    choices: [],
    anchor: 100,
    inset: 8,
    maxWidth: 300,
    height: 800,
    ...patch,
  };
}

function item(label: string, patch: Partial<Extract<Choice, { kind: "item" }>> = {}): Choice {
  return { kind: "item", label, command: `do:${label}`, detail: null, checked: null, ...patch };
}

/** Delivers an event the way Rust emits it: the handler the page subscribed with. */
function emit<T>(subscription: { mock: { calls: unknown[][] } }, payload: T) {
  const handler = subscription.mock.calls.at(-1)![0] as (payload: T) => void;
  act(() => handler(payload));
}

async function renderTip() {
  render(<RailTipApp />);
  await vi.waitFor(() => {
    expect(mocked.onRailTipContent).toHaveBeenCalled();
    expect(mocked.onRailTipPlaced).toHaveBeenCalled();
    expect(mocked.onRailTipHidden).toHaveBeenCalled();
  });
}

const showTip = (patch: Partial<RailTipContent> = {}) => emit(mocked.onRailTipContent, content(patch));
const phase = () => screen.getByRole("tooltip").getAttribute("data-phase");

describe("RailTipApp", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    document.elementFromPoint = vi.fn();
  });
  afterEach(() => Reflect.deleteProperty(document, "elementFromPoint"));

  it("shows nothing until Rust sends a tip", async () => {
    await renderTip();
    expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
  });

  describe("the card", () => {
    it("shows the label and the detail", async () => {
      await renderTip();
      showTip({ tip: { label: "Search", detail: "Find notes in the vault", shortcut: null } });
      const tip = screen.getByRole("tooltip");
      expect(within(tip).getByText("Search")).toBeInTheDocument();
      expect(within(tip).getByText("Find notes in the vault")).toBeInTheDocument();
    });

    it("draws one key cap per key of the shortcut", async () => {
      await renderTip();
      showTip({ tip: { label: "Quick Notes", detail: null, shortcut: "ctrl+cmd+m" } });
      const tip = screen.getByRole("tooltip");
      expect(["⌃", "⌘", "M"].map((key) => within(tip).getByText(key).tagName)).toEqual(["KBD", "KBD", "KBD"]);
    });

    it("draws no key caps without a shortcut", async () => {
      await renderTip();
      showTip();
      expect(within(screen.getByRole("tooltip")).queryByText("⌘")).not.toBeInTheDocument();
    });

    it("has no menu without choices", async () => {
      await renderTip();
      showTip({ choices: [] });
      expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    });

    it("reports its placement with the content's sequence", async () => {
      await renderTip();
      showTip({ sequence: 7 });
      expect(mocked.railTipRendered).toHaveBeenCalledWith(7, expect.objectContaining({ y: expect.any(Number), height: expect.any(Number) }));
      showTip({ sequence: 8 });
      expect(mocked.railTipRendered).toHaveBeenLastCalledWith(8, expect.any(Object));
    });
  });

  describe("phases", () => {
    it("enters when content arrives", async () => {
      await renderTip();
      showTip({ sequence: 1 });
      expect(phase()).toBe("entering");
    });

    it("opens when Rust places the content just rendered", async () => {
      await renderTip();
      showTip({ sequence: 3 });
      emit(mocked.onRailTipPlaced, 3);
      expect(phase()).toBe("open");
    });

    it("ignores a placement for another sequence", async () => {
      await renderTip();
      showTip({ sequence: 3 });
      emit(mocked.onRailTipPlaced, 2);
      expect(phase()).toBe("entering");
    });

    it("stays open when the next content arrives while open", async () => {
      await renderTip();
      showTip({ sequence: 1 });
      emit(mocked.onRailTipPlaced, 1);
      showTip({ sequence: 2 });
      expect(phase()).toBe("open");
    });

    it("hides when Rust hides the tip", async () => {
      await renderTip();
      showTip({ sequence: 1 });
      emit(mocked.onRailTipPlaced, 1);
      emit(mocked.onRailTipHidden, undefined);
      expect(phase()).toBe("hidden");
    });
  });

  describe("choices", () => {
    it("lists items as menu items when none can be checked", async () => {
      await renderTip();
      showTip({ choices: [item("Open Notes"), item("New Note")] });
      const menu = screen.getByRole("menu", { name: "Quick Notes" });
      expect(within(menu).getAllByRole("menuitem").map((el) => el.textContent)).toEqual(["Open Notes", "New Note"]);
    });

    it("lists checkable items as radios with their checked state", async () => {
      await renderTip();
      showTip({ choices: [item("Work", { checked: true }), item("Private", { checked: false })] });
      expect(screen.getByRole("menuitemradio", { name: "Work" })).toBeChecked();
      expect(screen.getByRole("menuitemradio", { name: "Private" })).not.toBeChecked();
    });

    it("shows an item's detail", async () => {
      await renderTip();
      showTip({ choices: [item("Standup", { detail: "09:30" })] });
      expect(screen.getByRole("menuitem", { name: /Standup/ })).toHaveTextContent("09:30");
    });

    it("shows headings and separators between the items", async () => {
      await renderTip();
      showTip({ choices: [{ kind: "heading", label: "Playlists" }, item("Focus"), { kind: "separator" }, item("Calm")] });
      expect(screen.getByText("Playlists")).toBeInTheDocument();
      expect(screen.getByRole("separator")).toBeInTheDocument();
    });

    it("shows steps as pressed buttons in a labelled group", async () => {
      await renderTip();
      showTip({
        choices: [
          {
            kind: "steps",
            label: "Volume",
            steps: [
              { label: "Low", command: "music:volume:low", active: false },
              { label: "High", command: "music:volume:high", active: true },
            ],
          },
        ],
      });
      const group = screen.getByRole("group", { name: "Volume" });
      expect(within(group).getByRole("button", { name: "High", pressed: true })).toBeInTheDocument();
      expect(within(group).getByRole("button", { name: "Low", pressed: false })).toBeInTheDocument();
    });

    it("runs the command of a clicked item", async () => {
      await renderTip();
      showTip({ choices: [item("Open Notes", { command: "quicknotes:open" })] });
      await userEvent.click(screen.getByRole("menuitem", { name: "Open Notes" }));
      expect(mocked.railTipRun).toHaveBeenCalledWith("quicknotes:open");
    });

    it("runs the command of a clicked step", async () => {
      await renderTip();
      showTip({ choices: [{ kind: "steps", label: "Volume", steps: [{ label: "Low", command: "music:volume:low", active: false }] }] });
      await userEvent.click(screen.getByRole("button", { name: "Low" }));
      expect(mocked.railTipRun).toHaveBeenCalledWith("music:volume:low");
    });
  });

  describe("pointer hover relayed by Rust", () => {
    function movePointer(over: HTMLElement | null) {
      vi.mocked(document.elementFromPoint).mockReturnValue(over);
      emit<RailPoint | null>(mocked.onRailTipPointer, over ? { x: 1, y: 1 } : null);
    }

    it("marks the row under the point and clears the mark when the pointer leaves", async () => {
      await renderTip();
      showTip({ choices: [item("Open Notes"), item("New Note")] });
      movePointer(screen.getByRole("menuitem", { name: "New Note" }));
      expect(screen.getByRole("menuitem", { name: "New Note" })).toHaveAttribute("data-hover", "true");
      expect(screen.getByRole("menuitem", { name: "Open Notes" })).not.toHaveAttribute("data-hover");
      movePointer(null);
      expect(screen.getByRole("menuitem", { name: "New Note" })).not.toHaveAttribute("data-hover");
    });

    it("marks a step under the point", async () => {
      await renderTip();
      showTip({ choices: [{ kind: "steps", label: "Volume", steps: [{ label: "Low", command: "music:volume:low", active: false }] }] });
      movePointer(screen.getByRole("button", { name: "Low" }));
      expect(screen.getByRole("button", { name: "Low" })).toHaveAttribute("data-hover", "true");
    });
  });
});
