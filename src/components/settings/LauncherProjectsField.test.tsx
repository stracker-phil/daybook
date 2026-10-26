import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type LauncherProject, type LauncherProjects, type Project, type Settings } from "../../api";
import { settings } from "../../test/settings";
import { ProjectsPage } from "./ProjectsPage";

const mocked = vi.mocked(api, true);
const save = vi.fn<(patch: Partial<Settings>) => void>();

const project = (patch: Partial<Project>): Project => ({ name: "Project", slackEmoji: "", active: true, contexts: [], keywords: [], launcherProjects: [], ...patch });
const entry = (id: string, name: string, archived = false): LauncherProject => ({ id, name, archived });
const answer = (projects: LauncherProject[], rest: Partial<LauncherProjects> = {}): LauncherProjects => ({ found: true, projects, error: null, ...rest });

const OSCORP = project({ name: "Oscorp", launcherProjects: [{ id: "p1", name: "Osc Orp" }] });
const INTERNAL = project({ name: "Internal", launcherProjects: [{ id: "p2", name: "Intern" }] });

function Harness({ initial }: { initial: Settings }) {
  const [s, setS] = useState(initial);
  return (
    <ProjectsPage
      s={s}
      set={(patch) => {
        save(patch);
        setS((current) => ({ ...current, ...patch }));
      }}
    />
  );
}

function renderPage(projects: Project[] = [OSCORP, INTERNAL]) {
  render(<Harness initial={settings({ projects })} />);
}

const card = (title: string) => within(screen.getByRole("heading", { name: title }).closest("section")!);
const saved = (name: string) => save.mock.lastCall![0].projects!.find((p) => p.name === name)!;
const matchTexts = (title = "Oscorp") => within(card(title).getByRole("listbox", { name: "Matching Launcher projects" })).getAllByRole("option").map((o) => o.textContent);

describe("LauncherProjectsField", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.launcherProjects.mockResolvedValue(answer([entry("p1", "Osc Orp"), entry("p2", "Intern")]));
  });

  describe("loading", () => {
    it("asks Project Launcher once for the whole page, without forcing a refresh", async () => {
      renderPage();
      await screen.findAllByRole("textbox", { name: "Add a Launcher project" });
      expect(mocked.launcherProjects).toHaveBeenCalledTimes(1);
      expect(mocked.launcherProjects).toHaveBeenCalledWith(false);
    });

    it("says it is asking while the first answer is pending", () => {
      mocked.launcherProjects.mockReturnValue(new Promise(() => {}));
      renderPage();
      expect(card("Oscorp").getByText("Asking Project Launcher…")).toBeInTheDocument();
      expect(card("Oscorp").queryByRole("textbox", { name: "Add a Launcher project" })).not.toBeInTheDocument();
    });
  });

  describe("linked projects", () => {
    it("lists the linked projects by name", async () => {
      renderPage();
      await screen.findAllByRole("textbox", { name: "Add a Launcher project" });
      expect(card("Oscorp").getByRole("list", { name: "Launcher projects" })).toHaveTextContent("Osc Orp");
    });

    it("unlinks one with its button", async () => {
      renderPage();
      await userEvent.click(await card("Oscorp").findByRole("button", { name: "Unlink Osc Orp" }));
      expect(saved("Oscorp").launcherProjects).toEqual([]);
      expect(saved("Internal").launcherProjects).toEqual([{ id: "p2", name: "Intern" }]);
    });

    it("marks a project missing from Project Launcher's list", async () => {
      mocked.launcherProjects.mockResolvedValue(answer([entry("p2", "Intern")]));
      renderPage();
      expect(await card("Oscorp").findByText("Not in Project Launcher anymore")).toBeInTheDocument();
      expect(card("Internal").queryByText("Not in Project Launcher anymore")).not.toBeInTheDocument();
    });
  });

  describe("problems", () => {
    it("explains how to install the command line tool when it is not found", async () => {
      mocked.launcherProjects.mockResolvedValue(answer([], { found: false }));
      renderPage();
      expect(await card("Oscorp").findByText(/Install Project Launcher’s command line tool first/)).toBeInTheDocument();
      expect(card("Oscorp").queryByRole("textbox", { name: "Add a Launcher project" })).not.toBeInTheDocument();
    });

    it("shows the error Project Launcher answered with and marks no linked project as missing", async () => {
      mocked.launcherProjects.mockResolvedValue(answer([], { error: "exit 1" }));
      renderPage();
      expect(await card("Oscorp").findByText("Project Launcher didn’t answer: exit 1")).toBeInTheDocument();
      expect(card("Oscorp").queryByText("Not in Project Launcher anymore")).not.toBeInTheDocument();
    });

    it("reads a rejected call as Project Launcher not answering", async () => {
      mocked.launcherProjects.mockRejectedValue({ kind: "io", message: "boom", code: null });
      renderPage();
      expect(await card("Oscorp").findByText("Project Launcher didn’t answer: boom")).toBeInTheDocument();
    });

    it("asks again fresh with Try Again and then offers the adder", async () => {
      mocked.launcherProjects.mockResolvedValueOnce(answer([], { error: "exit 1" }));
      renderPage();
      await userEvent.click((await screen.findAllByRole("button", { name: "Try Again" }))[0]);
      expect(mocked.launcherProjects).toHaveBeenLastCalledWith(true);
      expect(await card("Oscorp").findByRole("textbox", { name: "Add a Launcher project" })).toBeInTheDocument();
    });
  });

  describe("adder", () => {
    const many = Array.from({ length: 10 }, (_, i) => entry(`m${i}`, `Proj ${String(i).padStart(2, "0")}`));

    it("offers only matches that contain every typed word", async () => {
      mocked.launcherProjects.mockResolvedValue(answer([entry("a", "Alpha Beta"), entry("b", "Beta Only"), entry("c", "Gamma")]));
      renderPage();
      await userEvent.type(await card("Oscorp").findByRole("textbox", { name: "Add a Launcher project" }), "bet alp");
      expect(matchTexts()).toEqual(["Alpha Beta"]);
    });

    it("lists names starting with the text before the others", async () => {
      mocked.launcherProjects.mockResolvedValue(answer([entry("a", "Alpha Beta"), entry("b", "Beta Zulu"), entry("c", "Beta Alpha")]));
      renderPage();
      await userEvent.type(await card("Oscorp").findByRole("textbox", { name: "Add a Launcher project" }), "beta");
      expect(matchTexts()).toEqual(["Beta Alpha", "Beta Zulu", "Alpha Beta"]);
    });

    it("shows at most eight matches", async () => {
      mocked.launcherProjects.mockResolvedValue(answer(many));
      renderPage();
      await userEvent.type(await card("Oscorp").findByRole("textbox", { name: "Add a Launcher project" }), "proj");
      expect(matchTexts()).toHaveLength(8);
    });

    it("leaves out archived projects and the ones already linked to this project", async () => {
      mocked.launcherProjects.mockResolvedValue(answer([entry("p1", "Osc Orp"), entry("x", "Osc Old", true), entry("y", "Osc New")]));
      renderPage();
      await userEvent.type(await card("Oscorp").findByRole("textbox", { name: "Add a Launcher project" }), "osc");
      expect(matchTexts()).toEqual(["Osc New"]);
    });

    it("says which other project a match is linked to", async () => {
      renderPage();
      await userEvent.type(await card("Oscorp").findByRole("textbox", { name: "Add a Launcher project" }), "intern");
      expect(card("Oscorp").getByRole("option", { name: /Intern/ })).toHaveTextContent("also in Internal");
    });

    it("says when nothing matches", async () => {
      renderPage();
      await userEvent.type(await card("Oscorp").findByRole("textbox", { name: "Add a Launcher project" }), "zzz");
      expect(card("Oscorp").getByText("No matching project.")).toBeInTheDocument();
    });

    it("adds the highlighted match with Enter as id and name, and clears the text", async () => {
      mocked.launcherProjects.mockResolvedValue(answer(many));
      renderPage([project({ name: "Oscorp" })]);
      const input = await card("Oscorp").findByRole("textbox", { name: "Add a Launcher project" });
      await userEvent.type(input, "proj{Enter}");
      expect(saved("Oscorp").launcherProjects).toEqual([{ id: "m0", name: "Proj 00" }]);
      expect(input).toHaveValue("");
      expect(card("Oscorp").getByRole("list", { name: "Launcher projects" })).toHaveTextContent("Proj 00");
    });

    it("moves the highlight with the arrow keys before adding", async () => {
      mocked.launcherProjects.mockResolvedValue(answer(many));
      renderPage([project({ name: "Oscorp" })]);
      const input = await card("Oscorp").findByRole("textbox", { name: "Add a Launcher project" });
      await userEvent.type(input, "proj{ArrowDown}{ArrowDown}{ArrowUp}{Enter}");
      expect(saved("Oscorp").launcherProjects).toEqual([{ id: "m1", name: "Proj 01" }]);
    });

    it("adds a clicked match", async () => {
      mocked.launcherProjects.mockResolvedValue(answer(many));
      renderPage([project({ name: "Oscorp" })]);
      await userEvent.type(await card("Oscorp").findByRole("textbox", { name: "Add a Launcher project" }), "proj 05");
      await userEvent.click(card("Oscorp").getByRole("option", { name: "Proj 05" }));
      expect(saved("Oscorp").launcherProjects).toEqual([{ id: "m5", name: "Proj 05" }]);
    });

    it("clears the text with Escape", async () => {
      renderPage();
      const input = await card("Oscorp").findByRole("textbox", { name: "Add a Launcher project" });
      await userEvent.type(input, "intern{Escape}");
      expect(input).toHaveValue("");
      expect(card("Oscorp").queryByRole("listbox", { name: "Matching Launcher projects" })).not.toBeInTheDocument();
    });
  });
});
