import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { EntryFooter, FormRow, FormText, LeadSwitch } from "./controls";
import { ItemActions } from "./fields";

describe("LeadSwitch", () => {
  it("names the switch by its label and reports the new state when toggled", async () => {
    const onChange = vi.fn();
    render(<LeadSwitch label="Working and present" checked={false} onChange={onChange} />);
    expect(screen.getByRole("switch", { name: "Working and present" })).not.toBeChecked();
    await userEvent.click(screen.getByRole("switch", { name: "Working and present" }));
    expect(onChange).toHaveBeenLastCalledWith(true);
  });

  it("reports false when an on switch is toggled", async () => {
    const onChange = vi.fn();
    render(<LeadSwitch label="Working" checked onChange={onChange} />);
    await userEvent.click(screen.getByRole("switch", { name: "Working" }));
    expect(onChange).toHaveBeenLastCalledWith(false);
  });

  it("shows a title node with a field instead of the label, the label still naming the switch", async () => {
    const onChange = vi.fn();
    render(<LeadSwitch label="Recognize Zoom calls" title={<input aria-label="Name" defaultValue="Zoom" />} checked onChange={onChange} />);
    expect(screen.getByRole("textbox", { name: "Name" })).toHaveValue("Zoom");
    expect(screen.queryByText("Recognize Zoom calls")).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("switch", { name: "Recognize Zoom calls" }));
    expect(onChange).toHaveBeenLastCalledWith(false);
  });

  it("keeps its children visible and usable while off", async () => {
    render(
      <LeadSwitch label="Working" checked={false} onChange={() => {}}>
        <input aria-label="Channel message" />
      </LeadSwitch>,
    );
    const field = screen.getByRole("textbox", { name: "Channel message" });
    expect(field).toBeVisible();
    await userEvent.type(field, "hi");
    expect(field).toHaveValue("hi");
  });

  it("shows the accessory next to the switch", () => {
    render(<LeadSwitch label="Working" checked onChange={() => {}} accessory={<button type="button">Remove Rule</button>} />);
    expect(screen.getByRole("button", { name: "Remove Rule" })).toBeInTheDocument();
  });
});

describe("FormRow", () => {
  it("shows the label, the control and the hint", () => {
    render(
      <FormRow label="Minutes before" hint="Before the meeting starts.">
        <input aria-label="Minutes" />
      </FormRow>,
    );
    expect(screen.getByText("Minutes before")).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Minutes" })).toBeInTheDocument();
    expect(screen.getByText("Before the meeting starts.")).toBeInTheDocument();
  });

  it("shows no hint when none is given", () => {
    render(<FormRow label="Minutes before" />);
    expect(screen.queryByText("Before the meeting starts.")).not.toBeInTheDocument();
  });

  it("labels the control with the label when given a htmlFor", () => {
    render(
      <FormRow label="Window title" htmlFor="title">
        <input id="title" />
      </FormRow>,
    );
    expect(screen.getByLabelText("Window title")).toBeInTheDocument();
  });
});

describe("FormText", () => {
  it("shows the value, the placeholder and the hint under its label", () => {
    render(<FormText label="Window title" value="Standup" placeholder="Any" hint="Only windows on the current Space count." onCommit={() => {}} />);
    expect(screen.getByLabelText("Window title")).toHaveValue("Standup");
    expect(screen.getByText("Only windows on the current Space count.")).toBeInTheDocument();
  });

  it("shows the placeholder while empty", () => {
    render(<FormText label="Channel message" value="" placeholder="No message" onCommit={() => {}} />);
    expect(screen.getByPlaceholderText("No message")).toBeInTheDocument();
  });

  it("commits the edited text on Enter", async () => {
    const onCommit = vi.fn();
    render(<FormText label="Channel message" value="" onCommit={onCommit} />);
    await userEvent.type(screen.getByLabelText("Channel message"), "Back soon{Enter}");
    expect(onCommit).toHaveBeenLastCalledWith("Back soon");
  });

  it("commits the edited text on blur but not while typing", async () => {
    const onCommit = vi.fn();
    render(<FormText label="Channel message" value="a" onCommit={onCommit} />);
    await userEvent.type(screen.getByLabelText("Channel message"), "b");
    expect(onCommit).not.toHaveBeenCalled();
    await userEvent.tab();
    expect(onCommit).toHaveBeenLastCalledWith("ab");
  });
});

describe("EntryFooter", () => {
  it("shows the add buttons before the item's own actions", () => {
    render(
      <EntryFooter>
        <button type="button">Add Meeting</button>
        <button type="button">Add Todo</button>
        <ItemActions index={0} count={2} onMove={() => {}} onRemove={() => {}} removeLabel="Remove Rule" />
      </EntryFooter>,
    );
    expect(screen.getAllByRole("button").map((b) => b.getAttribute("aria-label") ?? b.textContent)).toEqual(["Add Meeting", "Add Todo", "Move Up", "Move Down", "Remove Rule"]);
  });

  it("runs the add button's action", async () => {
    const onAdd = vi.fn();
    render(
      <EntryFooter>
        <button type="button" onClick={onAdd}>
          Add Rule
        </button>
      </EntryFooter>,
    );
    await userEvent.click(screen.getByRole("button", { name: "Add Rule" }));
    expect(onAdd).toHaveBeenCalled();
  });
});
