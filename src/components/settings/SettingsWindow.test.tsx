import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { Workflow } from "lucide-react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { registerSettingsPage } from "../../registry/settingsPages";
import { resetStore } from "../../state/store";
import { settings } from "../../test/settings";
import { SettingsWindow } from "./SettingsWindow";

const mocked = vi.mocked(api, true);

function Shell({ initial }: { initial: string | undefined }) {
  const [page, setPage] = useState(initial);
  return <SettingsWindow page={page} onPage={setPage} />;
}

function renderWindow(page?: string, patch: Parameters<typeof settings>[0] = {}) {
  resetStore({ settings: settings(patch) });
  return render(<Shell initial={page} />);
}

const nav = () => screen.getByRole("navigation", { name: "Settings pages" });
const openPage = (name: string) => userEvent.click(within(nav()).getByRole("button", { name }));
const heading = (name: string) => screen.getByRole("heading", { name, level: 2 });
const searchField = () => screen.getByRole("searchbox", { name: "Search Settings" });
const navLabels = () => within(nav()).getAllByRole("button").map((b) => b.textContent?.trim());

const scrollIntoView = vi.fn();
Element.prototype.scrollIntoView = scrollIntoView;
const scrolledTo = () => (scrollIntoView.mock.contexts.at(-1) as Element | undefined)?.getAttribute("data-section");

describe("SettingsWindow", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    registerSettingsPage({ id: "demo", group: "Workflows", label: "Demo flow", icon: Workflow, Body: () => <p>Demo body</p> });
  });

  it("renders nothing until the settings arrived", () => {
    resetStore({ settings: null });
    render(<SettingsWindow page="connections" onPage={() => {}} />);
    expect(screen.queryByRole("navigation")).not.toBeInTheDocument();
  });

  it("opens on the page it is given", () => {
    renderWindow("appearance");
    expect(heading("Appearance")).toBeInTheDocument();
  });

  it.each([undefined, "no-such-page"])("opens on the first page, Connections, for %s", (page) => {
    renderWindow(page);
    expect(heading("Connections")).toBeInTheDocument();
  });

  it("switches the shown page from the navigation", async () => {
    renderWindow("connections");
    await openPage("Appearance");
    expect(heading("Appearance")).toBeInTheDocument();
  });

  it("shows a page registered by a module", async () => {
    renderWindow("connections");
    await openPage("Demo flow");
    expect(screen.getByText("Demo body")).toBeInTheDocument();
  });

  describe("former pages", () => {
    it.each([
      ["vault", "Connections"],
      ["calendar", "Connections"],
      ["slack", "Connections"],
      ["jira", "Connections"],
      ["timetracker", "Connections"],
      ["calls", "Workflows"],
      ["tools", "General"],
      ["rail", "Appearance"],
    ])("opens %s on the %s page, scrolled to its group", (id, page) => {
      renderWindow(id);
      expect(heading(page)).toBeInTheDocument();
      expect(scrolledTo()).toBe(id);
    });

    it("marks the page holding the section as current", () => {
      renderWindow("rail");
      expect(within(nav()).getByRole("button", { name: "Appearance" })).toHaveAttribute("aria-current", "page");
    });

    it("scrolls to the section the navigation was asked for again", () => {
      const { rerender } = renderWindow("vault");
      rerender(<SettingsWindow page="jira" onPage={() => {}} />);
      expect(scrolledTo()).toBe("jira");
    });
  });

  describe("navigation", () => {
    it("marks the shown page as current", async () => {
      renderWindow("connections");
      await openPage("General");
      expect(within(nav()).getByRole("button", { name: "General" })).toHaveAttribute("aria-current", "page");
      expect(within(nav()).getByRole("button", { name: "Connections" })).not.toHaveAttribute("aria-current");
    });

    it("lists the 16 pages in order, plus a page a module registered", () => {
      renderWindow();
      expect(navLabels()).toEqual([
        "Connections",
        "Contexts",
        "Projects",
        "Daily Note",
        "Routines",
        "Journal",
        "Search",
        "Workflows",
        "Reminders",
        "Links",
        "Music",
        "Demo flow",
        "General",
        "Appearance",
        "Shortcuts",
        "Automation",
        "Status",
      ]);
    });

    it("groups them under Setup, Obsidian, Workflows and System, in that order", () => {
      renderWindow();
      const text = nav().textContent.replace(/\s+/g, "");
      expect(text).toBe("SetupConnectionsContextsProjectsObsidianDailyNoteRoutinesJournalSearchWorkflowsWorkflowsRemindersLinksMusicDemoflowSystemGeneralAppearanceShortcutsAutomationStatus");
    });

    describe("search", () => {
      it("finds a section's page and shows the section as a hint", async () => {
        renderWindow();
        await userEvent.type(searchField(), "jira");
        expect(navLabels()).toEqual(["Connections Jira"]);
      });

      it("opens the page at the section when its hit is clicked", async () => {
        renderWindow("general");
        await userEvent.type(searchField(), "jira");
        await userEvent.click(within(nav()).getByRole("button", { name: "Connections Jira" }));
        expect(heading("Connections")).toBeInTheDocument();
        expect(scrolledTo()).toBe("jira");
      });

      it("lists the pages of a nav group by its name", async () => {
        renderWindow();
        await userEvent.type(searchField(), "system");
        expect(navLabels()).toEqual(["General", "Appearance", "Shortcuts", "Automation", "Status"]);
      });

      it("finds a page by its label, ignoring case", async () => {
        renderWindow();
        await userEvent.type(searchField(), "REMIND");
        expect(navLabels()).toEqual(["Reminders"]);
      });

      it("says No Results when nothing matches", async () => {
        renderWindow();
        await userEvent.type(searchField(), "zzzzz");
        expect(screen.getByText("No Results")).toBeInTheDocument();
        expect(within(nav()).queryAllByRole("button")).toHaveLength(0);
      });

      it("opens the first hit on Enter", async () => {
        renderWindow("general");
        await userEvent.type(searchField(), "jira{Enter}");
        expect(heading("Connections")).toBeInTheDocument();
        expect(scrolledTo()).toBe("jira");
      });

      it("opens the first page of a group on Enter", async () => {
        renderWindow();
        await userEvent.type(searchField(), "system{Enter}");
        expect(heading("General")).toBeInTheDocument();
      });

      it("opens nothing on Enter without a hit", async () => {
        renderWindow("general");
        await userEvent.type(searchField(), "zzzzz{Enter}");
        expect(heading("General")).toBeInTheDocument();
      });
    });
  });

  describe("Appearance", () => {
    const schemes = (name: string) => within(screen.getByRole("group", { name }));
    const renderAppearance = (patch: Parameters<typeof settings>[0] = {}) => renderWindow("appearance", patch);

    it("has no Mode switcher", () => {
      renderAppearance();
      expect(screen.queryByRole("group", { name: "Mode" })).not.toBeInTheDocument();
    });

    it.each(["Light mode scheme", "Dark mode scheme"])("offers %s as Default, Light and Dark", (name) => {
      renderAppearance();
      expect(schemes(name).getAllByRole("button").map((b) => b.textContent)).toEqual(["Default", "Light", "Dark"]);
    });

    it("presses the saved scheme in each group", () => {
      renderAppearance({ lightScheme: "light", darkScheme: "default" });
      expect(schemes("Light mode scheme").getByRole("button", { name: "Light" })).toHaveAttribute("aria-pressed", "true");
      expect(schemes("Light mode scheme").getByRole("button", { name: "Default" })).toHaveAttribute("aria-pressed", "false");
      expect(schemes("Dark mode scheme").getByRole("button", { name: "Default" })).toHaveAttribute("aria-pressed", "true");
      expect(schemes("Dark mode scheme").getByRole("button", { name: "Light" })).toHaveAttribute("aria-pressed", "false");
    });

    it("saves the clicked tile as the light scheme and leaves the dark one unchanged", async () => {
      renderAppearance({ lightScheme: "default", darkScheme: "dark" });
      await userEvent.click(schemes("Light mode scheme").getByRole("button", { name: "Dark" }));
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ lightScheme: "dark", darkScheme: "dark" }));
    });

    it("saves the clicked tile as the dark scheme and leaves the light one unchanged", async () => {
      renderAppearance({ lightScheme: "default", darkScheme: "dark" });
      await userEvent.click(schemes("Dark mode scheme").getByRole("button", { name: "Light" }));
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ lightScheme: "default", darkScheme: "light" }));
    });

    describe("Text size", () => {
      const sizes = () => schemes("Text size");

      it("offers Small, Medium and Large", () => {
        renderAppearance();
        expect(sizes().getAllByRole("button").map((b) => b.textContent)).toEqual(["Small", "Medium", "Large"]);
      });

      it("presses the saved size", () => {
        renderAppearance({ textSize: "large" });
        expect(sizes().getByRole("button", { name: "Large" })).toHaveAttribute("aria-pressed", "true");
        expect(sizes().getByRole("button", { name: "Medium" })).toHaveAttribute("aria-pressed", "false");
      });

      it("saves the clicked size", async () => {
        renderAppearance();
        await userEvent.click(sizes().getByRole("button", { name: "Small" }));
        expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ textSize: "small" }));
      });

      it("shows no pixel sizes", () => {
        renderAppearance();
        expect(document.body.textContent).not.toMatch(/\d\s*px|\bpx\b/i);
      });
    });

    describe("Day timeline", () => {
      const optionTexts = (name: string) => within(screen.getByRole("combobox", { name })).getAllByRole("option").map((o) => o.textContent);

      it("shows the saved first and last hour", () => {
        renderAppearance({ timeline: { firstHour: 7, lastHour: 22 } });
        expect(screen.getByRole("combobox", { name: "Starts at" })).toHaveValue("7");
        expect(screen.getByRole("combobox", { name: "Ends at" })).toHaveValue("22");
      });

      it("offers only hours before the end to start at and only hours after the start to end at", () => {
        renderAppearance({ timeline: { firstHour: 20, lastHour: 23 } });
        const starts = optionTexts("Starts at");
        expect(starts[0]).toBe("00:00");
        expect(starts.at(-1)).toBe("22:00");
        const ends = optionTexts("Ends at");
        expect(ends[0]).toBe("21:00");
        expect(ends.at(-1)).toBe("24:00");
      });

      it("saves the chosen start and keeps the end", async () => {
        renderAppearance({ timeline: { firstHour: 0, lastHour: 24 } });
        await userEvent.selectOptions(screen.getByRole("combobox", { name: "Starts at" }), "08:00");
        expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ timeline: { firstHour: 8, lastHour: 24 } }));
      });

      it("saves the chosen end and keeps the start", async () => {
        renderAppearance({ timeline: { firstHour: 6, lastHour: 24 } });
        await userEvent.selectOptions(screen.getByRole("combobox", { name: "Ends at" }), "20:00");
        expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ timeline: { firstHour: 6, lastHour: 20 } }));
      });
    });

    it("saves the accent of the clicked swatch", async () => {
      renderAppearance();
      await userEvent.click(screen.getByRole("button", { name: "Accent teal" }));
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ accent: "teal" }));
    });

    it("saves the system accent when the System swatch is clicked", async () => {
      renderAppearance();
      await userEvent.click(screen.getByRole("button", { name: "Accent system" }));
      expect(mocked.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ accent: "system" }));
    });

    it("marks the current accent as pressed", () => {
      renderAppearance({ accent: "system" });
      expect(screen.getByRole("button", { name: "Accent system" })).toHaveAttribute("aria-pressed", "true");
      expect(screen.getByRole("button", { name: "Accent orange" })).toHaveAttribute("aria-pressed", "false");
    });
  });

  describe("General", () => {
    it("installs launch at login when the switch is turned on", async () => {
      renderWindow("general");
      await userEvent.click(screen.getByRole("switch", { name: "Launch at login" }));
      expect(mocked.autostartSet).toHaveBeenLastCalledWith(true);
      await vi.waitFor(() => expect(screen.getByRole("switch", { name: "Launch at login" })).toBeChecked());
    });

    it("shows the launch at login switch on when the LaunchAgent is installed", async () => {
      mocked.autostartStatus.mockResolvedValueOnce(true);
      renderWindow("general");
      await vi.waitFor(() => expect(screen.getByRole("switch", { name: "Launch at login" })).toBeChecked());
    });

    it("has no Accessibility row", () => {
      renderWindow("general");
      expect(screen.queryByText("Accessibility")).not.toBeInTheDocument();
    });
  });
});
