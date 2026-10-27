import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Star } from "lucide-react";
import { describe, expect, it, vi } from "vitest";
import { CurrentMarker, PanelList, PanelOption, SectionHeader } from "./PanelList";

describe("PanelList", () => {
  it("is a listbox holding its options", () => {
    render(
      <PanelList aria-label="Contexts">
        <PanelOption selected={false}>Work</PanelOption>
      </PanelList>,
    );
    expect(screen.getByRole("listbox", { name: "Contexts" })).toContainElement(screen.getByRole("option", { name: "Work" }));
  });

  it("tints the selection only for the tint variant", () => {
    render(
      <>
        <PanelList aria-label="Grey" />
        <PanelList aria-label="Tinted" selection="tint" />
      </>,
    );
    expect(screen.getByRole("listbox", { name: "Tinted" })).toHaveClass("tint");
    expect(screen.getByRole("listbox", { name: "Grey" })).not.toHaveClass("tint");
  });
});

describe("PanelOption", () => {
  it("marks the selected option with aria-selected", () => {
    render(
      <PanelList>
        <PanelOption selected>Work</PanelOption>
        <PanelOption selected={false}>Personal</PanelOption>
      </PanelList>,
    );
    expect(screen.getByRole("option", { name: "Work" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("option", { name: "Personal" })).toHaveAttribute("aria-selected", "false");
  });

  it("shows its number", () => {
    render(
      <PanelOption selected={false} number="⌘1">
        Work
      </PanelOption>,
    );
    expect(screen.getByText("⌘1")).toBeInTheDocument();
  });

  it("renders its icon and children", () => {
    const { container } = render(
      <PanelOption selected={false} icon={Star}>
        Work
      </PanelOption>,
    );
    expect(container.querySelector("svg")).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "Work" })).toBeInTheDocument();
  });

  it("keeps the focus in a focused input when the mouse is pressed on it", async () => {
    render(
      <>
        <input aria-label="Filter" />
        <PanelOption selected={false}>Work</PanelOption>
      </>,
    );
    const input = screen.getByLabelText("Filter");
    await userEvent.click(input);
    await userEvent.pointer({ keys: "[MouseLeft>]", target: screen.getByRole("option", { name: "Work" }) });
    expect(input).toHaveFocus();
  });

  it("lets the mouse take the focus when keepFocus is off", () => {
    render(<PanelOption selected={false} keepFocus={false}>Work</PanelOption>);
    const notPrevented = fireEvent.mouseDown(screen.getByRole("option", { name: "Work" }));
    expect(notPrevented).toBe(true);
  });

  it("still calls the caller's onMouseDown and onClick", async () => {
    const onMouseDown = vi.fn();
    const onClick = vi.fn();
    render(
      <PanelOption selected={false} onMouseDown={onMouseDown} onClick={onClick}>
        Work
      </PanelOption>,
    );
    await userEvent.click(screen.getByRole("option", { name: "Work" }));
    expect(onMouseDown).toHaveBeenCalled();
    expect(onClick).toHaveBeenCalled();
  });
});

describe("SectionHeader", () => {
  it("shows its heading outside the options", () => {
    render(
      <PanelList>
        <SectionHeader>Recent</SectionHeader>
      </PanelList>,
    );
    expect(screen.getByText("Recent")).toBeInTheDocument();
    expect(screen.queryByRole("option")).not.toBeInTheDocument();
  });
});

describe("CurrentMarker", () => {
  it("reads Current", () => {
    render(<CurrentMarker />);
    expect(screen.getByText("Current")).toBeInTheDocument();
  });
});
