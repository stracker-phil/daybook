import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import type { QuickLink, Settings } from "../../api";
import { settings } from "../../test/settings";
import { LinksPage } from "./LinksPage";

const save = vi.fn<(patch: Partial<Settings>) => void>();

const link = (patch: Partial<QuickLink> = {}): QuickLink => ({
  title: "Plan",
  target: "https://example.com/plan",
  contexts: null,
  projects: null,
  days: null,
  time: null,
  event: null,
  eventWindow: "until_end",
  leadMinutes: 15,
  pinned: true,
  ...patch,
});

function Harness({ initial }: { initial: Settings }) {
  const [s, setS] = useState(initial);
  return (
    <LinksPage
      s={s}
      set={(patch) => {
        save(patch);
        setS((current) => ({ ...current, ...patch }));
      }}
    />
  );
}

function renderPage(links: QuickLink[], patch: Partial<Settings> = {}) {
  render(<Harness initial={settings({ links, ...patch })} />);
}

const savedLinks = () => save.mock.lastCall![0].links!;
const savedLink = (i = 0) => savedLinks()[i];
const show = () => userEvent.click(screen.getAllByRole("button", { name: "Show Details" })[0]);
const setTime = (label: string, value: string) => {
  const input = screen.getByLabelText(label);
  fireEvent.change(input, { target: { value } });
  fireEvent.blur(input);
};

describe("LinksPage", () => {
  beforeEach(() => vi.clearAllMocks());

  describe("card layout", () => {
    it("lists all links as lines of one Quick access card", () => {
      renderPage([link({ title: "A" }), link({ title: "B" }), link({ title: "C" })]);
      expect(screen.getAllByRole("heading", { level: 3 }).map((h) => h.textContent)).toEqual(["Quick access"]);
      expect(screen.getAllByRole("textbox", { name: "Title" }).map((t) => (t as HTMLInputElement).value)).toEqual(["A", "B", "C"]);
    });

    it("opens the details of a link below its line while the others stay collapsed", async () => {
      renderPage([link({ title: "A" }), link({ title: "B" })]);
      await show();
      expect(screen.getAllByRole("textbox", { name: "URL" })).toHaveLength(1);
      expect(screen.getAllByRole("button", { name: "Show Details" })).toHaveLength(1);
    });

    it("offers Add Link even without links", () => {
      renderPage([]);
      expect(screen.getByRole("button", { name: "Add Link" })).toBeInTheDocument();
    });
  });

  describe("collapsed card", () => {
    it("shows the title and starts without details", () => {
      renderPage([link()]);
      expect(screen.getByRole("textbox", { name: "Title" })).toHaveValue("Plan");
      expect(screen.getByRole("button", { name: "Show Details" })).toHaveAttribute("aria-expanded", "false");
      expect(screen.queryByRole("textbox", { name: "URL" })).not.toBeInTheDocument();
    });

    it("summarizes every rule, days starting with Monday, joined by bars", () => {
      renderPage([link({ contexts: ["work"], projects: ["Oscorp"], days: [0, 1, 5], time: { from: 540, to: 720 }, event: "Weekly" })]);
      expect(screen.getByText("[W] Work | Oscorp | Mo, Fr, Su | 09:00–12:00 | 🗓️ Weekly")).toBeInTheDocument();
    });

    it.each([
      [true, "Always"],
      [false, "Only under More"],
    ])("summarizes a link without rules and pinned=%s as %s", (pinned, text) => {
      renderPage([link({ pinned })]);
      expect(screen.getByText(text)).toBeInTheDocument();
    });

    it("shows the details after Show Details and hides them after Hide Details", async () => {
      renderPage([link()]);
      await show();
      expect(screen.getByRole("textbox", { name: "URL" })).toHaveValue("https://example.com/plan");
      await userEvent.click(screen.getByRole("button", { name: "Hide Details" }));
      expect(screen.queryByRole("textbox", { name: "URL" })).not.toBeInTheDocument();
    });
  });

  describe("list actions", () => {
    it("adds an opened pinned New link", async () => {
      renderPage([link()]);
      await userEvent.click(screen.getByRole("button", { name: "Add Link" }));
      expect(savedLink(1)).toMatchObject({ title: "New link", target: "https://", pinned: true, contexts: null, projects: null, days: null, time: null, event: null });
      expect(screen.getByRole("textbox", { name: "URL" })).toHaveValue("https://");
    });

    it("saves a renamed title", async () => {
      renderPage([link()]);
      await userEvent.clear(screen.getByRole("textbox", { name: "Title" }));
      await userEvent.type(screen.getByRole("textbox", { name: "Title" }), "Roadmap{Enter}");
      expect(savedLink()).toMatchObject({ title: "Roadmap", target: "https://example.com/plan" });
    });

    it("drops a link left with neither title nor target", async () => {
      renderPage([link({ title: "Gone", target: "" }), link({ title: "Stays" })]);
      await userEvent.clear(screen.getAllByRole("textbox", { name: "Title" })[0]);
      await userEvent.tab();
      expect(savedLinks().map((l) => l.title)).toEqual(["Stays"]);
    });

    it("moves a link down and keeps its details open", async () => {
      renderPage([link({ title: "A" }), link({ title: "B" })]);
      await show();
      await userEvent.click(screen.getAllByRole("button", { name: "Move Down" })[0]);
      expect(savedLinks().map((l) => l.title)).toEqual(["B", "A"]);
      expect(screen.getAllByRole("button", { name: /Details$/ }).map((b) => b.getAttribute("aria-label"))).toEqual(["Show Details", "Hide Details"]);
    });

    it("removes a link and keeps the others' open state", async () => {
      renderPage([link({ title: "A" }), link({ title: "B" })]);
      await userEvent.click(screen.getAllByRole("button", { name: "Show Details" })[1]);
      await userEvent.click(screen.getAllByRole("button", { name: "Remove Link" })[0]);
      expect(savedLinks().map((l) => l.title)).toEqual(["B"]);
      expect(screen.getByRole("button", { name: "Hide Details" })).toBeInTheDocument();
    });
  });

  describe("URL", () => {
    it("saves the URL trimmed", async () => {
      renderPage([link()]);
      await show();
      await userEvent.clear(screen.getByRole("textbox", { name: "URL" }));
      await userEvent.type(screen.getByRole("textbox", { name: "URL" }), " 2-Work/Plan.md {Enter}");
      expect(savedLink().target).toBe("2-Work/Plan.md");
    });

    it("ignores an emptied URL and shows the saved one again", async () => {
      renderPage([link()]);
      await show();
      await userEvent.clear(screen.getByRole("textbox", { name: "URL" }));
      await userEvent.tab();
      expect(save).not.toHaveBeenCalled();
      expect(screen.getByRole("textbox", { name: "URL" })).toHaveValue("https://example.com/plan");
    });
  });

  describe("Context and Project", () => {
    it("adds a context by its chip and saves null when the last one is switched off", async () => {
      renderPage([link()]);
      await show();
      const chip = within(screen.getByRole("group", { name: "Context" })).getByRole("button", { name: "[P] Private" });
      expect(chip).toHaveAttribute("aria-pressed", "false");
      await userEvent.click(chip);
      expect(savedLink().contexts).toEqual(["private"]);
      await userEvent.click(chip);
      expect(savedLink().contexts).toBeNull();
    });

    it("hides the Context chips with a single context", async () => {
      renderPage([link()], { contexts: [settings().contexts[0]] });
      await show();
      expect(screen.queryByRole("group", { name: "Context" })).not.toBeInTheDocument();
    });

    it("saves the projects in the order of the Settings, not of picking", async () => {
      renderPage([link()]);
      await show();
      const chips = within(screen.getByRole("group", { name: "Project" }));
      await userEvent.click(chips.getByRole("button", { name: "Oscorp" }));
      await userEvent.click(chips.getByRole("button", { name: "Internal" }));
      expect(savedLink().projects).toEqual(["Internal", "Oscorp"]);
    });

    it("presses the chip of a chosen project and saves null after switching off the last one", async () => {
      renderPage([link({ projects: ["Oscorp"] })]);
      await show();
      const chip = within(screen.getByRole("group", { name: "Project" })).getByRole("button", { name: "Oscorp" });
      expect(chip).toHaveAttribute("aria-pressed", "true");
      await userEvent.click(chip);
      expect(savedLink().projects).toBeNull();
    });

    it("hides the Project chips without projects", async () => {
      renderPage([link()], { projects: [] });
      await show();
      expect(screen.queryByRole("group", { name: "Project" })).not.toBeInTheDocument();
    });
  });

  describe("Days", () => {
    it("adds a day to the chosen ones", async () => {
      renderPage([link({ days: [1] })]);
      await show();
      await userEvent.click(screen.getByRole("button", { name: "Tu" }));
      expect(savedLink().days).toEqual([1, 2]);
    });

    it("saves null when the last day is switched off", async () => {
      renderPage([link({ days: [1] })]);
      await show();
      await userEvent.click(screen.getByRole("button", { name: "Mo" }));
      expect(savedLink().days).toBeNull();
    });

    it("saves null when all days are chosen", async () => {
      renderPage([link({ days: [1, 2, 3, 4, 5, 6] })]);
      await show();
      await userEvent.click(screen.getByRole("button", { name: "Su" }));
      expect(savedLink().days).toBeNull();
    });
  });

  describe("Time", () => {
    it("turns on as 09:00 to 17:00", async () => {
      renderPage([link()]);
      await show();
      await userEvent.click(screen.getByRole("switch", { name: "Time" }));
      expect(savedLink().time).toEqual({ from: 540, to: 1020 });
      expect(screen.getByLabelText("Shows from")).toHaveValue("09:00");
      expect(screen.getByLabelText("Shows until")).toHaveValue("17:00");
    });

    it("turns off as null and brings the times back when turned on again", async () => {
      renderPage([link({ time: { from: 480, to: 600 } })]);
      await show();
      await userEvent.click(screen.getByRole("switch", { name: "Time" }));
      expect(savedLink().time).toBeNull();
      await userEvent.click(screen.getByRole("switch", { name: "Time" }));
      expect(savedLink().time).toEqual({ from: 480, to: 600 });
    });

    it("saves a changed end", async () => {
      renderPage([link({ time: { from: 540, to: 1020 } })]);
      await show();
      setTime("Shows until", "18:30");
      expect(savedLink().time).toEqual({ from: 540, to: 1110 });
    });

    it("does not save a range that ends before it starts", async () => {
      renderPage([link({ time: { from: 540, to: 1020 } })]);
      await show();
      setTime("Shows until", "08:00");
      expect(save).not.toHaveBeenCalled();
    });
  });

  describe("Meeting", () => {
    it("offers neither window nor lead time without a meeting", async () => {
      renderPage([link()]);
      await show();
      expect(screen.queryByRole("combobox", { name: "Shows" })).not.toBeInTheDocument();
    });

    it("saves the meeting text and shows the window select with its labels", async () => {
      renderPage([link()]);
      await show();
      await userEvent.type(screen.getByRole("textbox", { name: "Meeting" }), "Weekly{Enter}");
      expect(savedLink().event).toBe("Weekly");
      const select = screen.getByRole("combobox", { name: "Shows" });
      expect(select).toHaveValue("until_end");
      expect(Array.from(select.querySelectorAll("option")).map((o) => o.textContent)).toEqual(["Until the meeting ends", "During the meeting", "Before and during it"]);
    });

    it("saves an emptied meeting as null and hides the window", async () => {
      renderPage([link({ event: "Weekly" })]);
      await show();
      await userEvent.clear(screen.getByRole("textbox", { name: "Meeting" }));
      await userEvent.tab();
      expect(savedLink().event).toBeNull();
      expect(screen.queryByRole("combobox", { name: "Shows" })).not.toBeInTheDocument();
    });

    it.each([
      ["During the meeting", "during"],
      ["Before and during it", "before"],
    ])("saves %s as the window", async (label, value) => {
      renderPage([link({ event: "Weekly" })]);
      await show();
      await userEvent.selectOptions(screen.getByRole("combobox", { name: "Shows" }), label);
      expect(savedLink().eventWindow).toBe(value);
    });

    it.each(["until_end", "during"] as const)("hides Minutes before for the %s window", async (eventWindow) => {
      renderPage([link({ event: "Weekly", eventWindow })]);
      await show();
      expect(screen.queryByRole("spinbutton", { name: "Minutes before" })).not.toBeInTheDocument();
    });

    it("saves the minutes before for the before window", async () => {
      renderPage([link({ event: "Weekly", eventWindow: "before" })]);
      await show();
      const input = screen.getByRole("spinbutton", { name: "Minutes before" });
      expect(input).toHaveValue(15);
      await userEvent.clear(input);
      await userEvent.type(input, "30{Enter}");
      expect(savedLink().leadMinutes).toBe(30);
    });
  });

  describe("Pinned", () => {
    it("saves the switch for a link without rules", async () => {
      renderPage([link()]);
      await show();
      await userEvent.click(screen.getByRole("switch", { name: "Show when no rule is set" }));
      expect(savedLink().pinned).toBe(false);
    });
  });
});
