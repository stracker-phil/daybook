import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type Project, type Settings } from "../../api";
import { installCommands } from "../../commands";
import { resetStore } from "../../state/store";
import { settings } from "../../test/settings";
import { SettingsDialog } from "./SettingsDialog";

const mocked = vi.mocked(api, true);

const project = (patch: Partial<Project>): Project => ({ name: "Project", slackEmoji: "", active: true, contexts: [], keywords: [], launcherProjects: [], ...patch });

function renderPage(patch: Partial<Settings> = {}) {
  resetStore({ settings: settings(patch), dialog: { type: "settings", page: "projects" } });
  installCommands();
  render(<SettingsDialog />);
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

    it("offers the contexts field with two or more contexts", () => {
      renderPage();
      expect(oscorp().getByRole("textbox", { name: "Add to Contexts" })).toBeInTheDocument();
    });

    it("hides the contexts field with a single context", () => {
      renderPage({ contexts: [settings().contexts[0]] });
      expect(oscorp().queryByRole("textbox", { name: "Add to Contexts" })).not.toBeInTheDocument();
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

    it("adds a context typed into the contexts field", async () => {
      renderPage();
      await userEvent.type(oscorp().getByRole("textbox", { name: "Add to Contexts" }), "priv{Enter}");
      expect(savedProject("Oscorp").contexts).toEqual(["private"]);
    });

    it("lists the chosen contexts by title and removes one", async () => {
      renderPage({ projects: [project({ name: "Oscorp", contexts: ["work", "private"] })] });
      expect(oscorp().getByRole("list", { name: "Contexts" })).toHaveTextContent("[W] Work[P] Private");
      await userEvent.click(oscorp().getByRole("button", { name: "Remove [W] Work" }));
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
      expect(saved()[2]).toEqual({ name: "New project", slackEmoji: "", active: true, contexts: [], keywords: [], launcherProjects: [] });
      expect(await screen.findByRole("heading", { name: "New project" })).toBeInTheDocument();
    });
  });
});
