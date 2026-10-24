import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { installCommands } from "../commands";
import { resetStore, useStore } from "../state/store";
import { ShortcutsDialog } from "./ShortcutsDialog";

describe("ShortcutsDialog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore();
    installCommands("main");
  });

  const row = (group: HTMLElement, title: string) => within(group).getByText(title).closest("div") as HTMLElement;

  it("shows the heading, the groups of the active scopes and a Close button", () => {
    render(<ShortcutsDialog scopes={["global", "hub"]} />);
    expect(screen.getByRole("dialog", { name: "Keyboard shortcuts" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Every window" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Daybook window" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Close" })).toBeInTheDocument();
  });

  it("closes the dialog with the Close button", async () => {
    const scopes = ["global", "hub"];
    resetStore({ dialog: { type: "shortcuts", scopes } });
    render(<ShortcutsDialog scopes={scopes} />);
    await userEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(useStore.getState().dialog).toBeNull();
  });

  it.each([
    ["Every window", "Larger text", "⌘="],
    ["Every window", "Smaller text", "⌘-"],
    ["Daybook window", "Keyboard shortcuts", "⇧?"],
    ["Daybook window", "Toggle sidebar", "⌘⌃S"],
    ["Daybook window", "Toggle inspector", "⌘⌃I"],
  ])("lists %s: %s with %s", (groupName, title, keys) => {
    render(<ShortcutsDialog scopes={["global", "hub"]} />);
    const group = screen.getByRole("region", { name: groupName });
    expect(within(row(group, title)).getByText(keys)).toBeInTheDocument();
  });

  it("omits commands without a key binding", () => {
    render(<ShortcutsDialog scopes={["global", "hub"]} />);
    const group = screen.getByRole("region", { name: "Every window" });
    expect(within(group).queryByText("Default text size")).not.toBeInTheDocument();
  });

  it("shows a rebound key instead of the default", () => {
    resetStore({ keybindings: { "sidebar.toggle": "Mod+Shift+J" } });
    installCommands("main");
    render(<ShortcutsDialog scopes={["global", "hub"]} />);
    const group = screen.getByRole("region", { name: "Daybook window" });
    expect(within(row(group, "Toggle sidebar")).getByText("⌘⇧J")).toBeInTheDocument();
  });

  it("shows no group for a scope that is not active", () => {
    render(<ShortcutsDialog scopes={[]} />);
    expect(screen.queryByRole("region")).not.toBeInTheDocument();
  });
});
