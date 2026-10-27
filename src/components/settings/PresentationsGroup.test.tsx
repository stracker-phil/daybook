import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type Api, type AutomationInfo, type Settings } from "../../api";
import { resetStore, useStore } from "../../state/store";
import { settings } from "../../test/settings";
import { PresentationsGroup, ago } from "./PresentationsGroup";

const mocked = vi.mocked(api, true);
const save = vi.fn<(patch: Partial<Settings>) => void>();
const REGISTER = "stage register http://127.0.0.1:27180/v1/stage --token secret-token";
const FILE = "/Users/pat/Library/Application Support/glaze-stage/apps/com.philippstracker.daybook.json";

function Harness({ initial, info }: { initial: Settings; info: AutomationInfo }) {
  const [s, setS] = useState(initial);
  return (
    <PresentationsGroup
      s={s}
      info={info}
      set={(patch) => {
        save(patch);
        setS((current) => ({ ...current, ...patch }));
      }}
    />
  );
}

function info(patch: Partial<AutomationInfo> & { error?: string } = {}): AutomationInfo {
  const { error, ...rest } = patch;
  return {
    cli: { path: "/Users/pat/.local/bin/daybook", linked: true },
    api: { running: true, error: error ?? null, port: 27180 },
    token: "secret-token",
    baseUrl: "http://127.0.0.1:27180/v1",
    stageFile: null,
    lastPresenterRequest: null,
    registerCommand: REGISTER,
    examples: [],
    ...rest,
  };
}

function renderGroup(apiPatch: Partial<Api> = {}, patch: Parameters<typeof info>[0] = {}) {
  const initial = settings();
  initial.api = { ...initial.api, enabled: true, presentations: true, ...apiPatch };
  render(<Harness initial={initial} info={info(patch)} />);
}

const rowOf = (label: string) => screen.getByText(label).closest(".settings-row") as HTMLElement;
const toastMessages = () => useStore.getState().toasts.map((t) => t.message);

describe("PresentationsGroup", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore({ settings: settings() });
  });

  describe("status", () => {
    it("reads Discoverable when the discovery file exists", () => {
      renderGroup({}, { stageFile: FILE });
      expect(within(rowOf("Status")).getByText("Discoverable")).toBeInTheDocument();
    });

    it("reads Off when presentations are switched off", () => {
      renderGroup({ presentations: false });
      expect(within(rowOf("Status")).getByText("Off")).toBeInTheDocument();
    });

    it("shows the server error when presentations are on but the API failed", () => {
      renderGroup({}, { error: "Port 27180 is already used by another app." });
      expect(within(rowOf("Status")).getByText("Port 27180 is already used by another app.")).toBeInTheDocument();
    });

    it("says it needs the HTTP API when presentations are on and there is neither file nor error", () => {
      renderGroup({ enabled: false });
      expect(within(rowOf("Status")).getByText("Off (needs the HTTP API)")).toBeInTheDocument();
    });
  });

  describe("switch", () => {
    it.each([true, false])("shows Allow presentations as %s as saved", (presentations) => {
      renderGroup({ presentations });
      const toggle = screen.getByRole("switch", { name: "Allow presentations" });
      if (presentations) expect(toggle).toBeChecked();
      else expect(toggle).not.toBeChecked();
    });

    it("saves api.presentations and keeps the rest of the API settings", async () => {
      renderGroup({ presentations: true, port: 27180 });
      await userEvent.click(screen.getByRole("switch", { name: "Allow presentations" }));
      expect(save.mock.lastCall![0].api).toMatchObject({ presentations: false, port: 27180, enabled: true });
    });
  });

  describe("discovery file", () => {
    it("is not listed without a file", () => {
      renderGroup();
      expect(screen.queryByText("Discovery file")).not.toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "Show in Finder" })).not.toBeInTheDocument();
    });

    it("shows the path and reveals the file in Finder", async () => {
      renderGroup({}, { stageFile: FILE });
      expect(within(rowOf("Discovery file")).getByText(FILE)).toBeInTheDocument();
      await userEvent.click(screen.getByRole("button", { name: "Show in Finder" }));
      expect(mocked.stageRevealFile).toHaveBeenCalled();
    });

    it("reports a toast when Finder can't show the file", async () => {
      mocked.stageRevealFile.mockRejectedValue({ kind: "io", message: "Finder couldn’t show the file.", code: null });
      renderGroup({}, { stageFile: FILE });
      await userEvent.click(screen.getByRole("button", { name: "Show in Finder" }));
      await vi.waitFor(() => expect(toastMessages()).toContain("Couldn’t show the file: Finder couldn’t show the file."));
    });
  });

  describe("Register with Stage", () => {
    it("shows the command and copies it with a confirming toast", async () => {
      const user = userEvent.setup();
      renderGroup();
      expect(within(rowOf("Register with Stage")).getByText(REGISTER)).toBeInTheDocument();
      await user.click(within(rowOf("Register with Stage")).getByRole("button", { name: "Copy" }));
      expect(await navigator.clipboard.readText()).toBe(REGISTER);
      expect(toastMessages()).toContain("Copied the command");
    });

    it.each([
      ["presentations are off", { presentations: false }],
      ["the HTTP API is off", { enabled: false }],
    ])("is not offered while %s", (_, apiPatch) => {
      renderGroup(apiPatch);
      expect(screen.queryByText("Register with Stage")).not.toBeInTheDocument();
    });
  });

  describe("last presenter request", () => {
    const NOW = new Date("2026-10-27T12:00:00Z").getTime();

    beforeEach(() => vi.useFakeTimers({ toFake: ["Date"], now: NOW }));
    afterEach(() => vi.useRealTimers());

    it("says None since Daybook started without a request", () => {
      renderGroup();
      expect(within(rowOf("Last presenter request")).getByText("None since Daybook started")).toBeInTheDocument();
    });

    it("tells how long ago the last request was", () => {
      renderGroup({}, { lastPresenterRequest: NOW - 5 * 60_000 });
      expect(within(rowOf("Last presenter request")).getByText("5 minutes ago")).toBeInTheDocument();
    });
  });
});

describe("ago", () => {
  const NOW = new Date("2026-10-27T12:00:00Z").getTime();
  const MINUTE = 60_000;
  const HOUR = 60 * MINUTE;

  it.each([
    [0, "just now"],
    [MINUTE - 1, "just now"],
    [MINUTE, "a minute ago"],
    [2 * MINUTE - 1, "a minute ago"],
    [2 * MINUTE, "2 minutes ago"],
    [HOUR - MINUTE, "59 minutes ago"],
    [HOUR, "an hour ago"],
    [2 * HOUR - 1, "an hour ago"],
    [2 * HOUR, "2 hours ago"],
    [24 * HOUR - 1, "23 hours ago"],
  ])("words %i ms as %s", (elapsed, words) => {
    expect(ago(NOW - elapsed, NOW)).toBe(words);
  });

  it("gives a date and time after a day", () => {
    const time = NOW - 24 * HOUR;
    expect(ago(time, NOW)).toBe(new Date(time).toLocaleString([], { dateStyle: "medium", timeStyle: "short" }));
  });
});
