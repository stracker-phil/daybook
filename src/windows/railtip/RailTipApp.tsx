import { useEffect, useLayoutEffect, useReducer, useRef, useState, type CSSProperties } from "react";
import { Check, Volume2 } from "lucide-react";
import { api, type Choice, type RailTipContent } from "../../api";
import { KeyCap } from "../../components/Field";
import { shortcutKeys } from "../../lib/shortcuts";
import { useSettings } from "../../lib/useSettings";
import { useRailTheme } from "../rail/railLook";
import { elementAt } from "../rail/useRailLayout";

/** Entering: placed but not yet on screen, so it doesn't glide in from the last spot. */
type Phase = "hidden" | "entering" | "open";

interface TipState {
  content: RailTipContent | null;
  phase: Phase;
}

type TipAction = { type: "content"; content: RailTipContent } | { type: "placed"; sequence: number } | { type: "hidden" };

function reduce(state: TipState, action: TipAction): TipState {
  switch (action.type) {
    case "content":
      return { content: action.content, phase: state.phase === "open" ? "open" : "entering" };
    case "placed":
      return state.content?.sequence === action.sequence ? { ...state, phase: "open" } : state;
    case "hidden":
      return { ...state, phase: "hidden" };
  }
}

/** Keeps the card off the window's top and bottom edges. */
const EDGE = 8;
/** Long lists still show this much before they scroll. */
const MIN_CHOICES_HEIGHT = 160;
/** Room left for the header above the choices: its 8 px padding, the label and up to three lines of detail. */
const HEADER_ALLOWANCE = 96;
/** How far the card drifts toward the rail while it fades in or out. */
const DRIFT = 6;

const pick = (command: string) => () => void api.railTipRun(command);

/** The hovered choice, from the relayed pointer as in the rail (`usePointerHover`). */
function useRelayedHover(): string | null {
  const [hovered, setHovered] = useState<string | null>(null);
  useEffect(() => {
    const unlisten = api.onRailTipPointer((point) => {
      const row = elementAt(point, "[data-item]");
      setHovered(row?.dataset.item ?? null);
    });
    return () => void unlisten.then((fn) => fn()).catch(() => {});
  }, []);
  return hovered;
}

function ChoiceRow({ choice, checks, hovered }: { choice: Choice; checks: boolean; hovered: string | null }) {
  switch (choice.kind) {
    case "separator":
      return <div role="separator" className="railtip-separator" />;
    case "heading":
      return <div className="railtip-heading">{choice.label}</div>;
    case "steps":
      return (
        <div className="railtip-steps" role="group" aria-label={choice.label}>
          <Volume2 className="railtip-steps-icon" aria-hidden="true" />
          {choice.steps.map((step) => (
            <button
              key={step.command}
              type="button"
              className="railtip-step"
              aria-pressed={step.active}
              data-item={step.command}
              data-hover={hovered === step.command || undefined}
              onClick={pick(step.command)}
            >
              {step.label}
            </button>
          ))}
        </div>
      );
    case "item":
      return (
        <button
          type="button"
          className="railtip-item"
          role={choice.checked === null ? "menuitem" : "menuitemradio"}
          aria-checked={choice.checked ?? undefined}
          data-item={choice.command}
          data-hover={hovered === choice.command || undefined}
          onClick={pick(choice.command)}
        >
          {checks && <span className="railtip-check">{choice.checked && <Check aria-hidden="true" />}</span>}
          <span className="railtip-item-label">{choice.label}</span>
          {choice.detail && <span className="railtip-item-detail">{choice.detail}</span>}
        </button>
      );
  }
}

/** A tool's choices below the tooltip, in the same card; long lists scroll. */
function ChoiceList({ label, choices, maxHeight }: { label: string; choices: Choice[]; maxHeight: number }) {
  const hovered = useRelayedHover();
  // The check column only where a choice can be checked (contexts, playlists).
  const checks = choices.some((choice) => choice.kind === "item" && choice.checked !== null);
  return (
    <div role="menu" aria-label={label} className="railtip-choices" style={{ maxHeight }}>
      {choices.map((choice, i) => (
        <ChoiceRow key={i} choice={choice} checks={checks} hovered={hovered} />
      ))}
    </div>
  );
}

/**
 * The rail's tooltip, in a click-through column beside the rail as tall as the screen. One card carries the
 * label, shortcut and detail and, for some tools, their choices. It fades in beside the hovered button, glides
 * along as the pointer moves down the rail, and fades out toward it.
 */
export function RailTipApp() {
  const settings = useSettings();
  const [{ content, phase }, dispatch] = useReducer(reduce, { content: null, phase: "hidden" });
  useRailTheme(content?.theme ?? settings?.rail.theme, settings?.accent);
  const card = useRef<HTMLDivElement>(null);
  const header = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const unlisten = [
      api.onRailTipContent((next) => dispatch({ type: "content", content: next })),
      api.onRailTipPlaced((sequence) => dispatch({ type: "placed", sequence })),
      api.onRailTipHidden(() => dispatch({ type: "hidden" })),
    ];
    return () => unlisten.forEach((u) => void u.then((fn) => fn()).catch(() => {}));
  }, []);

  // Measured right after the commit, not after an animation frame: a hidden window gets none.
  // The header's middle sits beside the button; the card stays on screen.
  useLayoutEffect(() => {
    const box = card.current;
    if (!content || !box || !header.current) return;
    const height = box.offsetHeight;
    const middle = header.current.offsetTop + header.current.offsetHeight / 2;
    const top = Math.round(Math.max(EDGE, Math.min(content.anchor - middle, content.height - EDGE - height)));
    box.style.setProperty("--tip-y", `${top}px`);
    void api.railTipRendered(content.sequence, { x: box.offsetLeft, y: top, width: box.offsetWidth, height });
  }, [content]);

  if (!content) return null;
  const { tip } = content;
  const keys = shortcutKeys(tip.shortcut) ?? [];
  const choices = content.choices.length > 0 ? content.choices : null;
  const drift = phase === "open" ? 0 : content.side === "right" ? DRIFT : -DRIFT;
  const style = {
    [content.side === "right" ? "right" : "left"]: content.inset,
    "--tip-width": `${content.maxWidth}px`,
    transform: `translate3d(${drift}px, var(--tip-y, 0px), 0)`,
  } as CSSProperties;
  return (
    <div
      ref={card}
      className="railtip-card"
      role="tooltip"
      data-phase={phase}
      data-highlight={content.highlight}
      data-menu={choices ? true : undefined}
      style={style}
    >
      <div ref={header} className="railtip-header">
        <div className="railtip-title">
          <span className="railtip-label">{tip.label}</span>
          {keys.length > 0 && (
            <span className="railtip-keys">
              {keys.map((key) => (
                <KeyCap key={key} keys={key} />
              ))}
            </span>
          )}
        </div>
        {tip.detail && <div className="railtip-detail">{tip.detail}</div>}
      </div>
      {choices && <ChoiceList label={tip.label} choices={choices} maxHeight={Math.max(MIN_CHOICES_HEIGHT, content.height - 2 * EDGE - HEADER_ALLOWANCE)} />}
    </div>
  );
}
