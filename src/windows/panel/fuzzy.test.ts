import { describe, expect, it } from "vitest";
import { fuzzyFilter } from "./fuzzy";

describe("fuzzyFilter", () => {
  it("keeps every item for an empty or blank query", () => {
    expect(fuzzyFilter(["b", "a"], "")).toEqual(["b", "a"]);
    expect(fuzzyFilter(["b", "a"], "  ")).toEqual(["b", "a"]);
  });

  it("ranks a match at the start before one in the middle before scattered letters", () => {
    expect(fuzzyFilter(["xxabcxx", "a-x-b-x-c", "abcxx"], "abc")).toEqual(["abcxx", "xxabcxx", "a-x-b-x-c"]);
  });

  it("drops items that lack the letters", () => {
    expect(fuzzyFilter(["QRS-1", "ABC-2"], "qrs")).toEqual(["QRS-1"]);
  });

  it("matches regardless of case", () => {
    expect(fuzzyFilter(["QRS-6805"], "qrs")).toEqual(["QRS-6805"]);
  });

  it("requires every word to match", () => {
    expect(fuzzyFilter(["qrs 12", "qrs 99", "abc 12"], "qrs 12")).toEqual(["qrs 12"]);
  });

  it("keeps the original order of equally good matches", () => {
    expect(fuzzyFilter(["ab2", "ab1", "ab3"], "ab")).toEqual(["ab2", "ab1", "ab3"]);
  });
});
