import { describe, expect, it } from "vitest";
import { centerOf, intersectRect, isInView, visiblePart } from "./rect";

const list = { x: 0, y: 100, width: 300, height: 200 };
const page = { x: 0, y: 0, width: 800, height: 600 };
const rect = (x: number, y: number, width: number, height: number) => ({ x, y, width, height });

describe("intersectRect", () => {
  it("returns the overlap of two rectangles", () => {
    expect(intersectRect(rect(0, 0, 10, 10), rect(5, 5, 10, 10))).toEqual(rect(5, 5, 5, 5));
  });

  it.each([
    ["touch only at an edge", rect(10, 0, 5, 5)],
    ["lie apart", rect(50, 50, 5, 5)],
  ])("returns null when they %s", (_, other) => {
    expect(intersectRect(rect(0, 0, 10, 10), other)).toBeNull();
  });
});

describe("visiblePart", () => {
  it("cuts the element at the edge of its scroll area", () => {
    expect(visiblePart(rect(10, 280, 100, 40), [list, page])).toEqual(rect(10, 280, 100, 20));
  });

  it("returns null when the scroll area hides the element", () => {
    expect(visiblePart(rect(10, 400, 100, 40), [list, page])).toBeNull();
  });

  it("returns the element itself without clips", () => {
    expect(visiblePart(rect(1, 2, 3, 4), [])).toEqual(rect(1, 2, 3, 4));
  });
});

describe("isInView", () => {
  it.each([
    [rect(10, 120, 100, 40), [list, page], true],
    [rect(10, 280, 100, 40), [list, page], true],
    [rect(10, 290, 100, 40), [list, page], false],
    [rect(10, 700, 100, 40), [page], false],
    [rect(0, 150, 300, 1000), [list, page], true],
    [rect(0, 250, 300, 1000), [list, page], false],
  ])("takes %j in clips %j as in view: %s", (element, clips, expected) => {
    expect(isInView(element, clips)).toBe(expected);
  });
});

describe("centerOf", () => {
  it("returns the middle of the rectangle", () => {
    expect(centerOf(rect(10, 280, 100, 20))).toEqual({ x: 60, y: 290 });
  });
});
