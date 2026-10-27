import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { api, toAppError, type ContextState } from "../../api";
import { CurrentMarker, ESCAPE_HINT, PanelFooter, PanelList, PanelOption, PanelPrompt } from "./parts";

const STEPS: Record<string, number> = { ArrowDown: 1, ArrowUp: -1 };

/** A digit 1…9, alone or with ⌘ only; else 0. */
function digitOf(e: KeyboardEvent): number {
  if (e.ctrlKey || e.altKey || e.shiftKey) return 0;
  return /^Digit[1-9]$/.test(e.code) ? Number(e.code.slice(5)) : 0;
}

/** The contexts, numbered; ↑↓ and ↵ or 1…9 (also with ⌘) switch. Rust closes the overlay, also when the pick is current. */
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
    const digit = digitOf(e);
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

  return (
    <div className="panel-card context-view">
      <PanelPrompt>Switch context</PanelPrompt>
      <PanelList ref={list} aria-label="Contexts" tabIndex={0} onKeyDown={onKeyDown}>
        {contexts.map((context, i) => (
          <PanelOption key={context.id} selected={i === active} number={i < 9 ? `${i + 1}` : ""} narrowNumber onMouseMove={() => setActive(i)} onClick={() => choose(i)}>
            <span className="grow-text">{context.title}</span>
            {context.id === state?.current && <CurrentMarker context={context.color} />}
          </PanelOption>
        ))}
      </PanelList>
      <PanelFooter hints={[{ keys: ["↑", "↓"], label: "Context" }, { keys: ["↵"], label: "Switch" }, ESCAPE_HINT]} error={error} />
    </div>
  );
}
