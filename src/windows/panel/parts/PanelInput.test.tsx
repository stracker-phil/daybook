import { render, screen } from "@testing-library/react";
import { Search } from "lucide-react";
import { describe, expect, it } from "vitest";
import { PanelInput, PanelPrompt } from "./PanelInput";

describe("PanelPrompt", () => {
  it("shows its question and is a drag region", () => {
    render(<PanelPrompt>What is your status?</PanelPrompt>);
    expect(screen.getByText("What is your status?")).toHaveAttribute("data-tauri-drag-region", "deep");
  });
});

describe("PanelInput", () => {
  it("takes the keyboard on open", () => {
    render(<PanelInput icon={Search} placeholder="Search" />);
    expect(screen.getByPlaceholderText("Search")).toHaveFocus();
  });

  it("leaves the keyboard alone when autoFocus is off", () => {
    render(<PanelInput icon={Search} placeholder="Search" autoFocus={false} />);
    expect(screen.getByPlaceholderText("Search")).not.toHaveFocus();
  });

  it("renders the leading part before the field", () => {
    render(<PanelInput icon={Search} placeholder="Search" leading={<span>Scope</span>} />);
    expect(screen.getByText("Scope")).toBeInTheDocument();
  });
});
