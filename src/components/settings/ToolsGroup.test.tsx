import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import type { Settings, ToolPlacement } from "../../api";
import { settings } from "../../test/settings";
import { ToolsGroup } from "./ToolsGroup";
import { TOOL_INFO } from "./toolInfo";

const save = vi.fn<(patch: Partial<Settings>) => void>();

function Harness({ initial }: { initial: Settings }) {
  const [s, setS] = useState(initial);
  return (
    <ToolsGroup
      s={s}
      set={(patch) => {
        save(patch);
        setS((current) => ({ ...current, ...patch }));
      }}
    />
  );
}

function renderGroup(tools?: ToolPlacement[]) {
  render(<Harness initial={settings(tools ? { tools } : {})} />);
}

const rows = () => screen.getAllByRole("switch", { name: "Rail" }).map((el) => el.closest<HTMLElement>(".settings-row")!);
const rowAt = (i: number) => within(rows()[i]);
const labelAt = (i: number) => rows()[i].querySelector(".settings-row-label")!.textContent;
const saved = () => save.mock.lastCall![0].tools!;
const defaultTools = () => settings().tools;

describe("ToolsGroup", () => {
  beforeEach(() => vi.clearAllMocks());

  it("shows one row per tool, in the saved order, with its name and description", () => {
    renderGroup();
    const tools = defaultTools();
    expect(screen.getByRole("heading", { name: "Tools" })).toBeInTheDocument();
    expect(rows()).toHaveLength(tools.length);
    expect(rows().map((_, i) => labelAt(i))).toEqual(tools.map((t) => TOOL_INFO[t.id].label));
    expect(rowAt(1).getByText("Working, Break or Off; a click starts or ends a break.")).toBeInTheDocument();
  });

  it("follows a reordered list", () => {
    const [context, status, ...rest] = defaultTools();
    renderGroup([status, context, ...rest]);
    expect(labelAt(0)).toBe("Status");
  });

  it("checks the Rail and Window switches from each tool's placement", () => {
    renderGroup([
      { id: "context", rail: true, window: false },
      { id: "status", rail: false, window: true },
    ]);
    expect(rowAt(0).getByRole("switch", { name: "Rail" })).toBeChecked();
    expect(rowAt(0).getByRole("switch", { name: "Window" })).not.toBeChecked();
    expect(rowAt(1).getByRole("switch", { name: "Rail" })).not.toBeChecked();
    expect(rowAt(1).getByRole("switch", { name: "Window" })).toBeChecked();
  });

  it("saves a Rail switch turned off on that tool only", async () => {
    renderGroup();
    await userEvent.click(rowAt(1).getByRole("switch", { name: "Rail" }));
    expect(saved()[1]).toEqual({ id: "status", rail: false, window: true });
    expect(saved().filter((_, i) => i !== 1)).toEqual(defaultTools().filter((_, i) => i !== 1));
  });

  it("saves a Window switch turned off on that tool only", async () => {
    renderGroup();
    await userEvent.click(rowAt(0).getByRole("switch", { name: "Window" }));
    expect(saved()[0]).toEqual({ id: "context", rail: true, window: false });
    expect(saved().slice(1)).toEqual(defaultTools().slice(1));
  });

  it("keeps the Daybook tool out of its own window: Window is off and disabled, Rail still works", async () => {
    renderGroup();
    const hub = rows().findIndex((_, i) => labelAt(i) === "Daybook");
    expect(rowAt(hub).getByRole("switch", { name: "Window" })).toBeDisabled();
    expect(rowAt(hub).getByRole("switch", { name: "Window" })).not.toBeChecked();
    await userEvent.click(rowAt(hub).getByRole("switch", { name: "Rail" }));
    expect(saved()[hub]).toEqual({ id: "hub", rail: false, window: false });
  });

  it("disables Move Up on the first tool and Move Down on the last", () => {
    renderGroup();
    expect(rowAt(0).getByRole("button", { name: "Move Up" })).toBeDisabled();
    expect(rowAt(rows().length - 1).getByRole("button", { name: "Move Down" })).toBeDisabled();
  });

  it("saves the order after Move Down and shows it", async () => {
    renderGroup();
    await userEvent.click(rowAt(0).getByRole("button", { name: "Move Down" }));
    expect(saved().slice(0, 2).map((t) => t.id)).toEqual(["status", "context"]);
    expect(labelAt(0)).toBe("Status");
  });

  it("saves the order after Move Up", async () => {
    renderGroup();
    await userEvent.click(rowAt(2).getByRole("button", { name: "Move Up" }));
    expect(saved().slice(0, 3).map((t) => t.id)).toEqual(["context", "activity", "status"]);
  });
});
