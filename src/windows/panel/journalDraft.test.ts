import { describe, expect, it } from "vitest";
import type { JournalAnswer, JournalGroupAnswers } from "../../api";
import { contentKey, withoutTrailingRow, withTrailingRow } from "./journalDraft";

const answer = (patch: Partial<JournalAnswer> = {}): JournalAnswer => ({ items: [], tasks: [], text: "", ...patch });
const draft = (patch: Partial<JournalAnswer> = {}, note = ""): JournalGroupAnswers => ({ answers: { Q: answer(patch) }, note });

describe("contentKey", () => {
  it.each<[string, JournalGroupAnswers, JournalGroupAnswers]>([
    ["trailing spaces in an item", draft({ items: ["Sunshine "] }), draft({ items: ["Sunshine"] })],
    ["an empty item row", draft({ items: ["Sunshine", ""] }), draft({ items: ["Sunshine"] })],
    ["trailing spaces in a text", draft({ text: "Hallo \nWelt  " }), draft({ text: "Hallo\nWelt" })],
    ["an empty task", draft({ tasks: [{ text: "A", done: false }, { text: "", done: false }] }), draft({ tasks: [{ text: "A", done: false }] })],
    ["trailing spaces in the note", draft({}, "Note  "), draft({}, "Note")],
  ])("treats drafts as equal when they differ only by %s", (_, a, b) => {
    expect(contentKey(a)).toBe(contentKey(b));
  });

  it.each<[string, JournalGroupAnswers, JournalGroupAnswers]>([
    ["an item's text", draft({ items: ["Sunshine"] }), draft({ items: ["Regen"] })],
    ["a text answer", draft({ text: "a" }), draft({ text: "b" })],
    ["a task's done flag", draft({ tasks: [{ text: "A", done: false }] }), draft({ tasks: [{ text: "A", done: true }] })],
    ["the note", draft({}, "a"), draft({}, "b")],
  ])("treats drafts as different when %s differs", (_, a, b) => {
    expect(contentKey(a)).not.toBe(contentKey(b));
  });
});

describe("withTrailingRow", () => {
  const isEmpty = (row: string) => row === "";

  it("appends an empty row when the last row has content", () => {
    expect(withTrailingRow(["a"], isEmpty, "")).toEqual(["a", ""]);
  });

  it("gives an empty list one empty row", () => {
    expect(withTrailingRow([], isEmpty, "")).toEqual([""]);
  });

  it("keeps rows that already end in an empty row", () => {
    expect(withTrailingRow(["a", ""], isEmpty, "")).toEqual(["a", ""]);
  });
});

describe("withoutTrailingRow", () => {
  const isEmpty = (row: string) => row === "";

  it("drops the trailing empty row", () => {
    expect(withoutTrailingRow(["a", ""], isEmpty)).toEqual(["a"]);
  });

  it("keeps empty rows in between", () => {
    expect(withoutTrailingRow(["a", "", "b", ""], isEmpty)).toEqual(["a", "", "b"]);
  });
});
