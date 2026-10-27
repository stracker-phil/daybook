import { ChevronRight } from "lucide-react";
import { api, type OpenTodo, type TodoGroup, type UiState } from "../../api";
import { fail } from "../../state/requests";
import { useStore } from "../../state/store";
import { SegmentedControl } from "../SegmentedControl";
import { Toolbar } from "../Toolbar";
import { ActivityTab } from "./ActivityTab";
import { TimeTrackerButton } from "./TimeTrackerButton";

function TodoRow({ todo }: { todo: OpenTodo }) {
  const open = () => void api.todoOpen(todo.file).catch((e) => fail(e, "Couldn't open the note"));
  return (
    <button type="button" className="todo-row" title="Open the note in Obsidian" onClick={open}>
      <span className="todo-text">{todo.text}</span>
      <span className="todo-detail o3">{todo.detail}</span>
    </button>
  );
}

function Group({ group, folded, onToggle }: { group: TodoGroup; folded: boolean; onToggle: () => void }) {
  return (
    <section className="todo-group" aria-label={group.label}>
      <button type="button" className="todo-group-head" aria-expanded={!folded} onClick={onToggle}>
        <ChevronRight className="todo-chevron" size="0.923rem" aria-hidden="true" />
        {group.label} ({group.todos.length})
      </button>
      {!folded && group.todos.map((todo) => <TodoRow key={`${todo.file}:${todo.text}`} todo={todo} />)}
    </section>
  );
}

const TABS = ["todos", "activity"] as const satisfies readonly UiState["inspectorTab"][];

function TodoList({ groups, count }: { groups: TodoGroup[] | undefined; count: number }) {
  const folded = useStore((s) => s.uiState.collapsedTodoGroups);
  const toggle = useStore((s) => s.toggleTodoGroup);
  return (
    <>
      {groups && count === 0 && <p className="board-empty o3">Nothing open in your daily notes.</p>}
      {groups?.map((group) => (
        <Group key={group.label} group={group} folded={folded.includes(group.label)} onToggle={() => toggle(group.label)} />
      ))}
    </>
  );
}

/**
 * The Daybook window's right column: the open todos in the daily notes (the same on every day), or the shown day's
 * activity in TimeTracker, where Rust offers it. Read-only.
 */
export function Inspector() {
  const hub = useStore((s) => s.hub);
  const chosen = useStore((s) => s.uiState.inspectorTab);
  const choose = useStore((s) => s.setInspectorTab);
  // Without the tab the inspector shows the todos and keeps the choice for when it returns.
  const offered = hub?.activityTab ?? false;
  const tab = offered ? chosen : "todos";
  const groups = hub?.todos;
  const count = groups?.reduce((sum, group) => sum + group.todos.length, 0) ?? 0;
  const title = tab === "activity" ? "Activity" : groups ? `${count} open` : "Todos";
  return (
    <aside className="inspector" aria-label="Inspector" data-stage="hub:inspector" data-stage-label="Inspector: open todos or the day's activity" data-stage-role="region">
      <Toolbar title={title}>
        {tab === "activity" && hub && <TimeTrackerButton link={hub.timetracker} stage="hub:activity-timetracker" />}
        {offered && <SegmentedControl label="Inspector tab" options={TABS} value={tab} onChange={choose} />}
      </Toolbar>
      <div className="inspector-body">{tab === "activity" && hub ? <ActivityTab hub={hub} /> : <TodoList groups={groups} count={count} />}</div>
    </aside>
  );
}
