import { describe, expect, it } from "vitest";
import { parsePrefix, score } from "./parse";

describe("parsePrefix", () => {
  it.each([
    [">x", ">", "x"],
    ["p: a", null, "p: a"],
    ["  >x", ">", "x"],
    [" foo ", null, "foo"],
    ["", null, ""],
    [">", ">", ""],
  ])("splits %j into prefix %j and text %j", (input, prefix, needle) => {
    expect(parsePrefix(input)).toEqual({ prefix, needle });
  });
});

describe("score", () => {
  it("ranks equal over prefix over word start over inside over none", () => {
    const ranks = ["Reports", "Report card", "Copy report as text", "Unreported", "Zebra"].map((field) => score([field], "report"));
    expect(ranks).toEqual([4, 4, 3, 2, 0]);
    expect(score(["Report"], "report")).toBe(5);
    expect(score(["Reports"], "rep")).toBe(4);
    expect(score(["Copy report as text"], "rep")).toBe(3);
    expect(score(["Prepare"], "rep")).toBe(2);
  });

  it("ignores case", () => {
    expect(score(["REPORTS"], "rep")).toBe(4);
  });

  it("takes the best of several fields", () => {
    expect(score(["Prepare", "Reports"], "rep")).toBe(4);
  });

  it("matches everything with 1 when the text is empty", () => {
    expect(score(["Anything"], "")).toBe(1);
  });
});
