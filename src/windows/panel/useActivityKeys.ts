import type { KeyboardEvent } from "react";
import type { ActivitySuggestion, ActivityTtProject } from "../../api";
import { MAX_PROJECT_KEYS } from "./ActivityParts";
import { cycle } from "./listKeys";

const STEPS: Record<string, number> = { ArrowDown: 1, ArrowUp: -1 };

/** ⌘1…⌘9 as 1…9, else 0; ⌘ alone, so ⌃ and ⌥ combinations stay free. */
function projectDigit(e: KeyboardEvent): number {
  if (!e.metaKey || e.ctrlKey || e.altKey) return 0;
  const digit = Number(/^Digit([1-9])$/.exec(e.code)?.[1] ?? 0);
  return digit <= MAX_PROJECT_KEYS ? digit : 0;
}

interface ActivityKeys {
  items: ActivitySuggestion[];
  /** The highlighted suggestion, -1 for your own input. */
  current: number;
  choices: ActivityTtProject[];
  ttProjectId: string;
  highlight: (index: number) => void;
  pickProject: (id: string) => void;
  submit: () => void;
}

/** The note field's keys: ↑↓ suggestions, ⌘1–9 and Tab the project, ↵ submits. */
export function useActivityKeys({ items, current, choices, ttProjectId, highlight, pickProject, submit }: ActivityKeys) {
  return (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.nativeEvent.isComposing) return;
    const step = STEPS[e.key];
    const digit = projectDigit(e);
    if (step) {
      e.preventDefault();
      // Your input sits before the suggestions as position -1, so the cycle runs over one more place.
      if (items.length > 0) highlight(cycle(current + 1, step, items.length + 1) - 1);
    } else if (digit) {
      e.preventDefault();
      const choice = choices[digit - 1];
      if (choice) pickProject(choice.id);
    } else if (e.key === "Tab") {
      // Tab never leaves the overlay.
      e.preventDefault();
      if (choices.length < 2) return;
      const at = choices.findIndex((choice) => choice.id === ttProjectId);
      pickProject(choices[at < 0 ? 0 : cycle(at, e.shiftKey ? -1 : 1, choices.length)].id);
    } else if (e.key === "Enter") {
      e.preventDefault();
      submit();
    }
  };
}
