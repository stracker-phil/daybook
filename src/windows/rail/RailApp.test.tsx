import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type Rail, type RailButton, type RailModel, type RailShown, type RailTool, type Status, type ToolId } from "../../api";
import { settings } from "../../test/settings";
import { RailApp } from "./RailApp";

const mocked = vi.mocked(api, true);

function tool(id: ToolId, patch: Partial<RailTool> = {}): RailButton {
  return { kind: "tool", id, label: id, detail: null, ariaLabel: `${id} tool`, caption: null, shortcut: null, menu: null, command: `${id}:open`, badge: null, filled: false, accent: false, ...patch };
}

const SEPARATOR: RailButton = { kind: "separator" };

interface Setup {
  buttons?: RailButton[];
  status?: Status | null;
  rail?: Partial<Rail>;
  shown?: Partial<RailShown>;
}

async function renderRail({ buttons = [tool("search"), tool("jira")], status = null, rail = {}, shown = {} }: Setup = {}) {
  const model: RailModel = { buttons, status, settings: { ...settings().rail, ...rail } };
  mocked.railModel.mockResolvedValue(model);
  mocked.railShownState.mockResolvedValue({ visible: true, side: "right", sticky: false, ...shown });
  render(<RailApp />);
  const bar = await screen.findByRole("navigation", { name: "Rail" });
  await vi.waitFor(() => expect(bar).toHaveAttribute("data-motion", expect.not.stringMatching(/hidden/)));
  return bar;
}

const button = (name: string) => screen.getByRole("button", { name });

/** The point Rust relays over a button; `null` is the pointer leaving the rail. */
async function movePointer(over: HTMLElement | null) {
  await vi.waitFor(() => expect(mocked.onRailPointer).toHaveBeenCalled());
  const handler = mocked.onRailPointer.mock.calls.at(-1)![0];
  vi.mocked(document.elementFromPoint).mockReturnValue(over);
  await act(async () => handler(over ? { x: 1, y: 1 } : null));
}

describe("RailApp", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    document.elementFromPoint = vi.fn();
  });
  afterEach(() => {
    Reflect.deleteProperty(document, "elementFromPoint");
    vi.useRealTimers();
  });

  describe("buttons", () => {
    it("lists the model's buttons in order with separators between them", async () => {
      const bar = await renderRail({ buttons: [tool("search"), SEPARATOR, tool("jira")] });
      const items = Array.from(bar.querySelectorAll("button, [role=separator]"));
      expect(items.map((el) => el.getAttribute("aria-label") ?? el.getAttribute("role"))).toEqual(["search tool", "separator", "jira tool"]);
      expect(within(bar).getByRole("separator")).toBeInTheDocument();
    });

    it("runs a tool's command when its button is clicked", async () => {
      await renderRail();
      await userEvent.click(button("jira tool"));
      expect(mocked.runCommand).toHaveBeenCalledWith("jira:open");
    });

    it("shows the context's badge instead of an icon", async () => {
      await renderRail({ buttons: [tool("context", { badge: "WK", label: "Work" })] });
      expect(within(button("context tool")).getByText("WK")).toBeInTheDocument();
    });

    it("shows the first three letters of the context's label without a badge", async () => {
      await renderRail({ buttons: [tool("context", { badge: null, label: "Private" })] });
      expect(within(button("context tool")).getByText("Pri")).toBeInTheDocument();
    });

    it.each<Status>(["working", "break", "off"])("draws the status dot for %s", async (status) => {
      await renderRail({ buttons: [tool("status")], status });
      expect(button("status tool").firstElementChild).toHaveAttribute("data-status", status);
    });

    it("draws a hollow status dot before the day's first status", async () => {
      await renderRail({ buttons: [tool("status")], status: null });
      expect(button("status tool").firstElementChild).toHaveAttribute("data-status", "none");
    });

    it("opens the context menu on a right-click", async () => {
      await renderRail();
      await userEvent.pointer({ keys: "[MouseRight]", target: button("search tool") });
      expect(mocked.railContextMenu).toHaveBeenCalled();
    });
  });

  describe("captions", () => {
    it("shows a button's caption when captions are on", async () => {
      await renderRail({ buttons: [tool("search", { caption: "Find" })], rail: { captions: true } });
      expect(screen.getByText("Find")).toBeInTheDocument();
    });

    it("hides captions when captions are off", async () => {
      await renderRail({ buttons: [tool("search", { caption: "Find" })], rail: { captions: false } });
      expect(screen.queryByText("Find")).not.toBeInTheDocument();
    });

  });

  describe("look", () => {
    it("follows the shape, size and highlight settings and the side Rust shows it on", async () => {
      const bar = await renderRail({ rail: { shape: "docked", size: "large", highlight: "gray" }, shown: { side: "left" } });
      expect(bar).toHaveAttribute("data-shape", "docked");
      expect(bar).toHaveAttribute("data-size", "large");
      expect(bar).toHaveAttribute("data-highlight", "gray");
      expect(bar).toHaveAttribute("data-side", "left");
    });

    it("is shown while Rust shows the rail", async () => {
      const bar = await renderRail({ shown: { visible: true } });
      expect(bar).toHaveAttribute("data-motion", "shown");
    });

    it("is hidden while Rust slides the rail out", async () => {
      mocked.railModel.mockResolvedValue({ buttons: [], status: null, settings: settings().rail });
      mocked.railShownState.mockResolvedValue({ visible: false, side: "right", sticky: false });
      render(<RailApp />);
      const bar = await screen.findByRole("navigation", { name: "Rail" });
      await act(async () => {});
      expect(bar).toHaveAttribute("data-motion", "hidden");
    });
  });

  describe("fading when idle", () => {
    beforeEach(() => vi.useFakeTimers({ shouldAdvanceTime: true }));

    it("fades after 1.5 s without hover when the rail stays and fadeWhenIdle is on", async () => {
      const bar = await renderRail({ rail: { fadeWhenIdle: true }, shown: { sticky: true } });
      expect(bar).toHaveAttribute("data-motion", "shown");
      await act(() => vi.advanceTimersByTimeAsync(1500));
      expect(bar).toHaveAttribute("data-motion", "idle");
    });

    it("stays shown while the pointer is over a button", async () => {
      const bar = await renderRail({ rail: { fadeWhenIdle: true }, shown: { sticky: true } });
      await movePointer(button("search tool"));
      await act(() => vi.advanceTimersByTimeAsync(3000));
      expect(bar).toHaveAttribute("data-motion", "shown");
    });

    it("does not fade when the rail was opened by the pointer", async () => {
      const bar = await renderRail({ rail: { fadeWhenIdle: true }, shown: { sticky: false } });
      await act(() => vi.advanceTimersByTimeAsync(3000));
      expect(bar).toHaveAttribute("data-motion", "shown");
    });

    it("does not fade when fadeWhenIdle is off", async () => {
      const bar = await renderRail({ rail: { fadeWhenIdle: false }, shown: { sticky: true } });
      await act(() => vi.advanceTimersByTimeAsync(3000));
      expect(bar).toHaveAttribute("data-motion", "shown");
    });
  });

  describe("pointer hover relayed by Rust", () => {
    it("marks the button under the point as hovered and clears it when the pointer leaves", async () => {
      await renderRail();
      await movePointer(button("search tool"));
      expect(button("search tool")).toHaveAttribute("data-hover", "true");
      expect(button("jira tool")).not.toHaveAttribute("data-hover");
      await movePointer(null);
      expect(button("search tool")).not.toHaveAttribute("data-hover");
    });

    it("asks for the tip of the hovered tool at once without a tooltip delay", async () => {
      await renderRail({ rail: { tooltipDelay: 0 } });
      await movePointer(button("search tool"));
      expect(mocked.railShowTip).toHaveBeenCalledWith("search", expect.any(Number));
    });

    describe("with a tooltip delay", () => {
      beforeEach(() => vi.useFakeTimers({ shouldAdvanceTime: true }));

      it("asks for the first tip only after the delay", async () => {
        await renderRail({ rail: { tooltipDelay: 400 } });
        await movePointer(button("search tool"));
        await act(() => vi.advanceTimersByTimeAsync(399));
        expect(mocked.railShowTip).not.toHaveBeenCalled();
        await act(() => vi.advanceTimersByTimeAsync(1));
        expect(mocked.railShowTip).toHaveBeenCalledWith("search", expect.any(Number));
      });

      it("shows the next button's tip at once while a tip shows", async () => {
        await renderRail({ rail: { tooltipDelay: 400 } });
        await movePointer(button("search tool"));
        await act(() => vi.advanceTimersByTimeAsync(400));
        await movePointer(button("jira tool"));
        expect(mocked.railShowTip).toHaveBeenLastCalledWith("jira", expect.any(Number));
      });

      it("never asks for a tip when the pointer leaves before the delay", async () => {
        await renderRail({ rail: { tooltipDelay: 400 } });
        await movePointer(button("search tool"));
        await movePointer(null);
        await act(() => vi.advanceTimersByTimeAsync(1000));
        expect(mocked.railShowTip).not.toHaveBeenCalled();
        expect(mocked.railHideTip).not.toHaveBeenCalled();
      });

      it("hides the tip when the pointer leaves after one showed", async () => {
        await renderRail({ rail: { tooltipDelay: 400 } });
        await movePointer(button("search tool"));
        await act(() => vi.advanceTimersByTimeAsync(400));
        await movePointer(null);
        expect(mocked.railHideTip).toHaveBeenCalled();
      });
    });
  });
});
