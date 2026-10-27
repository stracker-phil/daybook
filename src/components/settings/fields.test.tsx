import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { Context } from "../../api";
import { ColorField, CommitInput, DaysField, ItemActions, TimeRangeField, ToggleChips, contextChoices, joinList, moveItem, parseList, replaceItem } from "./fields";

describe("ColorField", () => {
  it("offers the nine colours as radios and checks the value", () => {
    render(<ColorField label="Work color" value="teal" onChange={() => {}} />);
    const radios = within(screen.getByRole("radiogroup", { name: "Work color" })).getAllByRole("radio");
    expect(radios).toHaveLength(9);
    expect(screen.getByRole("radio", { name: "teal" })).toBeChecked();
    expect(screen.getByRole("radio", { name: "blue" })).not.toBeChecked();
  });

  it("reports the clicked colour", async () => {
    const onChange = vi.fn();
    render(<ColorField label="Work color" value="blue" onChange={onChange} />);
    await userEvent.click(screen.getByRole("radio", { name: "pink" }));
    expect(onChange).toHaveBeenLastCalledWith("pink");
  });
});

describe("DaysField", () => {
  const day = (name: string) => screen.getByRole("button", { name });

  it("lists the weekdays Monday first", () => {
    render(<DaysField label="Work days" days={[1]} onChange={() => {}} />);
    expect(screen.getAllByRole("button").map((b) => b.textContent)).toEqual(["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"]);
  });

  it("presses the listed days", () => {
    render(<DaysField label="Work days" days={[1, 5]} onChange={() => {}} />);
    expect(day("Mo")).toHaveAttribute("aria-pressed", "true");
    expect(day("Fr")).toHaveAttribute("aria-pressed", "true");
    expect(day("Tu")).toHaveAttribute("aria-pressed", "false");
  });

  it("shows every day on for an empty list", () => {
    render(<DaysField label="Work days" days={[]} onChange={() => {}} />);
    expect(screen.getAllByRole("button").every((b) => b.getAttribute("aria-pressed") === "true")).toBe(true);
  });

  it("turns Monday off from every day and keeps the rest sorted", async () => {
    const onChange = vi.fn();
    render(<DaysField label="Work days" days={[]} onChange={onChange} />);
    await userEvent.click(day("Mo"));
    expect(onChange).toHaveBeenLastCalledWith([0, 2, 3, 4, 5, 6]);
  });

  it("adds a day to the list in sorted order", async () => {
    const onChange = vi.fn();
    render(<DaysField label="Work days" days={[1, 5]} onChange={onChange} />);
    await userEvent.click(day("Tu"));
    expect(onChange).toHaveBeenLastCalledWith([1, 2, 5]);
  });

  it("saves an empty list when the last day is turned on to complete the week", async () => {
    const onChange = vi.fn();
    render(<DaysField label="Work days" days={[0, 1, 2, 3, 4, 5]} onChange={onChange} />);
    await userEvent.click(day("Sa"));
    expect(onChange).toHaveBeenLastCalledWith([]);
  });

  it("saves an empty list when the last day is turned off", async () => {
    const onChange = vi.fn();
    render(<DaysField label="Work days" days={[3]} onChange={onChange} />);
    await userEvent.click(day("We"));
    expect(onChange).toHaveBeenLastCalledWith([]);
  });
});

describe("TimeRangeField", () => {
  const from = () => screen.getByLabelText("Hours from");
  const until = () => screen.getByLabelText("Hours until");

  it("shows both times", () => {
    render(<TimeRangeField label="Hours" from="09:00" to="18:00" onChange={() => {}} />);
    expect(from()).toHaveValue("09:00");
    expect(until()).toHaveValue("18:00");
  });

  it("commits a changed start on blur and keeps the end", async () => {
    const onChange = vi.fn();
    render(<TimeRangeField label="Hours" from="09:00" to="18:00" onChange={onChange} />);
    await userEvent.click(from());
    fireEvent.change(from(), { target: { value: "08:30" } });
    expect(onChange).not.toHaveBeenCalled();
    await userEvent.tab();
    expect(onChange).toHaveBeenLastCalledWith({ from: "08:30", to: "18:00" });
  });

  it("commits a changed end on Enter and keeps the start", async () => {
    const onChange = vi.fn();
    render(<TimeRangeField label="Hours" from="09:00" to="18:00" onChange={onChange} />);
    fireEvent.change(until(), { target: { value: "17:15" } });
    until().focus();
    await userEvent.keyboard("{Enter}");
    expect(onChange).toHaveBeenLastCalledWith({ from: "09:00", to: "17:15" });
  });

  it("goes back to the value when the draft is emptied", async () => {
    const onChange = vi.fn();
    render(<TimeRangeField label="Hours" from="09:00" to="18:00" onChange={onChange} />);
    from().focus();
    fireEvent.change(from(), { target: { value: "" } });
    await userEvent.tab();
    expect(from()).toHaveValue("09:00");
    expect(onChange).not.toHaveBeenCalled();
  });

  it("goes back to the end when it is emptied without openEnd", async () => {
    const onChange = vi.fn();
    render(<TimeRangeField label="Hours" from="09:00" to="18:00" onChange={onChange} />);
    until().focus();
    fireEvent.change(until(), { target: { value: "" } });
    await userEvent.tab();
    expect(until()).toHaveValue("18:00");
    expect(onChange).not.toHaveBeenCalled();
  });

  it("commits an empty end with openEnd and keeps the start", async () => {
    const onChange = vi.fn();
    render(<TimeRangeField label="Hours" from="09:00" to="18:00" openEnd onChange={onChange} />);
    until().focus();
    fireEvent.change(until(), { target: { value: "" } });
    await userEvent.tab();
    expect(onChange).toHaveBeenLastCalledWith({ from: "09:00", to: "" });
  });

  it("still reverts an emptied start with openEnd", async () => {
    const onChange = vi.fn();
    render(<TimeRangeField label="Hours" from="09:00" to="" openEnd onChange={onChange} />);
    from().focus();
    fireEvent.change(from(), { target: { value: "" } });
    await userEvent.tab();
    expect(from()).toHaveValue("09:00");
    expect(onChange).not.toHaveBeenCalled();
  });
});

describe("ToggleChips", () => {
  const CHOICES = [
    { id: "work", title: "Work" },
    { id: "private", title: "Private" },
    { id: "side", title: "Side hustle" },
  ];
  const renderChips = (chosen: string[] = [], onChange = vi.fn()) => {
    render(<ToggleChips label="Contexts" choices={CHOICES} chosen={chosen} onChange={onChange} />);
    return onChange;
  };
  const chip = (name: string) => screen.getByRole("button", { name });

  it("offers every choice as a button in the labelled group", () => {
    renderChips();
    expect(within(screen.getByRole("group", { name: "Contexts" })).getAllByRole("button").map((b) => b.textContent)).toEqual(["Work", "Private", "Side hustle"]);
  });

  it("presses the chosen ones", () => {
    renderChips(["work", "side"]);
    expect(chip("Work")).toHaveAttribute("aria-pressed", "true");
    expect(chip("Side hustle")).toHaveAttribute("aria-pressed", "true");
    expect(chip("Private")).toHaveAttribute("aria-pressed", "false");
  });

  it("adds a chip to the chosen ones", async () => {
    const onChange = renderChips(["work"]);
    await userEvent.click(chip("Private"));
    expect(onChange).toHaveBeenLastCalledWith(["work", "private"]);
  });

  it("removes a chosen chip", async () => {
    const onChange = renderChips(["work", "side"]);
    await userEvent.click(chip("Work"));
    expect(onChange).toHaveBeenLastCalledWith(["side"]);
  });
});

describe("contextChoices", () => {
  it("titles each context by its emoji label and name, leaving out an empty label", () => {
    const contexts = [
      { id: "work", label: " 💼 ", name: "Work" },
      { id: "private", label: "", name: "Private" },
    ] as Context[];
    expect(contextChoices(contexts)).toEqual([
      { id: "work", title: "💼 Work" },
      { id: "private", title: "Private" },
    ]);
  });
});

describe("CommitInput", () => {
  it("commits the trimmed text on Enter", async () => {
    const onCommit = vi.fn();
    render(<CommitInput label="Name" value="Alpha" onCommit={onCommit} />);
    await userEvent.type(screen.getByRole("textbox", { name: "Name" }), "x  {Enter}");
    expect(onCommit).toHaveBeenLastCalledWith("Alphax");
  });

  it("commits on blur", async () => {
    const onCommit = vi.fn();
    render(<CommitInput label="Name" value="Alpha" onCommit={onCommit} />);
    await userEvent.type(screen.getByRole("textbox", { name: "Name" }), "!");
    expect(onCommit).not.toHaveBeenCalled();
    await userEvent.tab();
    expect(onCommit).toHaveBeenLastCalledWith("Alpha!");
  });

  it("does not commit an unchanged value", async () => {
    const onCommit = vi.fn();
    render(<CommitInput label="Name" value="Alpha" onCommit={onCommit} />);
    await userEvent.click(screen.getByRole("textbox", { name: "Name" }));
    await userEvent.tab();
    expect(onCommit).not.toHaveBeenCalled();
  });

  it("shows the saved value again when a required field is emptied", async () => {
    const onCommit = vi.fn();
    render(<CommitInput label="Name" value="Alpha" required onCommit={onCommit} />);
    await userEvent.clear(screen.getByRole("textbox", { name: "Name" }));
    await userEvent.tab();
    expect(screen.getByRole("textbox", { name: "Name" })).toHaveValue("Alpha");
    expect(onCommit).not.toHaveBeenCalled();
  });
});

describe("ItemActions", () => {
  const renderActions = (index: number, count: number) => {
    const onMove = vi.fn();
    const onRemove = vi.fn();
    render(<ItemActions index={index} count={count} onMove={onMove} onRemove={onRemove} removeLabel="Remove Thing" />);
    return { onMove, onRemove };
  };

  it("disables Move Up on the first item", () => {
    renderActions(0, 3);
    expect(screen.getByRole("button", { name: "Move Up" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Move Down" })).toBeEnabled();
  });

  it("disables Move Down on the last item", () => {
    renderActions(2, 3);
    expect(screen.getByRole("button", { name: "Move Down" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Move Up" })).toBeEnabled();
  });

  it("moves the item one step up or down", async () => {
    const { onMove } = renderActions(1, 3);
    await userEvent.click(screen.getByRole("button", { name: "Move Up" }));
    expect(onMove).toHaveBeenLastCalledWith(1, 0);
    await userEvent.click(screen.getByRole("button", { name: "Move Down" }));
    expect(onMove).toHaveBeenLastCalledWith(1, 2);
  });

  it("names the buttons Move Up, Move Down and the remove label", () => {
    renderActions(1, 3);
    expect(screen.getAllByRole("button").map((b) => b.getAttribute("aria-label"))).toEqual(["Move Up", "Move Down", "Remove Thing"]);
  });

  it("offers no remove button without an onRemove", () => {
    render(<ItemActions index={0} count={2} onMove={() => {}} />);
    expect(screen.getAllByRole("button").map((b) => b.getAttribute("aria-label"))).toEqual(["Move Up", "Move Down"]);
  });

  it("removes through the button with the given label", async () => {
    const { onRemove } = renderActions(0, 1);
    await userEvent.click(screen.getByRole("button", { name: "Remove Thing" }));
    expect(onRemove).toHaveBeenCalled();
  });
});

describe("list helpers", () => {
  it("moveItem moves an item without changing the original", () => {
    const list = ["a", "b", "c"];
    expect(moveItem(list, 0, 2)).toEqual(["b", "c", "a"]);
    expect(moveItem(list, 2, 1)).toEqual(["a", "c", "b"]);
    expect(list).toEqual(["a", "b", "c"]);
  });

  it("replaceItem patches only the item at the index", () => {
    const list = [{ n: 1, x: "a" }, { n: 2, x: "b" }];
    expect(replaceItem(list, 1, { x: "z" })).toEqual([{ n: 1, x: "a" }, { n: 2, x: "z" }]);
    expect(list[1].x).toBe("b");
  });

  it("parseList splits on commas, trims and drops empty words", () => {
    expect(parseList(" QRS, Initech ,, ")).toEqual(["QRS", "Initech"]);
    expect(parseList("")).toEqual([]);
  });

  it("joinList separates the words with a comma and a space", () => {
    expect(joinList(["QRS", "Initech"])).toBe("QRS, Initech");
  });
});
