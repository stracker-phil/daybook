import { render } from "@testing-library/react";
import { createElement } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_PRESENTATION_FONT, fitFontSize, type PresentationFont, usePresentationFont } from "./presentation";

describe("usePresentationFont", () => {
  const font: PresentationFont = { max: 80, min: 40 };
  const SIZE = "--presentation-font-size";

  function Harness({ font }: { font: PresentationFont }) {
    const { box, mirror } = usePresentationFont({ text: "Hello", active: true }, font);
    return createElement(
      "div",
      null,
      createElement("div", { ref: box }, createElement("div", { className: "cm-scroller" })),
      createElement("div", { ref: mirror }),
    );
  }

  /** The mirror always fits; the editor overflows once the box's font size exceeds `overflowAbove`. */
  function stubLayout(overflowAbove: number) {
    const editorSize = (el: HTMLElement) =>
      Number.parseFloat(el.parentElement?.style.getPropertyValue(SIZE) || "0");
    const define = (name: string, get: (el: HTMLElement) => number) =>
      Object.defineProperty(HTMLElement.prototype, name, {
        configurable: true,
        get(this: HTMLElement) {
          return get(this);
        },
      });
    const isEditor = (el: HTMLElement) => el.classList.contains("cm-scroller");
    define("scrollHeight", (el) => (isEditor(el) ? (editorSize(el) > overflowAbove ? 200 : 50) : 0));
    define("clientHeight", () => 100);
    define("scrollWidth", () => 0);
    define("clientWidth", () => 100);
  }

  afterEach(() => {
    for (const name of ["scrollHeight", "clientHeight", "scrollWidth", "clientWidth"]) {
      Reflect.deleteProperty(HTMLElement.prototype, name);
    }
  });

  function renderedBox() {
    const { container } = render(createElement(Harness, { font }));
    return container.firstElementChild?.firstElementChild as HTMLElement;
  }

  it("steps the size down until the real editor stops overflowing", async () => {
    stubLayout(60);
    const box = renderedBox();

    await vi.waitFor(() => expect(box.style.getPropertyValue(SIZE)).toBe("60px"));
  });

  it("keeps the mirror's size when the editor does not overflow", async () => {
    stubLayout(1000);
    const box = renderedBox();

    await vi.waitFor(() => expect(box.style.getPropertyValue(SIZE)).toBe("80px"));
  });

  it("never goes below the minimum when the editor overflows at every size", async () => {
    stubLayout(0);
    const box = renderedBox();

    await vi.waitFor(() => expect(box.style.getPropertyValue(SIZE)).toBe("40px"));
  });
});

describe("fitFontSize", () => {
  const font = { max: 100, min: 20 };

  it("returns the maximum when every size fits", () => {
    expect(fitFontSize(font, () => true)).toBe(100);
  });

  it("returns the minimum when no size fits", () => {
    expect(fitFontSize(font, () => false)).toBe(20);
  });

  it.each([21, 37, 64, 99])("returns the largest size that fits when sizes up to %i fit", (threshold) => {
    expect(fitFontSize(font, (size) => size <= threshold)).toBe(threshold);
  });

  it("returns a whole number within the range for fractional limits", () => {
    const size = fitFontSize({ max: 90.8, min: 20.2 }, (s) => s <= 55.5);
    expect(Number.isInteger(size)).toBe(true);
    expect(size).toBe(55);
    expect(fitFontSize({ max: 90.8, min: 20.2 }, () => true)).toBe(90);
    expect(fitFontSize({ max: 90.8, min: 20.2 }, () => false)).toBe(21);
  });

  it("honours another font range", () => {
    const other = { max: 80, min: 20 };
    expect(fitFontSize(other, () => true)).toBe(80);
    expect(fitFontSize(other, () => false)).toBe(20);
    expect(fitFontSize(other, (size) => size <= 50)).toBe(50);
  });

  it("returns the minimum when the range is a single size", () => {
    expect(fitFontSize({ max: 30, min: 30 }, () => true)).toBe(30);
  });

  it("uses a range of 42 to 156 by default", () => {
    expect(DEFAULT_PRESENTATION_FONT).toEqual({ max: 156, min: 42 });
    expect(fitFontSize(DEFAULT_PRESENTATION_FONT, () => true)).toBe(156);
    expect(fitFontSize(DEFAULT_PRESENTATION_FONT, () => false)).toBe(42);
  });
});
