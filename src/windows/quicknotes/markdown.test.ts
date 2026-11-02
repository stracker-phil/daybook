import { describe, expect, it } from "vitest";
import { textChange, toggleTask, toggleWrap, type Edit } from "./markdown";

const apply = (value: string, edit: Edit) => value.slice(0, edit.from) + edit.text + value.slice(edit.to);

describe("toggleWrap", () => {
  it("wraps the selection and selects the same text inside the markers", () => {
    const edit = toggleWrap("say hi", 4, 6, "**");
    expect(apply("say hi", edit)).toBe("say **hi**");
    expect(edit.select).toEqual([6, 8]);
  });

  it.each([
    ["the text inside the markers", 6, 8],
    ["the text with its markers", 4, 10],
  ])("unwraps a selection that covers %s", (_, start, end) => {
    expect(apply("say **hi**", toggleWrap("say **hi**", start, end, "**"))).toBe("say hi");
  });

  it("tells italic from bold", () => {
    expect(apply("**hi**", toggleWrap("**hi**", 2, 4, "*"))).toBe("***hi***");
    expect(apply("*hi*", toggleWrap("*hi*", 1, 3, "*"))).toBe("hi");
  });

  it("puts empty markers around the caret", () => {
    const edit = toggleWrap("ab", 1, 1, "*");
    expect(apply("ab", edit)).toBe("a**b");
    expect(edit.select).toEqual([2, 2]);
  });
});

describe("textChange", () => {
  it("is null for equal texts", () => {
    expect(textChange("same", "same")).toBeNull();
    expect(textChange("", "")).toBeNull();
  });

  it.each([
    ["an insertion in the middle", "hello world", "hello big world", { from: 6, to: 6, insert: "big " }],
    ["an insertion at the start", "world", "hello world", { from: 0, to: 0, insert: "hello " }],
    ["an insertion at the end", "hello", "hello world", { from: 5, to: 5, insert: " world" }],
    ["a deletion in the middle", "hello big world", "hello world", { from: 6, to: 10, insert: "" }],
    ["a deletion at the start", "hello world", "world", { from: 0, to: 6, insert: "" }],
    ["a deletion at the end", "hello world", "hello", { from: 5, to: 11, insert: "" }],
    ["a replacement in the middle", "a cat sat", "a dog sat", { from: 2, to: 5, insert: "dog" }],
    ["a replacement of everything", "abc", "xyz", { from: 0, to: 3, insert: "xyz" }],
    ["a text growing from empty", "", "new", { from: 0, to: 0, insert: "new" }],
    ["a text cleared", "old", "", { from: 0, to: 3, insert: "" }],
  ])("gives the smallest range for %s", (_, before, after, expected) => {
    const change = textChange(before, after)!;
    expect(change).toEqual(expected);
    expect(before.slice(0, change.from) + change.insert + before.slice(change.to)).toBe(after);
  });

  it("does not let the common start and end overlap in repeated text", () => {
    const change = textChange("aaa", "aaaa")!;
    expect("aaa".slice(0, change.from) + change.insert + "aaa".slice(change.to)).toBe("aaaa");
    expect(change.to - change.from).toBe(0);
    expect(change.insert).toBe("a");
  });
});

describe("toggleTask", () => {
  it("cycles a line through todo, done and open", () => {
    let value = "buy milk";
    value = apply(value, toggleTask(value, 3, 3));
    expect(value).toBe("- [ ] buy milk");
    value = apply(value, toggleTask(value, 8, 8));
    expect(value).toBe("- [x] buy milk");
    value = apply(value, toggleTask(value, 8, 8));
    expect(value).toBe("- [ ] buy milk");
  });

  it("adds a checkbox to a list item and keeps the caret on the text", () => {
    const edit = toggleTask("a\n  - item\nb", 6, 6);
    expect(apply("a\n  - item\nb", edit)).toBe("a\n  - [ ] item\nb");
    expect(edit.select).toEqual([10, 10]);
  });

  it("changes every non-empty selected line", () => {
    const value = "one\n\ntwo";
    expect(apply(value, toggleTask(value, 0, value.length))).toBe("- [ ] one\n\n- [ ] two");
  });
});
