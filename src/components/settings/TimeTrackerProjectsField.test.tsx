import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type Project, type Settings, type TimeTrackerProjects, type TtProject } from "../../api";
import { resetStore } from "../../state/store";
import { settings } from "../../test/settings";
import { ProjectsPage } from "./ProjectsPage";

const mocked = vi.mocked(api, true);
const save = vi.fn<(patch: Partial<Settings>) => void>();

const project = (patch: Partial<Project>): Project => ({ name: "Project", slackEmoji: "", active: true, contexts: [], keywords: [], launcherProjects: [], timetrackerProjects: [], ...patch });
const tt = (id: string, name: string, patch: Partial<TtProject> = {}): TtProject => ({
  id,
  name,
  clientId: null,
  client: null,
  color: "blue",
  shade: 3,
  billable: true,
  archived: false,
  ...patch,
});
const answer = (projects: TtProject[], state: TimeTrackerProjects["state"] = "available"): TimeTrackerProjects => ({ state, projects });

const OSCORP = project({ name: "Oscorp", timetrackerProjects: [{ id: "t1", name: "Checkout" }] });
const INTERNAL = project({ name: "Internal" });

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
  resetStore();
  render(<Harness initial={settings({ projects })} />);
}

const card = (title: string) => within(screen.getByRole("heading", { name: title }).closest("section")!);
const linked = (title = "Oscorp") => card(title).queryByRole("list", { name: "TimeTracker projects" });
const adder = (title = "Oscorp") => card(title).findByRole("textbox", { name: "Add a TimeTracker project" });
const saved = (name: string) => save.mock.lastCall![0].projects!.find((p) => p.name === name)!;
const matches = (title = "Oscorp") => within(card(title).getByRole("listbox", { name: "Matching TimeTracker projects" })).getAllByRole("option");

describe("TimeTrackerProjectsField", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.timetrackerProjects.mockResolvedValue(answer([tt("t1", "Checkout", { client: "Oscorp" }), tt("t2", "Disputes", { client: "Oscorp" }), tt("t3", "Retro", { client: "Internal" })]));
  });

  describe("linked projects", () => {
    it("lists a linked project with its client", async () => {
      renderPage();
      await vi.waitFor(() => expect(linked()).toHaveTextContent("CheckoutOscorp"));
    });

    it("marks a linked project that was archived in TimeTracker", async () => {
      mocked.timetrackerProjects.mockResolvedValue(answer([tt("t1", "Checkout", { archived: true })]));
      renderPage();
      await vi.waitFor(() => expect(linked()).toHaveTextContent("(archived)"));
    });

    it("marks a linked project TimeTracker no longer has", async () => {
      mocked.timetrackerProjects.mockResolvedValue(answer([]));
      renderPage();
      await vi.waitFor(() => expect(linked()).toHaveTextContent("Not in TimeTracker anymore"));
    });

    it("does not call a project gone while TimeTracker cannot be asked", async () => {
      mocked.timetrackerProjects.mockResolvedValue(answer([], "notRunning"));
      renderPage();
      await card("Oscorp").findByText(/isn't running/);
      expect(linked()).toHaveTextContent("Checkout");
      expect(linked()).not.toHaveTextContent("Not in TimeTracker anymore");
    });

    it("unlinks a project with its button", async () => {
      renderPage();
      await userEvent.click(await card("Oscorp").findByRole("button", { name: "Unlink Checkout" }));
      expect(saved("Oscorp").timetrackerProjects).toEqual([]);
    });
  });

  describe("linking", () => {
    it("lists the projects of a client typed by name", async () => {
      renderPage([project({ name: "Oscorp" })]);
      await userEvent.type(await adder(), "oscorp");
      expect(matches().map((o) => o.textContent)).toEqual(["CheckoutOscorp", "DisputesOscorp"]);
    });

    it("does not offer archived or already linked projects", async () => {
      mocked.timetrackerProjects.mockResolvedValue(answer([tt("t1", "Checkout"), tt("t2", "Checkout old", { archived: true }), tt("t3", "Checkout new")]));
      renderPage();
      await userEvent.type(await adder(), "checkout");
      expect(matches().map((o) => o.textContent)).toEqual(["Checkout new"]);
    });

    it("says when another project already links a match", async () => {
      renderPage([project({ name: "Oscorp" }), project({ name: "Internal", timetrackerProjects: [{ id: "t2", name: "Disputes" }] })]);
      await userEvent.type(await adder("Oscorp"), "disputes");
      expect(matches("Oscorp")[0]).toHaveTextContent("also in Internal");
    });

    it("links the highlighted match with Enter", async () => {
      renderPage([project({ name: "Oscorp" })]);
      await userEvent.type(await adder(), "disp{Enter}");
      expect(saved("Oscorp").timetrackerProjects).toEqual([{ id: "t2", name: "Disputes" }]);
    });

    it("links a clicked match and keeps the earlier ones", async () => {
      renderPage();
      await userEvent.type(await adder(), "retro");
      await userEvent.click(matches()[0]);
      expect(saved("Oscorp").timetrackerProjects).toEqual([
        { id: "t1", name: "Checkout" },
        { id: "t3", name: "Retro" },
      ]);
    });
  });

  describe("problems", () => {
    it("asks to start TimeTracker when it is not running and opens it", async () => {
      mocked.timetrackerProjects.mockResolvedValue(answer([], "notRunning"));
      renderPage();
      expect(await card("Oscorp").findByText("TimeTracker isn't running: start it to pick its projects.")).toBeInTheDocument();
      await userEvent.click(card("Oscorp").getByRole("button", { name: "Open TimeTracker" }));
      expect(mocked.timetrackerFocus).toHaveBeenCalled();
    });

    it("points to the settings when TimeTracker is not installed and asks again on Try Again", async () => {
      mocked.timetrackerProjects.mockResolvedValue(answer([], "missing"));
      renderPage();
      expect(await card("Oscorp").findByText("TimeTracker isn't installed: check its command in Settings → TimeTracker.")).toBeInTheDocument();
      await userEvent.click(card("Oscorp").getByRole("button", { name: "Try Again" }));
      await vi.waitFor(() => expect(mocked.timetrackerProjects).toHaveBeenLastCalledWith(true));
    });

    it("shows the error when TimeTracker did not answer and offers Try Again", async () => {
      mocked.timetrackerProjects.mockRejectedValueOnce({ kind: "io", message: "Timed out", code: null });
      renderPage();
      expect(await card("Oscorp").findByText("TimeTracker didn't answer: Timed out")).toBeInTheDocument();
      await userEvent.click(card("Oscorp").getByRole("button", { name: "Try Again" }));
      await vi.waitFor(() => expect(mocked.timetrackerProjects).toHaveBeenLastCalledWith(true));
      expect(await adder()).toBeInTheDocument();
    });

    it("says it is asking until the first answer arrives", async () => {
      mocked.timetrackerProjects.mockReturnValue(new Promise(() => {}));
      renderPage();
      expect(await card("Oscorp").findByText("Asking TimeTracker…")).toBeInTheDocument();
    });
  });
});
