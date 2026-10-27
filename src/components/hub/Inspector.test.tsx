import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, EMPTY_UI_STATE, type OpenTodo, type TodoGroup } from "../../api";
import { flushUiState, resetStore, useStore } from "../../state/store";
import { Inspector } from "./Inspector";

const mocked = vi.mocked(api, true);

const todo = (text: string, patch: Partial<OpenTodo> = {}): OpenTodo => ({ text, detail: "Mon 19 Oct", date: "2026-10-19", file: "Daily/2026-10-19.md", ...patch });
const group = (label: string, todos: OpenTodo[]): TodoGroup => ({ label, todos });

async function renderInspector(todos: TodoGroup[] | null, collapsed: string[] = []) {
  const hub = { ...(await api.hubView()), todos: todos ?? [] };
  resetStore({ hub: todos ? hub : null, uiState: { ...EMPTY_UI_STATE, collapsedTodoGroups: collapsed } });
  render(<Inspector />);
}

describe("Inspector", () => {
  beforeEach(() => vi.clearAllMocks());

  afterEach(() => flushUiState());

  it("titles the column with the number of open todos across all groups", async () => {
    await renderInspector([group("Today", [todo("Write report"), todo("Call Sam")]), group("Last week", [todo("Pay invoice")])]);
    expect(screen.getByText("3 open")).toBeInTheDocument();
  });

  it("says nothing is open when no group has a todo", async () => {
    await renderInspector([]);
    expect(screen.getByText("0 open")).toBeInTheDocument();
    expect(screen.getByText("Nothing open in your daily notes.")).toBeInTheDocument();
  });

  it("shows a generic title and no empty text before the hub arrived", async () => {
    await renderInspector(null);
    expect(screen.getByText("Todos")).toBeInTheDocument();
    expect(screen.queryByText("Nothing open in your daily notes.")).not.toBeInTheDocument();
  });

  it("lists each group as a section with its label and count", async () => {
    await renderInspector([group("Today", [todo("Write report"), todo("Call Sam")]), group("Last week", [todo("Pay invoice")])]);
    expect(within(screen.getByRole("region", { name: "Today" })).getByRole("button", { name: "Today (2)" })).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("button", { name: "Last week (1)" })).toBeInTheDocument();
  });

  it("shows the text and the detail of a todo", async () => {
    await renderInspector([group("Today", [todo("Write report", { detail: "Sat 24 Oct" })])]);
    const row = screen.getByRole("button", { name: /Write report/ });
    expect(row).toHaveTextContent("Sat 24 Oct");
  });

  it("opens the note of a clicked todo", async () => {
    await renderInspector([group("Today", [todo("Write report", { file: "Daily/2026-10-24.md" }), todo("Call Sam")])]);
    await userEvent.click(screen.getByRole("button", { name: /Write report/ }));
    expect(mocked.todoOpen).toHaveBeenCalledWith("Daily/2026-10-24.md");
  });

  it("toasts when the note cannot be opened", async () => {
    mocked.todoOpen.mockRejectedValueOnce({ kind: "io", message: "Obsidian missing", code: null });
    await renderInspector([group("Today", [todo("Write report")])]);
    await userEvent.click(screen.getByRole("button", { name: /Write report/ }));
    await vi.waitFor(() => expect(useStore.getState().toasts.map((t) => t.message).join()).toContain("Obsidian missing"));
  });

  describe("Activity tab", () => {
    async function renderWithTabs(tab: "todos" | "activity", activityTab = true) {
      const hub = { ...(await api.hubView()), todos: [group("Today", [todo("Write report")])], activityTab };
      resetStore({ hub, uiState: { ...EMPTY_UI_STATE, inspectorTab: tab } });
      render(<Inspector />);
    }

    const tabSwitch = () => screen.queryByRole("group", { name: "Inspector tab" });

    it("offers the Todos and Activity switch when Rust offers the tab", async () => {
      await renderWithTabs("todos");
      const buttons = within(tabSwitch()!).getAllByRole("button");
      expect(buttons.map((b) => b.textContent)).toEqual(["Todos", "Activity"]);
      expect(within(tabSwitch()!).getByRole("button", { name: "Todos" })).toHaveAttribute("aria-pressed", "true");
    });

    it("shows no switch and the todos when the tab is not offered, even if Activity was saved", async () => {
      await renderWithTabs("activity", false);
      expect(tabSwitch()).not.toBeInTheDocument();
      expect(screen.getByText("1 open")).toBeInTheDocument();
      expect(screen.getByText("Write report")).toBeInTheDocument();
    });

    it("titles the column Activity and hides the todos on the Activity tab", async () => {
      await renderWithTabs("activity");
      expect(screen.getByText("Activity", { selector: ":not(button)" })).toBeInTheDocument();
      expect(screen.queryByText("1 open")).not.toBeInTheDocument();
      expect(screen.queryByText("Write report")).not.toBeInTheDocument();
    });

    it("switches to the todos and back to the title with the count", async () => {
      await renderWithTabs("activity");
      await userEvent.click(within(tabSwitch()!).getByRole("button", { name: "Todos" }));
      expect(screen.getByText("1 open")).toBeInTheDocument();
      expect(screen.getByText("Write report")).toBeInTheDocument();
    });

    it("saves the chosen tab after the debounce", async () => {
      await renderWithTabs("todos");
      await userEvent.click(within(tabSwitch()!).getByRole("button", { name: "Activity" }));
      await vi.waitFor(() => expect(mocked.setUiState).toHaveBeenCalledWith(expect.objectContaining({ inspectorTab: "activity" })), { timeout: 2000 });
    });

    it("runs the TimeTracker command from the header button on the Activity tab", async () => {
      await renderWithTabs("activity");
      await userEvent.click(screen.getByRole("button", { name: "Open in TimeTracker" }));
      expect(mocked.runCommand).toHaveBeenCalledWith("timetracker:show");
    });

    it("has no TimeTracker header button on the Todos tab", async () => {
      await renderWithTabs("todos");
      expect(screen.queryByRole("button", { name: "Open in TimeTracker" })).not.toBeInTheDocument();
    });
  });

  describe("folding", () => {
    it("hides the todos of a clicked group and keeps the other groups open", async () => {
      await renderInspector([group("Today", [todo("Write report")]), group("Last week", [todo("Pay invoice")])]);
      await userEvent.click(screen.getByRole("button", { name: "Today (1)" }));
      expect(screen.getByRole("button", { name: "Today (1)" })).toHaveAttribute("aria-expanded", "false");
      expect(screen.queryByText("Write report")).not.toBeInTheDocument();
      expect(screen.getByText("Pay invoice")).toBeInTheDocument();
    });

    it("shows the todos again when the group is clicked twice", async () => {
      await renderInspector([group("Today", [todo("Write report")])]);
      await userEvent.click(screen.getByRole("button", { name: "Today (1)" }));
      await userEvent.click(screen.getByRole("button", { name: "Today (1)" }));
      expect(screen.getByText("Write report")).toBeInTheDocument();
    });

    it("starts folded for a group saved as collapsed", async () => {
      await renderInspector([group("Today", [todo("Write report")]), group("Last week", [todo("Pay invoice")])], ["Last week"]);
      expect(screen.getByRole("button", { name: "Last week (1)" })).toHaveAttribute("aria-expanded", "false");
      expect(screen.queryByText("Pay invoice")).not.toBeInTheDocument();
      expect(screen.getByText("Write report")).toBeInTheDocument();
    });

    it("saves the folded groups after the debounce", async () => {
      await renderInspector([group("Today", [todo("Write report")])]);
      await userEvent.click(screen.getByRole("button", { name: "Today (1)" }));
      expect(mocked.setUiState).not.toHaveBeenCalled();
      await vi.waitFor(() => expect(mocked.setUiState).toHaveBeenCalledWith(expect.objectContaining({ collapsedTodoGroups: ["Today"] })), { timeout: 2000 });
    });

    it("keeps counting the todos of a folded group in the title", async () => {
      await renderInspector([group("Today", [todo("Write report"), todo("Call Sam")])], ["Today"]);
      expect(screen.getByText("2 open")).toBeInTheDocument();
    });
  });
});
