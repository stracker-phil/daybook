import { useState } from "react";
import { Check, ChevronRight } from "lucide-react";
import { api, type OpenTodo, type TodoGroup } from "../../api";
import { fail } from "../../state/requests";
import { useStore } from "../../state/store";

const keyOf = (todo: OpenTodo) => `${todo.file}:${todo.line}:${todo.text}`;

/** Hides a ticked todo at once and brings it back when writing it to its note fails. */
function useTicking() {
  const loadHub = useStore((s) => s.loadHub);
  const [ticked, setTicked] = useState<ReadonlySet<string>>(new Set());
  const mark = (key: string, on: boolean) =>
    setTicked((current) => {
      const next = new Set(current);
      if (on) next.add(key);
      else next.delete(key);
      return next;
    });
  const tick = async (todo: OpenTodo) => {
    const key = keyOf(todo);
    mark(key, true);
    try {
      await api.todoComplete(todo);
    } catch (e) {
      fail(e, "Couldn't mark it as done");
    }
    await loadHub();
    mark(key, false);
  };
  return { ticked, tick };
}

function TodoRow({ todo, onTick }: { todo: OpenTodo; onTick: () => void }) {
  const open = () => void api.todoOpen(todo.file).catch((e) => fail(e, "Couldn't open the note"));
  return (
    <div className="todo-row">
      <button type="button" role="checkbox" aria-checked={false} className="todo-check" aria-label={`Mark “${todo.text}” as done`} onClick={onTick}>
        <Check size={12} strokeWidth={3} aria-hidden="true" />
      </button>
      <button type="button" className="todo-open" title="Open the note in Obsidian" onClick={open}>
        <span className="todo-text">{todo.text}</span>
        <span className="todo-detail o3">{todo.detail}</span>
      </button>
    </div>
  );
}

function Group({ group, folded, onToggle, onTick }: { group: TodoGroup; folded: boolean; onToggle: () => void; onTick: (todo: OpenTodo) => void }) {
  return (
    <section className="todo-group" aria-label={group.label}>
      <button type="button" className="todo-group-head" aria-expanded={!folded} onClick={onToggle}>
        <ChevronRight className="todo-chevron" size={12} aria-hidden="true" />
        {group.label} ({group.todos.length})
      </button>
      {!folded && group.todos.map((todo) => <TodoRow key={keyOf(todo)} todo={todo} onTick={() => onTick(todo)} />)}
    </section>
  );
}

/** The open todos without the ones being ticked; a group they emptied goes. */
export function useOpenTodos(groups: TodoGroup[] | undefined) {
  const { ticked, tick } = useTicking();
  const open = groups
    ?.map((group) => ({ ...group, todos: group.todos.filter((todo) => !ticked.has(keyOf(todo))) }))
    .filter((group) => group.todos.length > 0);
  const count = open?.reduce((sum, group) => sum + group.todos.length, 0) ?? 0;
  return { open, count, tick };
}

/** The open todos, grouped by their note's section. */
export function TodoList({ groups, count, onTick }: { groups: TodoGroup[] | undefined; count: number; onTick: (todo: OpenTodo) => void }) {
  const folded = useStore((s) => s.uiState.collapsedTodoGroups);
  const toggle = useStore((s) => s.toggleTodoGroup);
  return (
    <>
      {groups && count === 0 && <p className="board-empty o3">Nothing open in your daily notes.</p>}
      {groups?.map((group) => (
        <Group key={group.label} group={group} folded={folded.includes(group.label)} onToggle={() => toggle(group.label)} onTick={onTick} />
      ))}
    </>
  );
}
