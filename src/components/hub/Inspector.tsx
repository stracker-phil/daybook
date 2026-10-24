import { Toolbar } from "../Toolbar";

/** The Daybook window's right column; its tabs come with their features, the open todos first. */
export function Inspector() {
  return (
    <aside className="inspector" aria-label="Inspector">
      <Toolbar title="Todos" />
      <div className="inspector-body" />
    </aside>
  );
}
