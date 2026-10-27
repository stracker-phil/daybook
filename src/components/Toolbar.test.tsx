import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { Toolbar } from "./Toolbar";

describe("Toolbar", () => {
  it("shows the title as the level-1 heading", () => {
    render(<Toolbar title="Projects" />);
    expect(screen.getByRole("heading", { level: 1, name: "Projects" })).toBeInTheDocument();
  });

  it("renders the actions", () => {
    render(
      <Toolbar title="Rules">
        <button type="button">New rule</button>
      </Toolbar>,
    );
    expect(screen.getByRole("button", { name: "New rule" })).toBeInTheDocument();
  });

  it("renders just the title when it has no actions", () => {
    render(<Toolbar title="Reports" />);
    expect(screen.getByRole("heading", { level: 1, name: "Reports" })).toBeInTheDocument();
    expect(screen.queryAllByRole("button")).toHaveLength(0);
  });
});
