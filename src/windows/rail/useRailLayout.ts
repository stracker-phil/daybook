import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { api, type RailModel, type RailPoint, type RailShown, type ToolId } from "../../api";
import { usePushed } from "../../lib/usePushed";

const HIDDEN: RailShown = { visible: false, side: "right", sticky: false };

export const useRailModel = () => usePushed<RailModel | null>(api.railModel, api.onRailModel, null);

/** `visible` turns false while Rust slides the rail out, true once it is shown. */
export const useRailShown = () => usePushed(api.railShownState, api.onRailShown, HIDDEN);

/** The hovered button's box inside the bar's border; the highlight glides to it. */
export interface HoverBox {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface Hover {
  tool: ToolId | null;
  box: HoverBox | null;
}

const NO_HOVER: Hover = { tool: null, box: null };

/** Layout offsets ignore transforms, so the slide and the hover scale never move the box. */
function offsetBox(el: HTMLElement, bar: HTMLElement): HoverBox {
  let x = 0;
  let y = 0;
  for (let node: HTMLElement | null = el; node && node !== bar; node = node.offsetParent as HTMLElement | null) {
    x += node.offsetLeft;
    y += node.offsetTop;
  }
  return { x, y, width: el.offsetWidth, height: el.offsetHeight };
}

/** The element matching `selector` under a relayed pointer, `null` off one. */
export function elementAt(point: RailPoint | null, selector: string): HTMLElement | null {
  return point ? (document.elementFromPoint(point.x, point.y)?.closest<HTMLElement>(selector) ?? null) : null;
}

/** Like native tooltips: the first one after `tooltipDelay`, then the next ones at once while one shows. */
function useTooltipScheduler(tooltipDelay: number) {
  const shown = useRef(false);
  const timer = useRef<number | undefined>(undefined);
  const delay = useRef(tooltipDelay);
  useEffect(() => {
    delay.current = tooltipDelay;
  });
  const show = useCallback((tool: ToolId, center: number) => {
    window.clearTimeout(timer.current);
    const open = () => {
      shown.current = true;
      void api.railShowTip(tool, center);
    };
    if (shown.current || delay.current <= 0) open();
    else timer.current = window.setTimeout(open, delay.current);
  }, []);
  const hide = useCallback(() => {
    window.clearTimeout(timer.current);
    if (!shown.current) return;
    shown.current = false;
    void api.railHideTip();
  }, []);
  return useMemo(() => ({ show, hide }), [show, hide]);
}

/**
 * The rail never becomes key, so WebKit sends it no hover: Rust relays the pointer (`rail_pointer`) and this finds
 * the button under it, returns it with its box and asks for its tooltip.
 */
export function usePointerHover(bar: HTMLElement | null, tooltipDelay: number): Hover {
  const [hover, setHover] = useState<Hover>(NO_HOVER);
  const hovered = useRef<HTMLElement | null>(null);
  const barRef = useRef(bar);
  useEffect(() => {
    barRef.current = bar;
  });
  const tip = useTooltipScheduler(tooltipDelay);

  useEffect(() => {
    const onPointer = (point: RailPoint | null) => {
      const target = elementAt(point, "[data-tool]");
      if (target === hovered.current) return;
      hovered.current = target;
      const owner = barRef.current;
      if (!target || !owner) {
        setHover(NO_HOVER);
        tip.hide();
        return;
      }
      const tool = target.dataset.tool as ToolId;
      setHover({ tool, box: offsetBox(target, owner) });
      const rect = target.getBoundingClientRect();
      tip.show(tool, rect.top + rect.height / 2);
    };
    const unlisten = api.onRailPointer(onPointer);
    return () => {
      void unlisten.then((fn) => fn()).catch(() => {});
      tip.hide();
    };
  }, [tip]);
  return hover;
}

/**
 * The rail window is as tall as the bar. WebKit runs no ResizeObserver callbacks while the window is hidden (a
 * context switch can change the bar meanwhile), so every render measures as well.
 */
export function useReportHeight(element: HTMLElement | null) {
  const reported = useRef(0);
  const report = useCallback(() => {
    if (!element) return;
    const height = Math.ceil(element.getBoundingClientRect().height);
    if (height === reported.current) return;
    reported.current = height;
    void api.railSetHeight(height);
  }, [element]);
  useLayoutEffect(report);
  useEffect(() => {
    if (!element) return;
    const observer = new ResizeObserver(report);
    observer.observe(element);
    return () => observer.disconnect();
  }, [element, report]);
}
