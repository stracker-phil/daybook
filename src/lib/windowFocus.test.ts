import { afterEach, describe, expect, it, vi } from "vitest";
import { trackWindowFocus } from "./windowFocus";

const blurred = () => document.documentElement.classList.contains("window-blurred");

describe("trackWindowFocus", () => {
  afterEach(() => vi.restoreAllMocks());

  it("marks the page blurred at once when the window is not key", () => {
    vi.spyOn(document, "hasFocus").mockReturnValue(false);
    const stop = trackWindowFocus();
    expect(blurred()).toBe(true);
    stop();
  });

  it("follows the window as it loses and regains focus", () => {
    const hasFocus = vi.spyOn(document, "hasFocus").mockReturnValue(true);
    const stop = trackWindowFocus();
    expect(blurred()).toBe(false);
    hasFocus.mockReturnValue(false);
    window.dispatchEvent(new Event("blur"));
    expect(blurred()).toBe(true);
    hasFocus.mockReturnValue(true);
    window.dispatchEvent(new Event("focus"));
    expect(blurred()).toBe(false);
    stop();
  });

  it("clears the mark and stops following when cleaned up", () => {
    const hasFocus = vi.spyOn(document, "hasFocus").mockReturnValue(false);
    const stop = trackWindowFocus();
    stop();
    expect(blurred()).toBe(false);
    hasFocus.mockReturnValue(false);
    window.dispatchEvent(new Event("blur"));
    expect(blurred()).toBe(false);
  });
});
