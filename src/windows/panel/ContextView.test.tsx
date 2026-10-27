import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type ContextState } from "../../api";
import { hintKeys } from "../../test/panelHints";
import { ContextView } from "./ContextView";

const mocked = vi.mocked(api, true);

const STATE: ContextState = {
  contexts: [
    { id: "work", title: "[W] Work", color: "blue" },
    { id: "private", title: "[P] Private", color: "green" },
    { id: "side", title: "Side", color: "pink" },
  ],
  current: "work",
};

const selected = () => screen.getAllByRole("option").find((o) => o.getAttribute("aria-selected") === "true")?.textContent;

async function renderView(state: ContextState = STATE) {
  mocked.contextState.mockResolvedValue(state);
  render(<ContextView />);
  await screen.findByRole("listbox", { name: "Contexts" });
  await screen.findByRole("option", { name: /Private/ });
}

describe("ContextView", () => {
  beforeEach(() => vi.clearAllMocks());

  it("lists the contexts with their numbers and marks the current one", async () => {
    await renderView();
    expect(screen.getAllByRole("option").map((o) => o.textContent)).toEqual(["1[W] WorkCurrent", "2[P] Private", "3Side"]);
  });

  it("lists the key hints in the footer", async () => {
    await renderView();
    expect(hintKeys("Context")).toEqual(["↑", "↓"]);
    expect(hintKeys("Switch")).toEqual(["↵"]);
    expect(hintKeys("Cancel")).toEqual(["esc"]);
  });

  it("starts on the current context", async () => {
    await renderView({ ...STATE, current: "private" });
    expect(selected()).toBe("2[P] PrivateCurrent");
  });

  it.each([
    ["a plain 2", "2"],
    ["⌘2", "{Meta>}2{/Meta}"],
  ])("switches to the second context with %s", async (_name, keys) => {
    await renderView();
    await userEvent.keyboard(keys);
    expect(mocked.runCommand).toHaveBeenCalledWith("mode:private");
  });

  it.each([
    ["a plain 9", "9"],
    ["⌘9", "{Meta>}9{/Meta}"],
  ])("ignores %s beyond the last context", async (_name, keys) => {
    await renderView();
    await userEvent.keyboard(keys);
    expect(mocked.runCommand).not.toHaveBeenCalled();
  });

  it("moves with the arrow keys, wraps, and switches the active context on Enter", async () => {
    await renderView();
    await userEvent.keyboard("{ArrowDown}{ArrowDown}");
    expect(selected()).toBe("3Side");
    await userEvent.keyboard("{ArrowDown}");
    expect(selected()).toBe("1[W] WorkCurrent");
    await userEvent.keyboard("{ArrowUp}{Enter}");
    expect(mocked.runCommand).toHaveBeenCalledWith("mode:side");
  });

  it("switches to the clicked context", async () => {
    await renderView();
    await userEvent.click(screen.getByRole("option", { name: /Side/ }));
    expect(mocked.runCommand).toHaveBeenCalledWith("mode:side");
  });

  it("runs the switch command also for the current context, so Rust closes the overlay", async () => {
    await renderView();
    await userEvent.keyboard("{Enter}");
    expect(mocked.runCommand).toHaveBeenCalledWith("mode:work");
  });

  it("shows the message of a rejected switch", async () => {
    mocked.runCommand.mockRejectedValueOnce({ kind: "io", message: "Could not switch", code: null });
    await renderView();
    await userEvent.keyboard("{Meta>}2{/Meta}");
    expect(await screen.findByText("Could not switch")).toBeInTheDocument();
    expect(hintKeys("Switch")).toEqual(["↵"]);
  });

  it("shows the message of a failed load", async () => {
    mocked.contextState.mockRejectedValue({ kind: "io", message: "Settings unreadable", code: null });
    render(<ContextView />);
    expect(await screen.findByText("Settings unreadable")).toBeInTheDocument();
  });
});
