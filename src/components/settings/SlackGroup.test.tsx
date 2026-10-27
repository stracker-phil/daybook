import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type SlackInfo } from "../../api";
import { resetStore, useStore } from "../../state/store";
import { settings } from "../../test/settings";
import { SlackGroup } from "./SlackGroup";

const mocked = vi.mocked(api, true);

const CONNECTED: SlackInfo = { connected: true, identity: { team: "Acme", user: "pat" } };

async function renderGroup(info: SlackInfo) {
  mocked.slackInfo.mockResolvedValue(info);
  resetStore({ settings: settings() });
  render(<SlackGroup />);
  await screen.findByText(info.connected ? "Connected" : "User token");
}

const toastMessages = () => useStore.getState().toasts.map((t) => t.message).join();

describe("SlackGroup", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.slackConnect.mockResolvedValue({ team: "Acme", user: "pat" });
    mocked.slackTest.mockResolvedValue({ team: "Acme", user: "pat" });
    mocked.slackDisconnect.mockResolvedValue(undefined);
    mocked.onSlackChanged.mockResolvedValue(() => {});
  });

  describe("without a token", () => {
    it("offers a password field for the token and a disabled Connect", async () => {
      await renderGroup({ connected: false, identity: null });
      expect(screen.getByRole("heading", { name: "Slack" })).toBeInTheDocument();
      expect(screen.getByLabelText("User token")).toHaveAttribute("type", "password");
      expect(screen.getByRole("button", { name: "Connect" })).toBeDisabled();
      expect(screen.queryByText("Connected")).not.toBeInTheDocument();
    });

    it("keeps Connect disabled for a whitespace token", async () => {
      await renderGroup({ connected: false, identity: null });
      await userEvent.type(screen.getByLabelText("User token"), "   ");
      expect(screen.getByRole("button", { name: "Connect" })).toBeDisabled();
    });

    it("connects with the typed token on click and clears the field", async () => {
      await renderGroup({ connected: false, identity: null });
      await userEvent.type(screen.getByLabelText("User token"), "xoxp-1");
      await userEvent.click(screen.getByRole("button", { name: "Connect" }));
      expect(mocked.slackConnect).toHaveBeenCalledWith("xoxp-1");
      await vi.waitFor(() => expect(screen.getByLabelText("User token")).toHaveValue(""));
    });

    it("connects on Enter", async () => {
      await renderGroup({ connected: false, identity: null });
      await userEvent.type(screen.getByLabelText("User token"), "xoxp-2{Enter}");
      expect(mocked.slackConnect).toHaveBeenCalledWith("xoxp-2");
    });

    it("keeps the token and toasts the reason when connecting fails", async () => {
      mocked.slackConnect.mockRejectedValue({ kind: "network", message: "invalid_auth", code: null });
      await renderGroup({ connected: false, identity: null });
      await userEvent.type(screen.getByLabelText("User token"), "xoxp-bad{Enter}");
      await vi.waitFor(() => expect(toastMessages()).toContain("Couldn't connect Slack: invalid_auth"));
      expect(screen.getByLabelText("User token")).toHaveValue("xoxp-bad");
    });

    it("shows the connection after Slack announces the token", async () => {
      let changed: () => void = () => {};
      mocked.onSlackChanged.mockImplementation(async (handler) => {
        changed = handler;
        return () => {};
      });
      await renderGroup({ connected: false, identity: null });
      mocked.slackInfo.mockResolvedValue(CONNECTED);
      changed();
      expect(await screen.findByText("Connected")).toBeInTheDocument();
      expect(screen.queryByLabelText("User token")).not.toBeInTheDocument();
    });
  });

  describe("with a token", () => {
    it("shows Connected with who the token belongs to", async () => {
      await renderGroup(CONNECTED);
      expect(screen.getByText("pat in Acme")).toBeInTheDocument();
      expect(screen.queryByLabelText("User token")).not.toBeInTheDocument();
    });

    it("says where the token is stored while the identity is unknown", async () => {
      await renderGroup({ connected: true, identity: null });
      expect(screen.getByText("The token is stored in the Keychain.")).toBeInTheDocument();
    });

    it("checks the token with Test Connection", async () => {
      await renderGroup(CONNECTED);
      await userEvent.click(screen.getByRole("button", { name: "Test Connection" }));
      expect(mocked.slackTest).toHaveBeenCalled();
    });

    it("toasts the reason when the test fails", async () => {
      mocked.slackTest.mockRejectedValue({ kind: "network", message: "token_revoked", code: null });
      await renderGroup(CONNECTED);
      await userEvent.click(screen.getByRole("button", { name: "Test Connection" }));
      await vi.waitFor(() => expect(toastMessages()).toContain("Slack connection failed: token_revoked"));
    });

    it("removes the token with Remove Token", async () => {
      await renderGroup(CONNECTED);
      await userEvent.click(screen.getByRole("button", { name: "Remove Token" }));
      expect(mocked.slackDisconnect).toHaveBeenCalled();
    });

    it("toasts when removing the token fails", async () => {
      mocked.slackDisconnect.mockRejectedValue({ kind: "io", message: "Keychain locked", code: null });
      await renderGroup(CONNECTED);
      await userEvent.click(screen.getByRole("button", { name: "Remove Token" }));
      await vi.waitFor(() => expect(toastMessages()).toContain("Couldn't remove the token: Keychain locked"));
    });
  });
});
