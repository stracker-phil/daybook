import { useEffect, useRef, useState } from "react";
import { Check, ExternalLink } from "lucide-react";
import { api, toAppError, type RoutineStepView, type RoutineView } from "../../api";
import { CLOSE_HINT, PanelFooter, PanelList, PanelOption, PanelPrompt } from "./parts";

/** Shows "All done" this long before the overlay closes by itself. */
const CLOSE_AFTER_DONE_MS = 700;

/** "https://time.example.com/x" → "time.example.com", "3-Personal/Work-Log.md" → "Work-Log". */
export function linkLabel(link: string): string {
  if (/^[a-z][\w+.-]*:\/\//i.test(link)) {
    try {
      return new URL(link).hostname.replace(/^www\./, "");
    } catch {
      return link;
    }
  }
  return link.split("/").pop()?.replace(/\.md$/, "") || link;
}

const unlisten = (subscription: Promise<() => void>) => void subscription.then((stop) => stop(), () => {});

/**
 * The routine as today's note has it, read again whenever the note or Settings change (ticks made in Obsidian
 * show), with its two actions. A tick shows at once; a failed one reads the note again.
 */
function useRoutine(id: string) {
  const [view, setView] = useState<RoutineView | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    const load = () => void api.routineViewToday(id).then(setView, (e) => setError(toAppError(e).message));
    load();
    const subscriptions = [api.onNoteChanged(load), api.onSettingsChanged(load)];
    return () => subscriptions.forEach(unlisten);
  }, [id]);

  const toggle = (step: RoutineStepView) => {
    const done = !step.done;
    setView((old) => old && { ...old, steps: old.steps.map((entry) => (entry.index === step.index ? { ...entry, done } : entry)) });
    api.routineStep(id, step.index, done).then(
      () => setError(null),
      (e) => {
        setError(`Couldn't update today's note: ${toAppError(e).message}`);
        void api.routineViewToday(id).then(setView, () => {});
      },
    );
  };
  const openLink = (step: RoutineStepView | undefined) => {
    if (step?.link) api.routineOpenLink(step.link).catch((e) => setError(toAppError(e).message));
  };
  return { view, error, toggle, openLink };
}

/** ↑↓ cycle through every item, ticked ones too; space or ↵ ticks; ⌘O opens the link. */
function useWizardKeys(steps: RoutineStepView[], current: RoutineStepView | undefined, actions: {
  select: (index: number) => void;
  toggle: (step: RoutineStepView) => void;
  openLink: (step: RoutineStepView | undefined) => void;
}) {
  const latest = useRef({ steps, current, actions });
  useEffect(() => {
    latest.current = { steps, current, actions };
  });
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const { steps, current, actions } = latest.current;
      if (e.isComposing || steps.length === 0) return;
      const position = Math.max(0, steps.findIndex((step) => step === current));
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        const next = (position + (e.key === "ArrowDown" ? 1 : -1) + steps.length) % steps.length;
        actions.select(steps[next].index);
      } else if ((e.key === " " || e.key === "Enter") && !(e.target instanceof HTMLButtonElement)) {
        e.preventDefault();
        actions.toggle(steps[position]);
      } else if (e.metaKey && e.key.toLowerCase() === "o") {
        e.preventDefault();
        actions.openLink(steps[position]);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}

function StepRow({ step, selected, onToggle, onSelect, onOpenLink }: {
  step: RoutineStepView;
  selected: boolean;
  onToggle: () => void;
  onSelect: () => void;
  onOpenLink: () => void;
}) {
  const label = step.link ? linkLabel(step.link) : "";
  return (
    <PanelOption selected={selected} aria-checked={step.done} data-done={step.done} onClick={onToggle} onMouseMove={onSelect}>
      <span className="wizard-check" aria-hidden="true">
        {step.done && <Check size={12} strokeWidth={3} />}
      </span>
      <span className="grow-text wizard-title">{step.title}</span>
      {step.link && (
        <button
          type="button"
          className="icon-btn"
          aria-label={`Open ${label}`}
          title={`Open ${label} (⌘O)`}
          onClick={(e) => {
            e.stopPropagation();
            onOpenLink();
          }}
        >
          <ExternalLink size={16} aria-hidden="true" />
        </button>
      )}
    </PanelOption>
  );
}

/**
 * A routine as a checklist to come back to: it opens on the first open item and closes by itself once the last
 * open item is ticked here; nothing is ever skipped.
 */
export function WizardView({ routine }: { routine: string }) {
  const { view, error, toggle, openLink } = useRoutine(routine);
  const [selected, setSelected] = useState<number | null>(null);
  const list = useRef<HTMLUListElement>(null);
  const hadOpenItems = useRef(false);

  const steps = view?.steps ?? [];
  const current = steps.find((step) => step.index === selected) ?? steps.find((step) => !step.done) ?? steps[0];
  const allDone = steps.length > 0 && steps.every((step) => step.done);
  const tick = (step: RoutineStepView) => {
    setSelected(step.index);
    toggle(step);
  };
  useWizardKeys(steps, current, { select: setSelected, toggle: tick, openLink });

  useEffect(() => {
    if (view) list.current?.focus();
  }, [view]);

  // A finished routine opened to look at stays open.
  useEffect(() => {
    if (view?.steps.some((step) => !step.done)) hadOpenItems.current = true;
  }, [view]);

  useEffect(() => {
    if (!allDone || !hadOpenItems.current) return;
    const timer = window.setTimeout(() => void api.panelClose(), CLOSE_AFTER_DONE_MS);
    return () => window.clearTimeout(timer);
  }, [allDone]);

  if (!view) {
    return error ? (
      <div className="panel-card">
        <PanelFooter error={error} hints={[CLOSE_HINT]} />
      </div>
    ) : null;
  }

  const doneCount = steps.filter((step) => step.done).length;
  return (
    <div className="panel-card wizard-view">
      <PanelPrompt className="wizard-heading">
        <span>{view.name}</span>
        <span className="o3 wizard-count">{allDone ? "All done" : `${doneCount} of ${steps.length}`}</span>
      </PanelPrompt>
      {steps.length === 0 ? (
        <p className="panel-body-text o3">Nothing due today.</p>
      ) : (
        <PanelList ref={list} className="wizard-list" aria-label={view.name} tabIndex={0}>
          {steps.map((step) => (
            <StepRow key={step.index} step={step} selected={step === current} onToggle={() => tick(step)} onSelect={() => setSelected(step.index)} onOpenLink={() => openLink(step)} />
          ))}
        </PanelList>
      )}
      <PanelFooter
        hints={[{ keys: ["↑", "↓"], label: "Select" }, { keys: ["space"], label: "Check" }, ...(current?.link ? [{ keys: ["⌘", "O"], label: "Open" }] : []), CLOSE_HINT]}
        error={error}
      />
    </div>
  );
}
