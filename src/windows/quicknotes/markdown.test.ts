import { describe, expect, it } from "vitest";
import { toggleTask, toggleWrap, type Edit } from "./markdown";

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
