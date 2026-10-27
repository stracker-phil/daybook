import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { PanelSegmented } from "./PanelSegmented";

const segments = [
  { id: "day", label: "Day" },
  { id: "week", label: "Week" },
] as const;

describe("PanelSegmented", () => {
  it("marks only the segment of the value as pressed", () => {
    render(<PanelSegmented label="Range" segments={[...segments]} value="week" onPick={vi.fn()} />);
    expect(screen.getByRole("group", { name: "Range" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Week" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "Day" })).toHaveAttribute("aria-pressed", "false");
  });

  it("marks none as pressed for a null value", () => {
    render(<PanelSegmented label="Range" segments={[...segments]} value={null} onPick={vi.fn()} />);
    expect(screen.getByRole("button", { name: "Day" })).toHaveAttribute("aria-pressed", "false");
    expect(screen.getByRole("button", { name: "Week" })).toHaveAttribute("aria-pressed", "false");
  });

  it("calls onPick with the id of the clicked segment", async () => {
    const onPick = vi.fn();
    render(<PanelSegmented label="Range" segments={[...segments]} value="day" onPick={onPick} />);
    await userEvent.click(screen.getByRole("button", { name: "Week" }));
    expect(onPick).toHaveBeenCalledWith("week");
  });

  it("leaves the keyboard in the field when a segment is pressed", async () => {
    render(
      <>
        <input aria-label="Filter" />
        <PanelSegmented label="Range" segments={[...segments]} value="day" onPick={vi.fn()} />
      </>,
    );
    const input = screen.getByLabelText("Filter");
    await userEvent.click(input);
    await userEvent.click(screen.getByRole("button", { name: "Week" }));
    expect(input).toHaveFocus();
  });
});
