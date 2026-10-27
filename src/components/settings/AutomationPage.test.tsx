import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { act, useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type Api, type AutomationInfo, type Settings } from "../../api";
import { resetStore, useStore } from "../../state/store";
import { settings } from "../../test/settings";
import { AutomationPage } from "./AutomationPage";

const mocked = vi.mocked(api, true);
const save = vi.fn<(patch: Partial<Settings>) => void>();

function Harness({ initial }: { initial: Settings }) {
  const [s, setS] = useState(initial);
  return (
    <AutomationPage
      s={s}
      set={(patch) => {
        save(patch);
        setS((current) => ({ ...current, ...patch }));
      }}
    />
  );
}

function info(patch: { cli?: Partial<AutomationInfo["cli"]>; api?: Partial<AutomationInfo["api"]>; token?: string } = {}): AutomationInfo {
  const status = { running: false, error: null, port: 27180, ...patch.api };
  const baseUrl = `http://127.0.0.1:${status.port}/v1`;
  return {
    cli: { path: "/Users/pat/.local/bin/daybook", linked: true, ...patch.cli },
    api: status,
    token: patch.token ?? "secret-token",
    baseUrl,
    examples: [`curl -s -H "Authorization: Bearer <token>" ${baseUrl}/state`, `curl -s -H "Authorization: Bearer <token>" -d status=break -d note=Lunch ${baseUrl}/status`],
  };
}

async function renderPage(apiPatch: Partial<Api> = {}) {
  const initial = settings();
  initial.api = { ...initial.api, port: 27180, enabled: false, ...apiPatch };
  render(<Harness initial={initial} />);
  await screen.findByRole("heading", { name: "Command line" });
}

const savedApi = () => save.mock.lastCall![0].api!;
const toastMessages = () => useStore.getState().toasts.map((t) => t.message);
const copyButton = (row: string) => within(screen.getByText(row).closest(".settings-row") as HTMLElement).getByRole("button", { name: "Copy" });

describe("AutomationPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetStore({ settings: settings() });
    mocked.automationInfo.mockResolvedValue(info());
  });

  it("renders nothing until the automation info arrived", async () => {
    let resolve!: (value: AutomationInfo) => void;
    mocked.automationInfo.mockReturnValue(new Promise((r) => (resolve = r)));
    render(<Harness initial={settings()} />);
    expect(screen.queryByRole("heading", { name: "Command line" })).not.toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "HTTP API" })).not.toBeInTheDocument();
    await act(async () => resolve(info()));
    expect(await screen.findByRole("heading", { name: "HTTP API" })).toBeInTheDocument();
  });

  describe("Command line", () => {
    it("shows the path of the command and that it is linked", async () => {
      await renderPage();
      expect(screen.getByText("/Users/pat/.local/bin/daybook")).toBeInTheDocument();
      expect(screen.getByText("Linked")).toBeInTheDocument();
      expect(screen.queryByText(/Installing a release build/)).not.toBeInTheDocument();
    });

    it("says Not linked and how to link it when the command is not linked", async () => {
      mocked.automationInfo.mockResolvedValue(info({ cli: { linked: false } }));
      await renderPage();
      expect(screen.getByText("Not linked")).toBeInTheDocument();
      expect(screen.queryByText("Linked")).not.toBeInTheDocument();
      expect(screen.getByText("Installing a release build (bun run release) links it to the installed app.")).toBeInTheDocument();
    });

    it("copies the test command and confirms with a toast", async () => {
      const user = userEvent.setup();
      await renderPage();
      await user.click(copyButton("Test command"));
      expect(await navigator.clipboard.readText()).toBe("daybook help");
      expect(toastMessages()).toContain("Copied the command");
    });

    it("lists examples without the log and activity commands", async () => {
      await renderPage();
      const examples = screen.getByText("daybook status break lunch").closest("pre")!;
      expect(within(examples).getByText("daybook help")).toBeInTheDocument();
      expect(examples.textContent).not.toMatch(/daybook (log|activity)/);
    });
  });

  describe("HTTP API", () => {
    it("describes the base URL and the available actions", async () => {
      await renderPage();
      const description = screen.getByText(/state, status, slack, command, show/);
      expect(description).toHaveTextContent("http://127.0.0.1:27180/v1/<action>");
    });

    it.each([true, false])("shows the Enable switch as %s as saved", async (enabled) => {
      await renderPage({ enabled });
      const toggle = screen.getByRole("switch", { name: "Enable the HTTP API" });
      if (enabled) expect(toggle).toBeChecked();
      else expect(toggle).not.toBeChecked();
    });

    it("saves api.enabled when the switch is turned on and keeps the port", async () => {
      await renderPage({ enabled: false, port: 27180 });
      await userEvent.click(screen.getByRole("switch", { name: "Enable the HTTP API" }));
      expect(savedApi()).toMatchObject({ enabled: true, port: 27180 });
    });

    describe("status", () => {
      it("reads Listening while the server runs", async () => {
        mocked.automationInfo.mockResolvedValue(info({ api: { running: true } }));
        await renderPage();
        expect(screen.getByText("Listening")).toBeInTheDocument();
      });

      it("shows the error text when the server failed to start", async () => {
        mocked.automationInfo.mockResolvedValue(info({ api: { error: "Port 27180 is in use" } }));
        await renderPage();
        expect(screen.getByText("Port 27180 is in use")).toBeInTheDocument();
        expect(screen.queryByText("Listening")).not.toBeInTheDocument();
      });

      it("reads Off when the server neither runs nor failed", async () => {
        await renderPage();
        expect(screen.getByText("Off")).toBeInTheDocument();
      });
    });

    describe("port", () => {
      it("saves a valid port on Enter", async () => {
        await renderPage();
        const port = screen.getByRole("spinbutton", { name: "Port" });
        await userEvent.clear(port);
        await userEvent.type(port, "8080{Enter}");
        expect(savedApi()).toMatchObject({ port: 8080 });
      });

      it("saves a valid port on blur", async () => {
        await renderPage();
        const port = screen.getByRole("spinbutton", { name: "Port" });
        await userEvent.clear(port);
        await userEvent.type(port, "65535");
        await userEvent.tab();
        expect(savedApi()).toMatchObject({ port: 65535 });
      });

      it.each(["80", "1023", "65536"])("resets the field and saves nothing for the port %s", async (value) => {
        await renderPage();
        const port = screen.getByRole("spinbutton", { name: "Port" });
        await userEvent.clear(port);
        await userEvent.type(port, `${value}{Enter}`);
        expect(save).not.toHaveBeenCalled();
        expect(port).toHaveValue(27180);
      });
    });

    describe("token", () => {
      it("shows the token in a password field", async () => {
        await renderPage();
        const field = screen.getByLabelText("API token");
        expect(field).toHaveAttribute("type", "password");
        expect(field).toHaveValue("secret-token");
      });

      it("copies the real token and confirms with a toast", async () => {
        const user = userEvent.setup();
        await renderPage();
        await user.click(copyButton("Token"));
        expect(await navigator.clipboard.readText()).toBe("secret-token");
        expect(toastMessages()).toContain("Copied the token");
      });

      it("disables Copy and Copy Examples while there is no token", async () => {
        mocked.automationInfo.mockResolvedValue(info({ token: "" }));
        await renderPage();
        expect(copyButton("Token")).toBeDisabled();
        expect(screen.getByRole("button", { name: "Copy Examples" })).toBeDisabled();
      });

      it("shows the new token after New Token", async () => {
        await renderPage();
        mocked.automationInfo.mockResolvedValue(info({ token: "new-token" }));
        await userEvent.click(screen.getByRole("button", { name: "New Token" }));
        await vi.waitFor(() => expect(screen.getByLabelText("API token")).toHaveValue("new-token"));
      });

      it("reports a toast and keeps the token when New Token fails", async () => {
        mocked.automationNewToken.mockRejectedValue({ kind: "io", message: "Keychain locked", code: null });
        await renderPage();
        await userEvent.click(screen.getByRole("button", { name: "New Token" }));
        await vi.waitFor(() => expect(toastMessages()).toContain("Couldn't make a new token: Keychain locked"));
        expect(screen.getByLabelText("API token")).toHaveValue("secret-token");
        expect(screen.getByRole("button", { name: "New Token" })).toBeEnabled();
      });
    });

    describe("examples", () => {
      it("shows curl examples with a <token> placeholder instead of the token", async () => {
        await renderPage();
        const examples = screen.getByText(/\/v1\/state$/).closest("pre")!;
        expect(examples).toHaveTextContent('Authorization: Bearer <token>" http://127.0.0.1:27180/v1/state');
        expect(examples).toHaveTextContent("-d status=break -d note=Lunch http://127.0.0.1:27180/v1/status");
        expect(examples.textContent).not.toContain("secret-token");
      });

      it("copies both lines with the real token and the current port", async () => {
        const user = userEvent.setup();
        mocked.automationInfo.mockResolvedValue(info({ api: { port: 8080 } }));
        await renderPage({ port: 8080 });
        await user.click(screen.getByRole("button", { name: "Copy Examples" }));
        expect(await navigator.clipboard.readText()).toBe(
          [
            'curl -s -H "Authorization: Bearer secret-token" http://127.0.0.1:8080/v1/state',
            'curl -s -H "Authorization: Bearer secret-token" -d status=break -d note=Lunch http://127.0.0.1:8080/v1/status',
          ].join("\n"),
        );
        expect(toastMessages()).toContain("Copied the examples");
      });
    });
  });

  describe("refreshing the info", () => {
    it("reads the server state again when the API is enabled", async () => {
      await renderPage({ enabled: false });
      expect(screen.getByText("Off")).toBeInTheDocument();
      mocked.automationInfo.mockResolvedValue(info({ api: { running: true } }));
      await userEvent.click(screen.getByRole("switch", { name: "Enable the HTTP API" }));
      expect(await screen.findByText("Listening")).toBeInTheDocument();
    });

    it("reads the info again when the port changes", async () => {
      await renderPage();
      mocked.automationInfo.mockResolvedValue(info({ api: { port: 9000 } }));
      const port = screen.getByRole("spinbutton", { name: "Port" });
      await userEvent.clear(port);
      await userEvent.type(port, "9000{Enter}");
      await vi.waitFor(() => expect(screen.getByText(/state, status, slack, command, show/)).toHaveTextContent("http://127.0.0.1:9000/v1/<action>"));
    });

    it("polls every three seconds so a server that started in the background shows up", async () => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
      try {
        await renderPage();
        expect(screen.getByText("Off")).toBeInTheDocument();
        mocked.automationInfo.mockResolvedValue(info({ api: { running: true } }));
        await act(async () => {
          await vi.advanceTimersByTimeAsync(3000);
        });
        expect(screen.getByText("Listening")).toBeInTheDocument();
      } finally {
        vi.useRealTimers();
      }
    });
  });
});
