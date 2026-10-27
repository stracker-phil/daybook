import type { KeyboardEvent } from "react";
import type { ActivitySuggestion } from "../../api";
import { cycle } from "./listKeys";
import { MAX_PROJECT_KEYS } from "./parts";

const STEPS: Record<string, number> = { ArrowDown: 1, ArrowUp: -1 };

/** The digit of ⌘N (`meta`) or ⌥N (`alt`) alone, so other combinations stay free; else null. */
function digitWith(e: KeyboardEvent, modifier: "meta" | "alt"): number | null {
  const held = modifier === "meta" ? e.metaKey && !e.altKey : e.altKey && !e.metaKey;
  if (!held || e.ctrlKey) return null;
  const digit = /^Digit([0-9])$/.exec(e.code)?.[1];
  return digit === undefined ? null : Number(digit);
}

interface ActivityKeys {
  items: ActivitySuggestion[];
  /** The highlighted suggestion, -1 for your own input. */
  current: number;
  /** The Daybook projects' names, in picker order. */
  projects: string[];
  project: string;
  /** The timer row's ids, "None" (`null`) first; empty without the row. */
  timers: (string | null)[];
  highlight: (index: number) => void;
  pickProject: (name: string) => void;
  pickTimer: (id: string | null) => void;
  submit: () => void;
}

/** The note field's keys: ↑↓ suggestions, ⌘1–9 and Tab the project, ⌥0–9 the timer, ↵ submits. */
export function useActivityKeys({ items, current, projects, project, timers, highlight, pickProject, pickTimer, submit }: ActivityKeys) {
  return (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.nativeEvent.isComposing) return;
    const step = STEPS[e.key];
    const projectDigit = digitWith(e, "meta");
    const timerDigit = digitWith(e, "alt");
    if (step) {
      e.preventDefault();
      // Your input sits before the suggestions as position -1, so the cycle runs over one more place.
      if (items.length > 0) highlight(cycle(current + 1, step, items.length + 1) - 1);
    } else if (projectDigit !== null) {
      e.preventDefault();
      if (projectDigit > 0 && projectDigit <= Math.min(MAX_PROJECT_KEYS, projects.length)) pickProject(projects[projectDigit - 1]);
    } else if (timerDigit !== null) {
      // ⌥N would type a character, so it never reaches the note.
      e.preventDefault();
      if (timerDigit < timers.length) pickTimer(timers[timerDigit]);
    } else if (e.key === "Tab") {
      // Tab never leaves the overlay.
      e.preventDefault();
      if (projects.length < 2) return;
      const at = projects.indexOf(project);
      pickProject(projects[at < 0 ? 0 : cycle(at, e.shiftKey ? -1 : 1, projects.length)]);
    } else if (e.key === "Enter") {
      e.preventDefault();
      submit();
    }
  };
}
