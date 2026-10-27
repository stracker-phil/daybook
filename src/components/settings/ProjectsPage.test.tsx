import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type Project, type Settings } from "../../api";
import { resetStore } from "../../state/store";
import { settings } from "../../test/settings";
import { SettingsWindow } from "./SettingsWindow";

const mocked = vi.mocked(api, true);

const project = (patch: Partial<Project>): Project => ({ name: "Project", slackEmoji: "", active: true, contexts: [], keywords: [], launcherProjects: [], timetrackerProjects: [], ...patch });

function renderPage(patch: Partial<Settings> = {}) {
  resetStore({ settings: settings(patch) });
  render(<SettingsWindow page="projects" onPage={() => {}} />);
}

const card = (title: string) => within(screen.getByRole("heading", { name: title }).closest("section")!);
const oscorp = () => card("Oscorp");

const saved = () => mocked.updateSettings.mock.lastCall![0].projects;
const savedProject = (name: string) => saved().find((p) => p.name === name)!;

describe("ProjectsPage", () => {
  beforeEach(() => vi.clearAllMocks());

  describe("cards", () => {
    it("shows one card per project", () => {
      renderPage();
      expect(screen.getAllByRole("heading", { level: 3 }).map((h) => h.textContent)).toEqual(["Internal", "Oscorp"]);
    });

    it("says in the title when a project is inactive", () => {
      renderPage({ projects: [project({ name: "Old", active: false })] });
      expect(screen.getByRole("heading", { name: "Old (inactive)" })).toBeInTheDocument();
      expect(screen.getByRole("switch", { name: "Active" })).not.toBeChecked();
    });

    it("fills the card with the project's values", () => {
      renderPage({ projects: [project({ name: "Oscorp", slackEmoji: ":oscorp:", keywords: ["QRS", "Initech"] })] });
      expect(oscorp().getByRole("textbox", { name: "Name" })).toHaveValue("Oscorp");
      expect(oscorp().getByRole("switch", { name: "Active" })).toBeChecked();
      expect(oscorp().getByRole("textbox", { name: "Meeting words" })).toHaveValue("QRS, Initech");
      expect(oscorp().getByRole("textbox", { name: "Slack emoji" })).toHaveValue(":oscorp:");
    });

    it("offers a chip per context with two or more contexts, none pressed for a project in every context", () => {
      renderPage();
      const chips = within(oscorp().getByRole("group", { name: "Contexts" }));
      expect(chips.getAllByRole("button").map((b) => b.textContent)).toEqual(["[W] Work", "[P] Private"]);
      for (const chip of chips.getAllByRole("button")) expect(chip).toHaveAttribute("aria-pressed", "false");
    });

    it("hides the context chips with a single context", () => {
      renderPage({ contexts: [settings().contexts[0]] });
      expect(oscorp().queryByRole("group", { name: "Contexts" })).not.toBeInTheDocument();
    });
  });

  describe("editing", () => {
    it("saves a renamed project on blur, trimmed", async () => {
      renderPage();
      await userEvent.clear(oscorp().getByRole("textbox", { name: "Name" }));
      await userEvent.type(oscorp().getByRole("textbox", { name: "Name" }), " Hooli ");
      await userEvent.tab();
      expect(saved().map((p) => p.name)).toEqual(["Internal", "Hooli"]);
    });

    it("keeps the name when it is emptied", async () => {
      renderPage();
      await userEvent.clear(oscorp().getByRole("textbox", { name: "Name" }));
      await userEvent.tab();
      expect(mocked.updateSettings).not.toHaveBeenCalled();
    });

    it("saves the Active switch", async () => {
      renderPage();
      await userEvent.click(oscorp().getByRole("switch", { name: "Active" }));
      expect(savedProject("Oscorp").active).toBe(false);
      expect(savedProject("Internal").active).toBe(true);
    });

    it("saves the meeting words as a list", async () => {
      renderPage();
      await userEvent.type(oscorp().getByRole("textbox", { name: "Meeting words" }), "QRS, Initech,{Enter}");
      expect(savedProject("Oscorp").keywords).toEqual(["QRS", "Initech"]);
    });

    it("saves the Slack emoji trimmed", async () => {
      renderPage();
      await userEvent.type(oscorp().getByRole("textbox", { name: "Slack emoji" }), " :oscorp: {Enter}");
      expect(savedProject("Oscorp").slackEmoji).toBe(":oscorp:");
    });

    it("adds a context by pressing its chip", async () => {
      renderPage();
      await userEvent.click(oscorp().getByRole("button", { name: "[P] Private" }));
      expect(savedProject("Oscorp").contexts).toEqual(["private"]);
    });

    it("presses the chips of the chosen contexts and removes one by pressing it again", async () => {
      renderPage({ projects: [project({ name: "Oscorp", contexts: ["work", "private"] })] });
      expect(oscorp().getByRole("button", { name: "[W] Work" })).toHaveAttribute("aria-pressed", "true");
      expect(oscorp().getByRole("button", { name: "[P] Private" })).toHaveAttribute("aria-pressed", "true");
      await userEvent.click(oscorp().getByRole("button", { name: "[W] Work" }));
      expect(savedProject("Oscorp").contexts).toEqual(["private"]);
    });
  });

  describe("list actions", () => {
    it("disables Move Up on the first card and Move Down on the last", () => {
      renderPage();
      expect(card("Internal").getByRole("button", { name: "Move Up" })).toBeDisabled();
      expect(oscorp().getByRole("button", { name: "Move Down" })).toBeDisabled();
    });

    it("saves the order after Move Down", async () => {
      renderPage();
      await userEvent.click(card("Internal").getByRole("button", { name: "Move Down" }));
      expect(saved().map((p) => p.name)).toEqual(["Oscorp", "Internal"]);
    });

    it("saves the order after Move Up", async () => {
      renderPage();
      await userEvent.click(oscorp().getByRole("button", { name: "Move Up" }));
      expect(saved().map((p) => p.name)).toEqual(["Oscorp", "Internal"]);
    });

    it("saves the list without the removed project", async () => {
      renderPage();
      await userEvent.click(oscorp().getByRole("button", { name: "Remove Project" }));
      expect(saved().map((p) => p.name)).toEqual(["Internal"]);
    });

    it("appends an active New project", async () => {
      renderPage();
      await userEvent.click(screen.getByRole("button", { name: "Add Project" }));
      expect(saved()[2]).toEqual({ name: "New project", slackEmoji: "", active: true, contexts: [], keywords: [], launcherProjects: [], timetrackerProjects: [] });
      expect(await screen.findByRole("heading", { name: "New project" })).toBeInTheDocument();
    });
  });
});
