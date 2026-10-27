import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { SegmentedControl } from "./SegmentedControl";

const OPTIONS = ["light", "dark", "system"] as const;

describe("SegmentedControl", () => {
  it("shows a capitalized button per option in a group named by the label", () => {
    render(<SegmentedControl label="Mode" options={OPTIONS} value="light" onChange={() => {}} />);
    const group = screen.getByRole("group", { name: "Mode" });
    expect(group).toContainElement(screen.getByRole("button", { name: "Light" }));
    expect(screen.getAllByRole("button").map((b) => b.textContent)).toEqual(["Light", "Dark", "System"]);
  });

  it("presses only the button of the current value", () => {
    render(<SegmentedControl label="Mode" options={OPTIONS} value="dark" onChange={() => {}} />);
    expect(screen.getByRole("button", { name: "Dark" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "Light" })).toHaveAttribute("aria-pressed", "false");
    expect(screen.getByRole("button", { name: "System" })).toHaveAttribute("aria-pressed", "false");
  });

  it("presses none when the value is null", () => {
    render(<SegmentedControl label="Mode" options={OPTIONS} value={null} onChange={() => {}} />);
    for (const button of screen.getAllByRole("button")) expect(button).toHaveAttribute("aria-pressed", "false");
  });

  it("disables every button and reports no click when disabled", async () => {
    const onChange = vi.fn();
    render(<SegmentedControl label="Mode" options={OPTIONS} value="light" onChange={onChange} disabled />);
    for (const button of screen.getAllByRole("button")) expect(button).toBeDisabled();
    await userEvent.click(screen.getByRole("button", { name: "Dark" }));
    expect(onChange).not.toHaveBeenCalled();
  });

  it("enables the buttons by default", () => {
    render(<SegmentedControl label="Mode" options={OPTIONS} value="light" onChange={() => {}} />);
    for (const button of screen.getAllByRole("button")) expect(button).toBeEnabled();
  });

  it("reports the option of the clicked button", async () => {
    const onChange = vi.fn();
    render(<SegmentedControl label="Mode" options={OPTIONS} value="light" onChange={onChange} />);
    await userEvent.click(screen.getByRole("button", { name: "System" }));
    expect(onChange).toHaveBeenLastCalledWith("system");
  });
});
