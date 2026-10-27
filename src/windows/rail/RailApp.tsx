import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { api, type RailSide } from "../../api";
import { useSettings } from "../../lib/useSettings";
import { RailButton } from "./RailButton";
import { useRailTheme } from "./railLook";
import { usePointerHover, useRailModel, useRailShown, useReportHeight, type HoverBox } from "./useRailLayout";

const IDLE_AFTER_MS = 1500;

/** A rail that stays (shortcut, meeting) fades back once the pointer has been away a moment. */
function useIdle(active: boolean, hovered: boolean): boolean {
  const [idle, setIdle] = useState(false);
  useEffect(() => {
    setIdle(false);
    if (!active || hovered) return;
    const timer = window.setTimeout(() => setIdle(true), IDLE_AFTER_MS);
    return () => window.clearTimeout(timer);
  }, [active, hovered]);
  return idle;
}

/** Opening on the other edge jumps the bar off-screen first, then slides it in. */
function useSideSwitch(side: RailSide, bar: HTMLElement | null): boolean {
  const [placing, setPlacing] = useState(false);
  const last = useRef(side);
  useLayoutEffect(() => {
    if (last.current === side) return;
    last.current = side;
    setPlacing(true);
  }, [side]);
  useLayoutEffect(() => {
    if (!placing) return;
    // Applies the off-screen spot before the slide starts from it.
    void bar?.offsetWidth;
    setPlacing(false);
  }, [placing, bar]);
  return placing;
}

/** One highlight behind the buttons that glides to the hovered one; coming from none it fades in in place. */
function HoverHighlight({ box }: { box: HoverBox | null }) {
  const last = useRef<HoverBox | null>(null);
  const wasShown = useRef(false);
  const appearing = !!box && !wasShown.current;
  const at = box ?? last.current;
  useEffect(() => {
    if (box) last.current = box;
    wasShown.current = !!box;
  });
  if (!at) return null;
  return (
    <div
      aria-hidden="true"
      className="rail-highlight"
      data-appearing={appearing || undefined}
      style={{ transform: `translate3d(${at.x}px, ${at.y}px, 0)`, width: at.width, height: at.height, opacity: box ? 1 : 0 }}
    />
  );
}

type Motion = "placing" | "hidden" | "idle" | "shown";

function motionOf(placing: boolean, visible: boolean, idle: boolean): Motion {
  if (placing) return "placing";
  if (!visible) return "hidden";
  return idle ? "idle" : "shown";
}

/**
 * A bar of tools at the screen edge, or a column of floating icons. It never takes focus: Rust relays the hover,
 * a click runs the button's command, a right-click shows the menu-bar menu. It slides in from its edge and back out
 * before the window hides.
 */
export function RailApp() {
  const accent = useSettings()?.accent;
  const model = useRailModel();
  useRailTheme(model?.settings.theme, accent);
  const { visible, side, sticky } = useRailShown();
  const [wrapper, setWrapper] = useState<HTMLDivElement | null>(null);
  const [bar, setBar] = useState<HTMLElement | null>(null);
  const { tool: hover, box } = usePointerHover(bar, model?.settings.tooltipDelay ?? 0);
  const idle = useIdle(!!model?.settings.fadeWhenIdle && sticky && visible, hover !== null);
  const placing = useSideSwitch(side, bar);
  useReportHeight(wrapper);

  if (!model) return null;
  const look = model.settings;
  return (
    <div
      className="rail-page"
      onContextMenu={(e) => {
        e.preventDefault();
        void api.railContextMenu();
      }}
    >
      <div ref={setWrapper} className="rail-wrapper" data-shape={look.shape} data-side={side}>
        <nav
          ref={setBar}
          aria-label="Rail"
          className="rail-bar"
          data-shape={look.shape}
          data-size={look.size}
          data-highlight={look.highlight}
          data-side={side}
          data-motion={motionOf(placing, visible, idle)}
        >
          {look.shape !== "floating" && <HoverHighlight box={box} />}
          {model.buttons.map((button, i) =>
            button.kind === "separator" ? (
              <div key={`separator-${i}`} role="separator" className="rail-divider" />
            ) : (
              <RailButton key={button.id} tool={button} status={model.status} hovered={button.id === hover} captions={look.captions} />
            ),
          )}
        </nav>
      </div>
    </div>
  );
}
