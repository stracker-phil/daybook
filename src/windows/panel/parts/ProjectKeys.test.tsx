import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { hintKeys } from "../../../test/panelHints";
import { ProjectKeys } from "./ProjectKeys";

describe("ProjectKeys", () => {
  it("numbers the projects from ⌘1 in the given order", () => {
    render(<ProjectKeys names={["Internal", "Oscorp"]} />);
    expect(hintKeys("Internal")).toEqual(["⌘1"]);
    expect(hintKeys("Oscorp")).toEqual(["⌘2"]);
  });

  it("shows at most nine entries, ⌘1 to ⌘9", () => {
    const names = Array.from({ length: 12 }, (_, i) => `Project ${i + 1}`);
    render(<ProjectKeys names={names} />);
    expect(hintKeys("Project 9")).toEqual(["⌘9"]);
    expect(screen.queryByText("Project 10")).not.toBeInTheDocument();
    expect(screen.queryByText("⌘10")).not.toBeInTheDocument();
  });

  it("renders nothing without names", () => {
    render(<ProjectKeys names={[]} />);
    expect(screen.queryByLabelText("Project keys")).not.toBeInTheDocument();
  });
});
