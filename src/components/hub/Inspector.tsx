import { ChevronRight } from "lucide-react";
import { api, type OpenTodo, type TodoGroup } from "../../api";
import { fail } from "../../state/requests";
import { useStore } from "../../state/store";
import { Toolbar } from "../Toolbar";

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

/** The Daybook window's right column: every open todo in the daily notes, the same on every day. Read-only. */
export function Inspector() {
  const groups = useStore((s) => s.hub?.todos);
  const folded = useStore((s) => s.uiState.collapsedTodoGroups);
  const toggle = useStore((s) => s.toggleTodoGroup);
  const count = groups?.reduce((sum, group) => sum + group.todos.length, 0) ?? 0;
  return (
    <aside className="inspector" aria-label="Inspector" data-stage="hub:inspector" data-stage-label="Inspector: open todos" data-stage-role="region">
      <Toolbar title={groups ? `${count} open` : "Todos"} />
      <div className="inspector-body">
        {groups && count === 0 && <p className="board-empty o3">Nothing open in your daily notes.</p>}
        {groups?.map((group) => (
          <Group key={group.label} group={group} folded={folded.includes(group.label)} onToggle={() => toggle(group.label)} />
        ))}
      </div>
    </aside>
  );
}
