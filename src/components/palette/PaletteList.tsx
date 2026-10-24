import { Fragment, useEffect, type RefObject } from "react";
import { CornerDownLeft } from "lucide-react";
import { KeyCap } from "../Field";
import type { Item } from "./usePaletteItems";

interface Props {
  items: Item[];
  active: number;
  activeRef: RefObject<HTMLLIElement | null>;
  onHover: (i: number) => void;
  onChoose: (i: number) => void;
}

/** The results under a label per section; the active row carries the accent bar and ↵. */
export function PaletteList({ items, active, activeRef, onHover, onChoose }: Props) {
  useEffect(() => {
    activeRef.current?.scrollIntoView?.({ block: "nearest" });
  }, [active, items, activeRef]);

  return (
    <ul className="palette-list" role="listbox" aria-label="Results">
      {items.map((it, i) => {
        const header = it.section !== items[i - 1]?.section ? it.section : null;
        return (
          <Fragment key={it.key}>
            {header && (
              <li role="presentation" className="palette-section">
                {header}
              </li>
            )}
            <li
              ref={i === active ? activeRef : undefined}
              role="option"
              aria-selected={i === active}
              className={i === active ? "active" : ""}
              onMouseEnter={() => onHover(i)}
              onClick={() => onChoose(i)}
            >
              <span className="grow-text">
                {it.title}
              </span>
              <KeyCap keys={it.keys ?? ""} />
              {i === active && <CornerDownLeft size="0.923rem" className="muted" />}
            </li>
          </Fragment>
        );
      })}
      {items.length === 0 && <li className="muted">No matches.</li>}
    </ul>
  );
}

const PREFIX_HINTS = "> commands only";

/** The palette's keys; the prefix while nothing is typed. */
export function PaletteFooter({ empty }: { empty: boolean }) {
  const keys = "↵ open · esc close";
  return (
    <div className="palette-foot o3">
      <span>{keys}</span>
      {empty && <span>{PREFIX_HINTS}</span>}
    </div>
  );
}
