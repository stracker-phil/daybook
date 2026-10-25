import { describe, expect, it } from "vitest";
import { asShortcode, emojiGlyph, emojiName } from "./slackEmoji";

describe("emojiName", () => {
  it.each([
    [":coffee:", "coffee"],
    ["coffee", "coffee"],
    [":coffee", "coffee"],
    ["  :coffee:  ", "coffee"],
    ["", ""],
  ])("turns %j into %j", (code, name) => {
    expect(emojiName(code)).toBe(name);
  });
});

describe("emojiGlyph", () => {
  it("gives the glyph of a known shortcode", () => {
    expect(emojiGlyph(":white_check_mark:")).toBe("✅");
  });

  it("accepts the name without colons", () => {
    expect(emojiGlyph("coffee")).toBe("☕");
  });

  it.each([":my_custom_emoji:", ""])("has no glyph for %j", (code) => {
    expect(emojiGlyph(code)).toBeUndefined();
  });
});

describe("asShortcode", () => {
  it.each([
    ["coffee", ":coffee:"],
    [":coffee", ":coffee:"],
    ["coffee:", ":coffee:"],
    [":coffee:", ":coffee:"],
    ["  coffee ", ":coffee:"],
  ])("turns %j into %j", (text, code) => {
    expect(asShortcode(text)).toBe(code);
  });

  it.each(["", "  ", "::"])("keeps %j empty", (text) => {
    expect(asShortcode(text)).toBe("");
  });
});
