import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { hintKeys } from "../../../test/panelHints";
import { CLOSE_HINT, ESCAPE_HINT, PanelFooter, PanelHintItem } from "./PanelFooter";

describe("PanelFooter", () => {
  it("renders one key cap per key and the label of each hint", () => {
    const { container } = render(
      <PanelFooter
        hints={[
          { keys: ["⌘", "L"], label: "Links" },
          { keys: ["↑", "↓"], label: "Navigate" },
          ESCAPE_HINT,
        ]}
      />,
    );
    expect(container.querySelectorAll("kbd")).toHaveLength(5);
    expect(screen.getByText("⌘")).toBeInTheDocument();
    expect(screen.getByText("L")).toBeInTheDocument();
    expect(screen.getByText("Links")).toBeInTheDocument();
    expect(screen.getByText("Navigate")).toBeInTheDocument();
    expect(screen.getByText("esc")).toBeInTheDocument();
    expect(screen.getByText("Cancel")).toBeInTheDocument();
  });

  it("labels the close hint Close", () => {
    render(<PanelFooter hints={[CLOSE_HINT]} />);
    expect(screen.getByText("Close")).toBeInTheDocument();
  });

  it("shows the status when there is no error", () => {
    render(<PanelFooter status="3 results" />);
    expect(screen.getByText("3 results")).toBeInTheDocument();
  });

  it("shows the error instead of the status", () => {
    render(<PanelFooter status="3 results" error="Disk full" />);
    expect(screen.getByText("Disk full")).toBeInTheDocument();
    expect(screen.queryByText("3 results")).not.toBeInTheDocument();
  });

  it("falls back to the status when the error is null", () => {
    render(<PanelFooter status="Ready" error={null} />);
    expect(screen.getByText("Ready")).toBeInTheDocument();
  });

  it("renders its children", () => {
    render(
      <PanelFooter>
        <button type="button">Prefix</button>
      </PanelFooter>,
    );
    expect(screen.getByRole("button", { name: "Prefix" })).toBeInTheDocument();
  });

  it("is a drag region", () => {
    const { container } = render(<PanelFooter status="Ready" />);
    expect(container.firstElementChild).toHaveAttribute("data-tauri-drag-region", "deep");
  });
});

describe("PanelHintItem", () => {
  it("renders one key cap per key, then the label", () => {
    render(<PanelHintItem keys={["⌘", "L"]} label="Links" />);
    expect(hintKeys("Links")).toEqual(["⌘", "L"]);
  });
});
