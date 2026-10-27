import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { collectAnchors } from "./anchors";

const RECT = { x: 10, y: 20, width: 100, height: 30 };
const ZERO = { x: 0, y: 0, width: 0, height: 0 };

/** jsdom has no layout: every element is 100x30 unless it carries `data-flat`. */
function mount(html: string) {
  document.body.innerHTML = html;
  return collectAnchors();
}

function only(html: string) {
  const anchors = mount(html);
  expect(anchors).toHaveLength(1);
  return anchors[0];
}

describe("collectAnchors", () => {
  beforeEach(() => {
    vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (this: Element) {
      return (this.hasAttribute("data-flat") ? ZERO : RECT) as DOMRect;
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
    document.body.innerHTML = "";
  });

  describe("which elements count", () => {
    it("lists the marked elements in document order with their rectangle", () => {
      const anchors = mount(`<button data-stage="b">B</button><button data-stage="a">A</button>`);
      expect(anchors.map((a) => a.id)).toEqual(["b", "a"]);
      expect(anchors[0].rect).toEqual(RECT);
      expect(anchors[0].inView).toBe(true);
      expect(anchors[0].visible).toEqual(RECT);
    });

    it("trims the id and skips empty ids", () => {
      const anchors = mount(`<div data-stage="  x  ">X</div><div data-stage="  ">Y</div>`);
      expect(anchors.map((a) => a.id)).toEqual(["x"]);
    });

    it("keeps only the first element of a duplicated id", () => {
      const anchors = mount(`<div data-stage="x">first</div><div data-stage="x">second</div>`);
      expect(anchors).toHaveLength(1);
      expect(anchors[0].label).toBe("first");
    });

    it("skips elements without a size", () => {
      const anchors = mount(`<div data-stage="flat" data-flat>Flat</div><div data-stage="real">Real</div>`);
      expect(anchors.map((a) => a.id)).toEqual(["real"]);
    });

    it("lists a sized duplicate behind a zero-size one", () => {
      const anchors = mount(`<div data-stage="x" data-flat>zero</div><div data-stage="x">sized</div>`);
      expect(anchors.map((a) => a.label)).toEqual(["sized"]);
    });
  });

  describe("label", () => {
    it.each([
      ["its own data-stage-label", `<div data-stage="a" data-stage-label="Own" aria-label="Aria" title="Title"><i data-stage-label="Nested"></i>Text</div>`, "Own"],
      ["a nested data-stage-label before aria-label", `<div data-stage="a" aria-label="Aria" title="Title"><i data-stage-label="Nested"></i>Text</div>`, "Nested"],
      ["aria-label before title", `<div data-stage="a" aria-label="Aria" title="Title">Text</div>`, "Aria"],
      ["the title before the text", `<div data-stage="a" title="Title">Text</div>`, "Title"],
      ["the text content", `<div data-stage="a">Text</div>`, "Text"],
      ["the text with whitespace collapsed", `<div data-stage="a">  Hello \n   <b>big</b>   world </div>`, "Hello big world"],
    ])("prefers %s", (_, html, label) => {
      expect(only(html).label).toBe(label);
    });

    it("cuts a label longer than 80 characters to 79 and an ellipsis", () => {
      const label = only(`<div data-stage="a">${"a".repeat(100)}</div>`).label;
      expect(label).toBe(`${"a".repeat(79)}…`);
    });

    it("keeps a label of exactly 80 characters", () => {
      expect(only(`<div data-stage="a">${"a".repeat(80)}</div>`).label).toBe("a".repeat(80));
    });
  });

  describe("role", () => {
    it.each([
      ["a button", `<button data-stage="a">Go</button>`, "button"],
      ["a link", `<a href="/x" data-stage="a">Go</a>`, "link"],
      ["a text input", `<input data-stage="a" />`, "field"],
      ["a textarea", `<textarea data-stage="a"></textarea>`, "field"],
      ["a checkbox", `<input type="checkbox" data-stage="a" />`, "toggle"],
      ["a switch", `<div role="switch" data-stage="a"></div>`, "toggle"],
      ["a tab", `<div role="tab" data-stage="a">T</div>`, "tab"],
      ["an option", `<div role="option" data-stage="a">O</div>`, "item"],
      ["a menu item", `<div role="menuitem" data-stage="a">M</div>`, "item"],
      ["a combobox", `<div role="combobox" data-stage="a">C</div>`, "field"],
    ])("is inferred from %s", (_, html, role) => {
      expect(only(html).role).toBe(role);
    });

    it("is inferred from the only control inside a wrapper", () => {
      expect(only(`<div data-stage="a"><span>Label</span><button>Go</button></div>`).role).toBe("button");
    });

    it("is left out when a wrapper holds several controls", () => {
      expect(only(`<div data-stage="a"><button>One</button><button>Two</button></div>`).role).toBeUndefined();
    });

    it("is left out for a plain element", () => {
      expect(only(`<div data-stage="a">Plain</div>`).role).toBeUndefined();
    });

    it("lets an explicit data-stage-role win over the inferred one", () => {
      expect(only(`<button data-stage="a" data-stage-role="item">Go</button>`).role).toBe("item");
    });

    it("ignores an explicit data-stage-role that is not a role", () => {
      expect(only(`<button data-stage="a" data-stage-role="bogus">Go</button>`).role).toBe("button");
    });
  });

  describe("list", () => {
    it("comes from the closest ancestor marked data-stage-list", () => {
      const anchors = mount(`<nav data-stage-list="tools"><div data-stage="tool:a">A</div></nav>`);
      expect(anchors[0].list).toBe("tools");
    });

    it("is left out outside a list", () => {
      expect(only(`<div data-stage="a">A</div>`).list).toBeUndefined();
    });
  });

  describe("states", () => {
    it.each([
      ["aria-selected true", `<div data-stage="a" aria-selected="true">A</div>`, true],
      ["aria-selected false", `<div data-stage="a" aria-selected="false">A</div>`, false],
      ["aria-current", `<div data-stage="a" aria-current="page">A</div>`, true],
      ["data-selected", `<div data-stage="a" data-selected>A</div>`, true],
      ["data-state active", `<div data-stage="a" data-state="active">A</div>`, true],
      ["data-state inactive", `<div data-stage="a" data-state="inactive">A</div>`, false],
    ])("reads selected from %s", (_, html, selected) => {
      expect(only(html).selected).toBe(selected);
    });

    it.each([
      ["aria-checked", `<div data-stage="a" role="switch" aria-checked="true"></div>`, true],
      ["aria-pressed", `<button data-stage="a" aria-pressed="false">A</button>`, false],
      ["a checked checkbox", `<input type="checkbox" data-stage="a" checked />`, true],
      ["an unchecked checkbox", `<input type="checkbox" data-stage="a" />`, false],
      ["data-state checked", `<button data-stage="a" data-state="checked">A</button>`, true],
      ["data-state off", `<button data-stage="a" data-state="off">A</button>`, false],
    ])("reads checked from %s", (_, html, checked) => {
      expect(only(html).checked).toBe(checked);
    });

    it("reads expanded from aria-expanded", () => {
      expect(only(`<button data-stage="a" aria-expanded="true">A</button>`).expanded).toBe(true);
    });

    it("reads the states of the only control inside a wrapper", () => {
      const anchor = only(`<div data-stage="a"><input type="checkbox" checked /></div>`);
      expect(anchor.checked).toBe(true);
      expect(anchor.role).toBe("toggle");
    });

    it.each([
      ["a disabled button", `<button data-stage="a" disabled>A</button>`],
      ["aria-disabled", `<div role="button" data-stage="a" aria-disabled="true">A</div>`],
    ])("reports disabled for %s", (_, html) => {
      expect(only(html).disabled).toBe(true);
    });

    it("leaves every state out for a plain enabled element", () => {
      const anchor = only(`<button data-stage="a">A</button>`);
      expect(anchor.disabled).toBeUndefined();
      expect(anchor.selected).toBeUndefined();
      expect(anchor.checked).toBeUndefined();
      expect(anchor.expanded).toBeUndefined();
    });
  });
});
