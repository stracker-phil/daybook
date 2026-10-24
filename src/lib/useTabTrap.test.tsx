import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useTabTrap } from "./useTabTrap";

function Page({ claimTab }: { claimTab?: boolean }) {
  useTabTrap();
  return (
    <>
      <button onKeyDown={claimTab ? (e) => e.key === "Tab" && e.preventDefault() : undefined}>First</button>
      <button>Second</button>
    </>
  );
}

describe("useTabTrap", () => {
  let rects: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    // jsdom has no layout, so nothing counts as visible without this.
    rects = vi.spyOn(HTMLElement.prototype, "getClientRects").mockReturnValue([new DOMRect()] as unknown as DOMRectList);
  });

  afterEach(() => rects.mockRestore());

  it("wraps focus around the page in both directions", async () => {
    render(<Page />);
    screen.getByRole("button", { name: "Second" }).focus();
    await userEvent.tab();
    expect(screen.getByRole("button", { name: "First" })).toHaveFocus();
    await userEvent.tab({ shift: true });
    expect(screen.getByRole("button", { name: "Second" })).toHaveFocus();
  });

  it("leaves Tab to a view that prevents its default", async () => {
    render(<Page claimTab />);
    const first = screen.getByRole("button", { name: "First" });
    first.focus();
    await userEvent.tab();
    expect(first).toHaveFocus();
  });
});
