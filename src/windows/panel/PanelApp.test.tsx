import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type PanelSession } from "../../api";
import { PanelApp } from "./PanelApp";

const mocked = vi.mocked(api, true);

const session = (view: string, n = 1): PanelSession => ({ session: n, view, args: null });
const field = () => screen.getByRole("textbox", { name: "Text" });
const button = () => screen.getByRole("button", { name: "A button" });
const selected = () => screen.getAllByRole("option").find((o) => o.getAttribute("aria-selected") === "true")?.textContent;

function pushSession(value: PanelSession | null) {
  const handler = mocked.onPanelSession.mock.calls[0][0];
  act(() => handler(value));
}

async function renderWith(initial: PanelSession | null) {
  mocked.panelSession.mockResolvedValue(initial);
  render(<PanelApp />);
  if (initial) await screen.findByRole("listbox", { name: "Items" });
}

describe("PanelApp", () => {
  let rects: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    vi.clearAllMocks();
    // jsdom has no layout, so nothing counts as visible to the tab trap.
    rects = vi.spyOn(HTMLElement.prototype, "getClientRects").mockReturnValue([new DOMRect()] as unknown as DOMRectList);
  });

  afterEach(() => rects.mockRestore());

  describe("rendering a session", () => {
    it("shows the sample view with its field focused and reports the session ready", async () => {
      await renderWith(session("sample"));
      expect(field()).toHaveFocus();
      expect(field()).toHaveAttribute("placeholder", "Hello, overlay");
      expect(screen.getByRole("listbox", { name: "Items" })).toBeInTheDocument();
      expect(mocked.panelReady).toHaveBeenCalledWith(1, expect.any(Number));
    });

    it("renders the same view with its own placeholder for the sticky variant", async () => {
      await renderWith(session("sample-sticky"));
      expect(field()).toHaveAttribute("placeholder", "Hello, sticky overlay");
    });

    it("renders the command palette for the palette view", async () => {
      mocked.paletteQuery.mockResolvedValue({
        prefix: null,
        prefixes: [],
        groups: [{ kind: "command", heading: "Commands", entries: [{ kind: "command", label: "Open Hub", detail: "", shortcut: null, command: "hub:open" }] }],
      });
      mocked.panelSession.mockResolvedValue(session("palette"));
      render(<PanelApp />);
      expect(await screen.findByRole("option", { name: "Open Hub" })).toBeInTheDocument();
      expect(screen.getByRole("textbox", { name: "Search" })).toHaveFocus();
      expect(mocked.panelReady).toHaveBeenCalledWith(1, expect.any(Number));
    });

    it("renders the status view with the request's preselected status", async () => {
      mocked.panelSession.mockResolvedValue({
        session: 1,
        view: "status",
        args: { status: "break", current: "working", messages: { working: "re", break: "afk", off: "bye" }, endOfDay: null, endOfDayList: false },
      });
      render(<PanelApp />);
      expect(await screen.findByText("What is your status?")).toBeInTheDocument();
      expect(screen.getByRole("textbox", { name: "Message" })).toHaveValue("afk");
      expect(mocked.panelReady).toHaveBeenCalledWith(1, expect.any(Number));
    });

    it("renders today's focus list for the focus view", async () => {
      mocked.focusInfo.mockResolvedValue({ body: "- `[ ]` One", projects: [], slack: "none", channel: "" });
      mocked.panelSession.mockResolvedValue(session("focus"));
      render(<PanelApp />);
      expect(await screen.findByRole("textbox", { name: "Focus list" })).toHaveValue("- `[ ]` One");
      expect(mocked.panelReady).toHaveBeenCalledWith(1, expect.any(Number));
    });

    it("renders the asked journal group for the journal view", async () => {
      const q = (name: string) => ({ question: name, hint: "", kind: "text" as const, answer: { items: [], tasks: [], text: "" }, answered: false });
      mocked.journalInfo.mockResolvedValue({
        noteExists: true,
        groups: [
          { title: "Morning", questions: [q("Plan")], answered: 0, total: 1, note: "" },
          { title: "Evening", questions: [q("Highlight")], answered: 0, total: 1, note: "" },
        ],
      });
      mocked.panelSession.mockResolvedValue({ session: 1, view: "journal", args: { group: "Evening" } });
      render(<PanelApp />);
      expect(await screen.findByRole("textbox", { name: "Highlight" })).toBeInTheDocument();
      expect(mocked.panelReady).toHaveBeenCalledWith(1, expect.any(Number));
    });

    it("renders the asked routine's checklist for the wizard view", async () => {
      mocked.routineViewToday.mockResolvedValue({ id: "morning", name: "Morning", steps: [{ index: 0, title: "Plan day", link: "", done: false }] });
      mocked.panelSession.mockResolvedValue({ session: 1, view: "wizard", args: { routine: "morning" } });
      render(<PanelApp />);
      expect(await screen.findByRole("option", { name: "Plan day" })).toBeInTheDocument();
      expect(mocked.routineViewToday).toHaveBeenCalledWith("morning");
      expect(mocked.panelReady).toHaveBeenCalledWith(1, expect.any(Number));
    });

    it("renders the topic picker for the quicknotes view", async () => {
      mocked.quickNotesList.mockResolvedValue({ rows: [{ topic: { title: "Planning", level: 3 }, create: false }], hasTopics: true, notesHeading: "## Notes" });
      mocked.panelSession.mockResolvedValue(session("quicknotes"));
      render(<PanelApp />);
      expect(await screen.findByRole("option", { name: /Planning/ })).toBeInTheDocument();
      expect(screen.getByRole("textbox", { name: "Topic" })).toHaveFocus();
      expect(mocked.panelReady).toHaveBeenCalledWith(1, expect.any(Number));
    });

    it("renders the search field with the session's scope for the search view", async () => {
      mocked.panelSession.mockResolvedValue({ session: 1, view: "search", args: { scope: "work" } });
      render(<PanelApp />);
      expect(await screen.findByRole("textbox", { name: "Search" })).toHaveFocus();
      expect(screen.getByRole("button", { name: "Scope work. Press Delete to remove it." })).toBeInTheDocument();
      expect(mocked.panelReady).toHaveBeenCalledWith(1, expect.any(Number));
    });

    it("renders the link picker for the links view", async () => {
      mocked.linksView.mockResolvedValue({ now: [{ title: "Board", target: "https://example.com", command: "link:board" }], more: [] });
      mocked.panelSession.mockResolvedValue(session("links"));
      render(<PanelApp />);
      expect(await screen.findByRole("option", { name: /Board/ })).toBeInTheDocument();
      expect(screen.getByRole("textbox", { name: "Link" })).toHaveFocus();
      expect(mocked.panelReady).toHaveBeenCalledWith(1, expect.any(Number));
    });

    it("renders the music picker with Music's state for the music view", async () => {
      mocked.musicState.mockResolvedValue({ entries: [{ id: "a", name: "Focus" }], player: "playing", track: "Song", volume: 30, entry: "a" });
      mocked.panelSession.mockResolvedValue(session("music"));
      render(<PanelApp />);
      expect(await screen.findByRole("option", { name: /Focus/ })).toBeInTheDocument();
      expect(screen.getByText("Playing · Song")).toBeInTheDocument();
      expect(screen.getByRole("textbox", { name: "Entry" })).toHaveFocus();
      expect(mocked.panelReady).toHaveBeenCalledWith(1, expect.any(Number));
    });

    it("renders the project picker for the projects view", async () => {
      mocked.projectsView.mockResolvedValue([{ project: "Atlas", projects: [{ id: "a", name: "api", command: "launcher:a" }] }]);
      mocked.panelSession.mockResolvedValue(session("projects"));
      render(<PanelApp />);
      expect(await screen.findByRole("option", { name: /api/ })).toBeInTheDocument();
      expect(screen.getByRole("textbox", { name: "Project" })).toHaveFocus();
      expect(mocked.panelReady).toHaveBeenCalledWith(1, expect.any(Number));
    });

    it("renders the Jira field with its recent keys for the jira view", async () => {
      mocked.jiraSuggestions.mockResolvedValue(["QRS-1"]);
      mocked.panelSession.mockResolvedValue(session("jira"));
      render(<PanelApp />);
      expect(await screen.findByRole("option", { name: "QRS-1" })).toBeInTheDocument();
      expect(screen.getByRole("textbox", { name: "Open Jira issue" })).toHaveFocus();
      expect(mocked.panelReady).toHaveBeenCalledWith(1, expect.any(Number));
    });

    it("renders the activity prompt with its note and project for the activity view", async () => {
      mocked.panelSession.mockResolvedValue({
        session: 1,
        view: "activity",
        args: {
          mode: "switch",
          prompt: "Switched to Work",
          projects: [{ name: "Oscorp", ttProjects: [{ id: "qrs", name: "QRS", client: null, color: "blue", shade: 0 }], hint: null }],
          project: "Oscorp",
          ttProjectId: "qrs",
          note: "Review",
          tag: null,
          tt: "available",
        },
      });
      render(<PanelApp />);
      expect(await screen.findByText("Switched to Work")).toBeInTheDocument();
      expect(screen.getByRole("textbox", { name: "Note" })).toHaveValue("Review");
      expect(screen.getByRole("button", { name: "QRS", pressed: true })).toBeInTheDocument();
      expect(mocked.panelReady).toHaveBeenCalledWith(1, expect.any(Number));
    });

    it("renders the offer to set Off for the offer-off view", async () => {
      mocked.panelSession.mockResolvedValue({ session: 1, view: "offer-off", args: { to: "🐈 Personal" } });
      render(<PanelApp />);
      expect(await screen.findByText("Switched to 🐈 Personal")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Set to Off" })).toBeInTheDocument();
    });

    it("renders nothing for an unknown view", async () => {
      mocked.panelSession.mockResolvedValue(session("nope"));
      render(<PanelApp />);
      await vi.waitFor(() => expect(mocked.panelReady).toHaveBeenCalledWith(1, expect.any(Number)));
      expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
      expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    });

    it("renders a session pushed to the page and removes the view when null is pushed", async () => {
      await renderWith(null);
      expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
      pushSession(session("sample", 2));
      expect(field()).toBeInTheDocument();
      expect(mocked.panelReady).toHaveBeenCalledWith(2, expect.any(Number));
      pushSession(null);
      expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    });
  });

  describe("Escape", () => {
    it("closes the overlay", async () => {
      await renderWith(session("sample"));
      await userEvent.keyboard("{Escape}");
      expect(mocked.panelClose).toHaveBeenCalled();
    });

    it("closes the overlay while the text field has focus", async () => {
      await renderWith(session("sample"));
      await userEvent.type(field(), "abc{Escape}");
      expect(mocked.panelClose).toHaveBeenCalled();
    });

    it("does not close when a view already handled the key", async () => {
      await renderWith(session("sample"));
      const handled = (e: KeyboardEvent) => e.preventDefault();
      document.body.addEventListener("keydown", handled);
      fireEvent.keyDown(field(), { key: "Escape" });
      document.body.removeEventListener("keydown", handled);
      expect(mocked.panelClose).not.toHaveBeenCalled();
    });
  });

  describe("Tab", () => {
    it("moves from the field to the button and wraps back to the field", async () => {
      await renderWith(session("sample"));
      await userEvent.tab();
      expect(button()).toHaveFocus();
      await userEvent.tab();
      expect(field()).toHaveFocus();
    });

    it("wraps backwards from the field to the button and back", async () => {
      await renderWith(session("sample"));
      await userEvent.tab({ shift: true });
      expect(button()).toHaveFocus();
      await userEvent.tab({ shift: true });
      expect(field()).toHaveFocus();
    });
  });

  describe("list selection", () => {
    it("starts on the first option", async () => {
      await renderWith(session("sample"));
      expect(selected()).toBe("First item");
    });

    it("moves down with ArrowDown and wraps to the first option", async () => {
      await renderWith(session("sample"));
      await userEvent.keyboard("{ArrowDown}");
      expect(selected()).toBe("Second item");
      await userEvent.keyboard("{ArrowDown}{ArrowDown}");
      expect(selected()).toBe("First item");
    });

    it("moves up with ArrowUp and wraps to the last option", async () => {
      await renderWith(session("sample"));
      await userEvent.keyboard("{ArrowUp}");
      expect(selected()).toBe("Third item");
      await userEvent.keyboard("{ArrowUp}");
      expect(selected()).toBe("Second item");
    });
  });
});
