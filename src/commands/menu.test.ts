import { readFileSync } from "node:fs";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api } from "../api";
import { resetStore, useStore } from "../state/store";
import { settings } from "../test/settings";
import { allCommands, getCommand, installCommands } from "./index";
import { installMenuSync, menuState, syncMenu } from "./menu";

const mocked = vi.mocked(api, true);
const enabled = () => menuState().enabled;
const checked = () => menuState().checked;

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

describe("the native menu list", () => {
  const source = readFileSync("src-tauri/src/menu/items.rs", "utf8");
  const list = source.slice(source.indexOf("pub const MENU_ITEMS"), source.indexOf("];", source.indexOf("pub const MENU_ITEMS")));
  const rustItems = [...list.matchAll(/\b(item|check)\(\s*"([^"]+)",\s*"[^"]+",\s*"([^"]*)",?\s*\)/g)].map(([, kind, id, binding]) => ({ kind, id, binding }));

  beforeEach(() => installCommands());

  it("lists the same command ids as the commands marked as menu items", () => {
    const marked = allCommands()
      .filter((c) => c.menu)
      .map((c) => c.id);
    expect(rustItems.map((i) => i.id).sort()).toEqual(marked.sort());
  });

  it("makes exactly the commands with a checkmark checkbox items", () => {
    const checkboxes = allCommands()
      .filter((c) => c.menu && c.checked)
      .map((c) => c.id);
    expect(rustItems.filter((i) => i.kind === "check").map((i) => i.id).sort()).toEqual(checkboxes.sort());
  });

  it("shows the default binding of each command", () => {
    const mismatches = rustItems.filter((i) => i.binding !== (getCommand(i.id)?.defaultBinding ?? "")).map((i) => [i.id, i.binding]);
    expect(mismatches).toEqual([]);
  });
});

describe("menuState", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore({ settings: settings() });
    installCommands();
  });

  it("enables the global menu commands while nothing is open", () => {
    expect(enabled()).toEqual(expect.arrayContaining(["settings.toggle", "palette.toggle", "window.hide", "app.quit", "app.launchAtLogin"]));
  });

  it("disables launch at login until the settings are loaded", () => {
    resetStore({ settings: null });
    expect(enabled()).not.toContain("app.launchAtLogin");
  });

  it("checks launch at login while it is on", () => {
    expect(checked()).not.toContain("app.launchAtLogin");
    resetStore({ settings: settings({ launchAtLogin: true }) });
    expect(checked()).toContain("app.launchAtLogin");
  });

  it("keeps only the overlay-safe commands enabled while a dialog is open", () => {
    resetStore({ settings: settings(), dialog: { type: "shortcuts", scopes: ["global"] } });
    expect(enabled()).toEqual(expect.arrayContaining(["settings.toggle", "app.quit", "app.background"]));
    expect(enabled()).not.toContain("sidebar.toggle");
    expect(enabled()).not.toContain("logs.open");
  });
});

describe("menu sync", () => {
  let stop = () => {};

  beforeEach(() => {
    vi.clearAllMocks();
    resetStore({ settings: settings() });
    installCommands();
  });

  afterEach(() => {
    stop();
    stop = () => {};
  });

  it("sends the state when the sync is installed", () => {
    stop = installMenuSync();
    expect(mocked.menuSync).toHaveBeenCalledWith(menuState());
  });

  it("sends nothing while the state is unchanged and sends again once it changed", () => {
    stop = installMenuSync();
    mocked.menuSync.mockClear();
    syncMenu();
    expect(mocked.menuSync).not.toHaveBeenCalled();
    useStore.setState({ settings: settings({ launchAtLogin: true }) });
    expect(mocked.menuSync).toHaveBeenCalledWith(expect.objectContaining({ checked: ["app.launchAtLogin"] }));
  });

  it("sends again after a failed send", async () => {
    mocked.menuSync.mockRejectedValueOnce({ kind: "io", message: "gone" });
    stop = installMenuSync();
    await flush();
    mocked.menuSync.mockClear();
    syncMenu();
    expect(mocked.menuSync).toHaveBeenCalledWith(menuState());
  });

  it("stops following once uninstalled", () => {
    installMenuSync()();
    mocked.menuSync.mockClear();
    useStore.getState().openDialog({ type: "settings" });
    expect(mocked.menuSync).not.toHaveBeenCalled();
  });
});
