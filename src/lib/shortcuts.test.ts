import { describe, expect, it } from "vitest";
import { findShortcutConflict, shortcutFromKeyEvent, shortcutKeys } from "./shortcuts";

const press = (code: string, modifiers: Partial<Record<"metaKey" | "ctrlKey" | "altKey" | "shiftKey", boolean>> = {}) => ({
  code,
  metaKey: false,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  ...modifiers,
});

describe("shortcutFromKeyEvent", () => {
  it.each([
    ["the physical key under ⌥, not the typed character", press("KeyK", { altKey: true }), "alt+k"],
    ["modifiers in the usual order", press("KeyK", { metaKey: true, shiftKey: true }), "shift+cmd+k"],
    ["a function key with ⌘", press("F1", { metaKey: true }), "cmd+f1"],
    ["a bare function key", press("F12"), "f12"],
    ["punctuation by its code", press("Comma", { metaKey: true }), "cmd+,"],
    ["a bare letter", press("KeyH"), null],
    ["⇧ alone with a letter", press("KeyH", { shiftKey: true }), null],
    ["a bare modifier", press("MetaLeft", { metaKey: true }), null],
  ])("reads %s", (_name, event, expected) => {
    expect(shortcutFromKeyEvent(event)).toBe(expected);
  });
});

describe("findShortcutConflict", () => {
  const taken = [{ shortcut: "CMD+Shift+K", label: "Search" }];

  it("finds a shortcut taken by another action, whatever the spelling", () => {
    expect(findShortcutConflict("shift+cmd+k", taken)?.label).toBe("Search");
  });

  it("finds nothing for a different shortcut", () => {
    expect(findShortcutConflict("cmd+k", taken)).toBeUndefined();
  });

  it("finds nothing for no shortcut", () => {
    expect(findShortcutConflict(undefined, taken)).toBeUndefined();
  });
});

describe("shortcutKeys", () => {
  it.each([
    ["all modifiers in the usual order", "ctrl+alt+cmd+j", ["⌃", "⌥", "⌘", "J"]],
    ["a named key as a glyph", "cmd+enter", ["⌘", "↩"]],
  ])("shows %s", (_name, shortcut, expected) => {
    expect(shortcutKeys(shortcut)).toEqual(expected);
  });

  it("shows nothing for an empty or missing shortcut", () => {
    expect(shortcutKeys("")).toBeUndefined();
    expect(shortcutKeys(null)).toBeUndefined();
  });
});
