import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { api, toAppError, type ContextState } from "../../api";

const STEPS: Record<string, number> = { ArrowDown: 1, ArrowUp: -1 };

/** The contexts, numbered; ↑↓ and ↵ or ⌘1…⌘9 switch. Rust closes the overlay, also when the pick is current. */
export function ContextView() {
  const [state, setState] = useState<ContextState | null>(null);
  const [active, setActive] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const list = useRef<HTMLUListElement>(null);

  useEffect(() => {
    api.contextState().then(
      (next) => {
        setState(next);
        setActive(Math.max(0, next.contexts.findIndex((context) => context.id === next.current)));
      },
      (e) => setError(toAppError(e).message),
    );
  }, []);

  useEffect(() => {
    if (state) list.current?.focus();
  }, [state]);

  const contexts = state?.contexts ?? [];
  const choose = (index: number) => {
    const context = contexts[index];
    if (!context) return;
    api.runCommand(`mode:${context.id}`).catch((e) => setError(toAppError(e).message));
  };

  const onKeyDown = (e: KeyboardEvent<HTMLUListElement>) => {
    const step = STEPS[e.key];
    const digit = e.metaKey && /^Digit[1-9]$/.test(e.code) ? Number(e.code.slice(5)) : 0;
    if (step && contexts.length > 0) {
      e.preventDefault();
      setActive((i) => (i + step + contexts.length) % contexts.length);
    } else if (e.key === "Enter") {
      e.preventDefault();
      choose(active);
    } else if (digit) {
      e.preventDefault();
      choose(digit - 1);
    }
  };

  const footer = error ? <span className="palette-error">{error}</span> : <span>↑↓ context · ↵ switch · esc close</span>;

  return (
    <div className="panel-card context-view">
      <div className="panel-heading o2">Switch context</div>
      <ul ref={list} className="palette-list" role="listbox" aria-label="Contexts" tabIndex={0} onKeyDown={onKeyDown}>
        {contexts.map((context, i) => (
          <li
            key={context.id}
            role="option"
            aria-selected={i === active}
            className={i === active ? "active" : undefined}
            onMouseEnter={() => setActive(i)}
            onClick={() => choose(i)}
          >
            <span className="context-number o3">{i < 9 ? `⌘${i + 1}` : ""}</span>
            <span className="grow-text">{context.title}</span>
            {context.id === state?.current && (
              <span className="context-pill" data-context={context.color}>
                Current
              </span>
            )}
          </li>
        ))}
      </ul>
      <div className="palette-foot o3">{footer}</div>
    </div>
  );
}
