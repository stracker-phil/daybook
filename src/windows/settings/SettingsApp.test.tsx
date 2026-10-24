import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { resetStore, useStore } from "../../state/store";
import { settings } from "../../test/settings";
import { SettingsApp } from "./SettingsApp";

const mocked = vi.mocked(api, true);

/** Renders the Settings window as Rust opens it, at `search` (e.g. `?page=vault`). */
async function renderSettings(search = "") {
  window.history.pushState({}, "", `/${search}`);
  resetStore();
  render(<SettingsApp />);
  await screen.findByRole("navigation", { name: "Settings pages" });
  await vi.waitFor(() => expect(mocked.onOpenSettings).toHaveBeenCalled());
  // This window's own load must be done: a stale load of an earlier test could have filled the store already.
  await mocked.getSettings.mock.results[0].value;
  await mocked.getKeybindings.mock.results[0].value;
  await act(async () => {});
}

function openSettingsEvent(page: string) {
  act(() => mocked.onOpenSettings.mock.calls[0][0](page));
}

describe("SettingsApp", () => {
  beforeEach(() => vi.clearAllMocks());
  afterEach(() => window.history.pushState({}, "", "/"));

  it("opens on the first page without a page in the address", async () => {
    await renderSettings();
    expect(screen.getByRole("heading", { name: "Vault", level: 2 })).toBeInTheDocument();
  });

  it("opens on the page named in the address", async () => {
    await renderSettings("?page=appearance");
    expect(screen.getByRole("heading", { name: "Appearance", level: 2 })).toBeInTheDocument();
  });

  it("falls back to the first page for an unknown page in the address", async () => {
    await renderSettings("?page=nonsense");
    expect(screen.getByRole("heading", { name: "Vault", level: 2 })).toBeInTheDocument();
  });

  it("shows the page of an open_settings event", async () => {
    await renderSettings("?page=vault");
    openSettingsEvent("contexts");
    expect(await screen.findByRole("heading", { name: "Contexts", level: 2 })).toBeInTheDocument();
  });

  it("follows the navigation after an event moved it", async () => {
    await renderSettings();
    openSettingsEvent("contexts");
    await userEvent.click(screen.getByRole("button", { name: "Tools" }));
    expect(await screen.findByRole("heading", { name: "Tools", level: 2 })).toBeInTheDocument();
  });

  it("applies the settings the backend announces as saved", async () => {
    await renderSettings();
    await vi.waitFor(() => expect(mocked.onSettingsChanged).toHaveBeenCalled());
    act(() => mocked.onSettingsChanged.mock.calls.forEach(([handler]) => handler(settings({ accent: "blue" }))));
    expect(useStore.getState().settings).toMatchObject({ accent: "blue" });
  });

  it("asks Rust to close the window on Escape", async () => {
    await renderSettings();
    await userEvent.keyboard("{Escape}");
    expect(mocked.runCommand).toHaveBeenCalledWith("window:close");
  });

  it("does not run the Daybook window's keys", async () => {
    await renderSettings();
    await userEvent.keyboard("{Meta>}{Control>}i{/Control}{/Meta}");
    expect(useStore.getState().uiState.inspectorHidden).toBe(false);
  });

  it("shows a rebinding announced by keybindings_changed on the Shortcuts page", async () => {
    await renderSettings("?page=shortcuts");
    await userEvent.type(screen.getByRole("textbox", { name: "Search shortcuts" }), "Toggle inspector");
    const recorder = () => screen.findByRole("button", { name: "Record shortcut for Toggle inspector" });
    expect(await recorder()).toHaveTextContent("⌘⌃I");
    await vi.waitFor(() => expect(mocked.onKeybindingsChanged).toHaveBeenCalled());
    act(() => mocked.onKeybindingsChanged.mock.calls[0][0]({ "inspector.toggle": "Mod+Shift+J" }));
    expect(await recorder()).toHaveTextContent("⌘⇧J");
  });
});
