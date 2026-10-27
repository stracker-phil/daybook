import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type Rail, type Settings } from "../../api";
import { resetStore, useStore } from "../../state/store";
import { settings } from "../../test/settings";
import { RailPage } from "./RailPage";

const mocked = vi.mocked(api, true);
const save = vi.fn<(patch: Partial<Settings>) => void>();

function Harness({ initial }: { initial: Settings }) {
  const [s, setS] = useState(initial);
  return (
    <RailPage
      s={s}
      set={(patch) => {
        save(patch);
        setS((current) => ({ ...current, ...patch }));
      }}
    />
  );
}

function renderPage(patch: Partial<Rail> = {}) {
  const base = settings();
  const initial = settings({ rail: { ...base.rail, ...patch } });
  render(<Harness initial={initial} />);
  return initial.rail;
}

const savedRail = () => save.mock.lastCall![0].rail!;
const group = (name: string) => within(screen.getByRole("group", { name }));
const optionTexts = (name: string) => within(screen.getByRole("combobox", { name })).getAllByRole("option").map((o) => o.textContent);

describe("RailPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore({ settings: settings() });
  });

  describe("saving", () => {
    it.each([
      ["Show at the screen edge", { enabled: false }],
      ["Keep it visible during meetings", { showDuringMeetings: false }],
      ["Captions", { captions: true }],
      ["Fade when idle", { fadeWhenIdle: true }],
      ["Show choices in tooltips", { hoverMenus: false }],
    ] as [string, Partial<Rail>][])("saves the rail with %s toggled", async (label, patch) => {
      const rail = renderPage({ captions: false });
      await userEvent.click(screen.getByRole("switch", { name: label }));
      expect(savedRail()).toEqual({ ...rail, ...patch });
    });

    it.each([
      ["Edges", "Both", { edges: "both" }],
      ["Trigger along", "Beside the rail", { triggerSpan: "rail" }],
      ["Edge during a call", "Left", { callEdge: "left" }],
      ["Position during a call", "Bottom", { callPosition: "bottom" }],
      ["Appearance", "Dark", { theme: "dark" }],
      ["Shape", "Floating icons", { shape: "floating" }],
      ["Size", "Large", { size: "large" }],
      ["Hover highlight", "Gray", { highlight: "gray" }],
      ["Position", "Top", { position: "top" }],
    ] as [string, string, Partial<Rail>][])("saves the rail when %s is set to %s", async (label, button, patch) => {
      const rail = renderPage();
      await userEvent.click(group(label).getByRole("button", { name: button }));
      expect(savedRail()).toEqual({ ...rail, ...patch });
    });

    it.each([
      ["Trigger zone", "Wide (12 pt)", { triggerWidth: 12 }],
      ["Show after", "0.7 s", { showDelay: 700 }],
      ["Hide after", "3 s", { hideDelay: 3000 }],
      ["First tooltip after", "Long", { tooltipDelay: 600 }],
    ] as [string, string, Partial<Rail>][])("saves %s as a number when %s is picked", async (label, option, patch) => {
      const rail = renderPage();
      await userEvent.selectOptions(screen.getByRole("combobox", { name: label }), option);
      expect(savedRail()).toEqual({ ...rail, ...patch });
    });

    it("presses the saved value in a segmented control", () => {
      renderPage({ shape: "docked" });
      expect(group("Shape").getByRole("button", { name: "Docked" })).toHaveAttribute("aria-pressed", "true");
      expect(group("Shape").getByRole("button", { name: "Pill" })).toHaveAttribute("aria-pressed", "false");
    });
  });

  describe("values outside the presets", () => {
    it.each([
      ["Trigger zone", { triggerWidth: 3 }, "3 pt"],
      ["Show after", { showDelay: 250 }, "250 ms"],
      ["Hide after", { hideDelay: 1000 }, "1000 ms"],
      ["First tooltip after", { tooltipDelay: 450 }, "450 ms"],
    ] as [string, Partial<Rail>, string][])("shows the %s value as its own selected option", (label, patch, text) => {
      renderPage(patch);
      expect(optionTexts(label)).toContain(text);
      expect(screen.getByRole("combobox", { name: label })).toHaveDisplayValue(text);
    });

    it("lists no extra option for a value that is a preset", () => {
      renderPage({ triggerWidth: 6 });
      expect(optionTexts("Trigger zone")).toEqual(["Very edge (1 pt)", "Narrow (2 pt)", "Medium (6 pt)", "Wide (12 pt)", "Extra wide (24 pt)"]);
    });
  });

  describe("controls that depend on others", () => {
    it("enables the edge triggers while Show at the screen edge is on", () => {
      renderPage();
      expect(screen.getByRole("combobox", { name: "Trigger zone" })).toBeEnabled();
      expect(screen.getByRole("combobox", { name: "Show after" })).toBeEnabled();
      for (const button of group("Trigger along").getAllByRole("button")) expect(button).toBeEnabled();
    });

    it("disables Trigger zone, Show after and Trigger along while Show at the screen edge is off", async () => {
      renderPage();
      await userEvent.click(screen.getByRole("switch", { name: "Show at the screen edge" }));
      expect(screen.getByRole("combobox", { name: "Trigger zone" })).toBeDisabled();
      expect(screen.getByRole("combobox", { name: "Show after" })).toBeDisabled();
      for (const button of group("Trigger along").getAllByRole("button")) expect(button).toBeDisabled();
    });

    it("keeps Hide after and the tooltip delay enabled while Show at the screen edge is off", () => {
      renderPage({ enabled: false });
      expect(screen.getByRole("combobox", { name: "Hide after" })).toBeEnabled();
      expect(screen.getByRole("combobox", { name: "First tooltip after" })).toBeEnabled();
    });

    it("disables Trigger along when Position is At the pointer, and enables it again for another position", async () => {
      renderPage();
      await userEvent.click(group("Position").getByRole("button", { name: "At the pointer" }));
      for (const button of group("Trigger along").getAllByRole("button")) expect(button).toBeDisabled();
      await userEvent.click(group("Position").getByRole("button", { name: "Top" }));
      for (const button of group("Trigger along").getAllByRole("button")) expect(button).toBeEnabled();
    });

    it("disables the call edge and position when Keep it visible during meetings is off", async () => {
      renderPage();
      for (const name of ["Edge during a call", "Position during a call"]) {
        for (const button of group(name).getAllByRole("button")) expect(button).toBeEnabled();
      }
      await userEvent.click(screen.getByRole("switch", { name: "Keep it visible during meetings" }));
      for (const name of ["Edge during a call", "Position during a call"]) {
        for (const button of group(name).getAllByRole("button")) expect(button).toBeDisabled();
      }
    });
  });

  describe("Show or Hide the Rail", () => {
    it("runs the rail toggle command", async () => {
      renderPage();
      await userEvent.click(screen.getByRole("button", { name: "Show or Hide the Rail" }));
      expect(mocked.runCommand).toHaveBeenCalledWith("rail:toggle");
    });

    it("shows an error toast when the command fails", async () => {
      mocked.runCommand.mockRejectedValue({ kind: "io", message: "No screen", code: null });
      renderPage();
      await userEvent.click(screen.getByRole("button", { name: "Show or Hide the Rail" }));
      await vi.waitFor(() => expect(useStore.getState().toasts[0]?.message).toContain("Couldn't show the rail: No screen"));
    });
  });
});
