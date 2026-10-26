import { describe, expect, it } from "vitest";
import { continueList, cycleCheckbox, indentLine, setLineProject } from "./focusText";

describe("cycleCheckbox", () => {
  it("moves a line's checkbox through in progress, done, cancelled and back to open", () => {
    let text = "- `[ ]` One";
    const marks: string[] = [];
    for (let i = 0; i < 4; i++) {
      text = cycleCheckbox(text, 4).text;
      marks.push(text.slice(4, 5));
    }
    expect(marks).toEqual(["/", "x", "-", " "]);
  });

  it("makes a line without a checkbox an open task and keeps the caret in the text", () => {
    expect(cycleCheckbox("plain", 2)).toEqual({ text: "- `[ ]` plain", caret: 10 });
  });

  it("turns a bullet into a task and a plain checkbox into the inline-code form", () => {
    expect(cycleCheckbox("- bullet", 8).text).toBe("- `[ ]` bullet");
    expect(cycleCheckbox("- [ ] One", 9).text).toBe("- [/] One");
  });

  it("changes only the line the caret is on", () => {
    expect(cycleCheckbox("- `[ ]` One\n- `[ ]` Two", 20).text).toBe("- `[ ]` One\n- `[/]` Two");
  });
});

describe("setLineProject", () => {
  it("starts an empty line as a task with the emoji", () => {
    expect(setLineProject("", 0, ":nexo:")).toEqual({ text: "- `[ ]` :nexo: ", caret: 15 });
  });

  it("swaps the emoji of a task and keeps its checkbox and text", () => {
    expect(setLineProject("- `[x]` :nexo: One\n- `[ ]` Two", 18, ":oscorp:")).toEqual({
      text: "- `[x]` :oscorp: One\n- `[ ]` Two",
      caret: 20,
    });
    expect(setLineProject("- `[/]` One", 11, ":nexo:").text).toBe("- `[/]` :nexo: One");
  });

  it("turns a plain checkbox into the inline-code form", () => {
    expect(setLineProject("- [/] One", 9, ":nexo:").text).toBe("- `[/]` :nexo: One");
  });

  it("makes a task with the same emoji a plain sub-item, and a task again with one more press", () => {
    const sub = setLineProject("- `[ ]` :nexo: Some content", 27, ":nexo:");
    expect(sub).toEqual({ text: "    - Some content", caret: 18 });
    expect(setLineProject(sub.text, sub.caret, ":nexo:").text).toBe("- `[ ]` :nexo: Some content");
  });
});

describe("continueList", () => {
  it("continues a task as an open task", () => {
    expect(continueList("- `[x]` One", 11)).toEqual({ text: "- `[x]` One\n- `[ ]` ", caret: 20 });
  });

  it("carries the line's emoji to the next task", () => {
    expect(continueList("- `[x]` :nexo: One", 18)).toEqual({
      text: "- `[x]` :nexo: One\n- `[ ]` :nexo: ",
      caret: 34,
    });
  });

  it("continues a sub-item as a sub-item", () => {
    expect(continueList("- `[ ]` One\n    - sub", 21)).toEqual({
      text: "- `[ ]` One\n    - sub\n    - ",
      caret: 28,
    });
  });

  it("ends the list on an item without text", () => {
    expect(continueList("- `[ ]` One\n- `[ ]` :nexo: ", 27)).toEqual({ text: "- `[ ]` One\n", caret: 12 });
  });

  it("leaves a plain line break to the field on a line that is no item", () => {
    expect(continueList("plain text", 10)).toBeNull();
  });

  it("leaves the line break to the field when the caret is not at the line's end", () => {
    expect(continueList("- `[ ]` One", 5)).toBeNull();
  });
});

describe("indentLine", () => {
  it("indents a line by four spaces", () => {
    expect(indentLine("- sub", 5, false)).toEqual({ text: "    - sub", caret: 9 });
  });

  it("turns an empty line into an indented sub-item", () => {
    expect(indentLine("- `[ ]` One\n", 12, false)).toEqual({ text: "- `[ ]` One\n    - ", caret: 18 });
  });

  it("outdents an indented line", () => {
    expect(indentLine("    - sub", 9, true)).toEqual({ text: "- sub", caret: 5 });
  });

  it("leaves a line at the margin where it is when outdenting", () => {
    expect(indentLine("- sub", 3, true)).toEqual({ text: "- sub", caret: 3 });
  });
});
