import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type ProjectGroup } from "../../api";
import { hintKeys } from "../../test/panelHints";
import { ProjectsView } from "./ProjectsView";

const mocked = vi.mocked(api, true);

const group = (project: string, ...names: string[]): ProjectGroup => ({
  project,
  projects: names.map((name) => ({ id: `${project}-${name}`, name, command: `launcher:${project}-${name}` })),
});

async function renderView(groups: ProjectGroup[]) {
  mocked.projectsView.mockResolvedValue(groups);
  render(<ProjectsView />);
  await screen.findByRole("listbox", { name: "Projects" });
}

const input = () => screen.getByRole("textbox", { name: "Project" });

describe("ProjectsView", () => {
  beforeEach(() => vi.clearAllMocks());

  it("lists each group's Launcher projects under the project's name, even for a single group", async () => {
    await renderView([group("Atlas", "api", "web")]);
    expect(await screen.findByText("Atlas")).toBeInTheDocument();
    expect(screen.getAllByRole("option").map((o) => o.textContent)).toEqual(["⌘1api", "⌘2web"]);
    expect(hintKeys("Project")).toEqual(["↑", "↓"]);
    expect(hintKeys("Show")).toEqual(["↵"]);
    expect(hintKeys("Close")).toEqual(["esc"]);
  });

  it("marks each row for Stage with its id", async () => {
    await renderView([group("Atlas", "api")]);
    expect(await screen.findByRole("option", { name: /api/ })).toHaveAttribute("data-stage", "project:Atlas-api");
  });

  it("matches the typed project name against its rows", async () => {
    await renderView([group("Atlas", "api"), group("Borealis", "web")]);
    await userEvent.type(input(), "borealis");
    expect(screen.getAllByRole("option").map((o) => o.textContent)).toEqual(["⌘1web"]);
  });

  it("runs the selected row's command with Enter and with ⌘ plus its digit", async () => {
    await renderView([group("Atlas", "api"), group("Borealis", "web")]);
    await userEvent.keyboard("{Meta>}2{/Meta}");
    expect(mocked.runCommand).toHaveBeenLastCalledWith("launcher:Borealis-web");
    await userEvent.keyboard("{Enter}");
    expect(mocked.runCommand).toHaveBeenLastCalledWith("launcher:Atlas-api");
  });

  it("explains where to link projects when there are none", async () => {
    await renderView([]);
    expect(await screen.findByText("No Launcher projects yet. Link them in Settings → Projects.")).toBeInTheDocument();
  });

  it("says so when the typed words hide every project", async () => {
    await renderView([group("Atlas", "api")]);
    await userEvent.type(input(), "zzz");
    expect(screen.getByText("No matching projects.")).toBeInTheDocument();
  });

  it("shows the message of a failed command in the footer", async () => {
    mocked.runCommand.mockRejectedValue({ kind: "io", message: "Launcher missing", code: null });
    await renderView([group("Atlas", "api")]);
    await userEvent.keyboard("{Enter}");
    expect(await screen.findByText("Launcher missing")).toBeInTheDocument();
  });
});
