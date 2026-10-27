import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type Settings, type VaultInfo } from "../../api";
import { resetStore } from "../../state/store";
import { settings } from "../../test/settings";
import { SettingsWindow } from "./SettingsWindow";

const mocked = vi.mocked(api, true);

const VAULT = { path: "/Users/me/Obsidian/Vault", name: "Vault", obsidianCliPath: "" };
const info = (patch: Partial<VaultInfo> = {}): VaultInfo => ({ dailyNotes: null, todayNote: null, todayNoteExists: false, obsidianCli: null, ...patch });

function renderPage(patch: Partial<Settings> = {}, vaultInfo: VaultInfo = info()) {
  mocked.vaultInfo.mockResolvedValue(vaultInfo);
  resetStore({ settings: settings({ vault: VAULT, ...patch }) });
  render(<SettingsWindow page="vault" onPage={() => {}} />);
}

const rowOf = (label: string) => within(screen.getByText(label).closest<HTMLElement>(".settings-row")!);
const savedVault = () => mocked.updateSettings.mock.lastCall![0].vault;

describe("VaultPage", () => {
  beforeEach(() => vi.clearAllMocks());

  describe("vault folder", () => {
    it("shows the folder with the home directory as ~ and offers Change…", () => {
      renderPage();
      expect(screen.getByText("~/Obsidian/Vault")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Change…" })).toBeInTheDocument();
    });

    it("asks to choose a folder, and hides the vault details, while none is set", () => {
      renderPage({ vault: { ...VAULT, path: null } });
      expect(screen.getByText("Choose the folder of your Obsidian vault.")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Choose…" })).toBeInTheDocument();
      expect(screen.queryByRole("textbox", { name: "Vault name" })).not.toBeInTheDocument();
    });

    it("saves the chosen folder as the vault path and keeps the rest of the vault settings", async () => {
      mocked.chooseFolder.mockResolvedValue("/Users/me/Notes");
      renderPage({ vault: { ...VAULT, path: null, name: "Notes" } });
      await userEvent.click(screen.getByRole("button", { name: "Choose…" }));
      await vi.waitFor(() => expect(savedVault()).toEqual({ path: "/Users/me/Notes", name: "Notes", obsidianCliPath: "" }));
      expect(await screen.findByText("~/Notes")).toBeInTheDocument();
    });

    it("saves nothing when the folder dialog is cancelled", async () => {
      mocked.chooseFolder.mockResolvedValue(null);
      renderPage();
      await userEvent.click(screen.getByRole("button", { name: "Change…" }));
      await vi.waitFor(() => expect(mocked.chooseFolder).toHaveBeenCalled());
      expect(mocked.updateSettings).not.toHaveBeenCalled();
    });
  });

  describe("vault name", () => {
    it("shows the saved name and saves an edited one trimmed", async () => {
      renderPage();
      const field = screen.getByRole("textbox", { name: "Vault name" });
      expect(field).toHaveValue("Vault");
      await userEvent.clear(field);
      await userEvent.type(field, "  Thought Forge  ");
      await userEvent.tab();
      expect(savedVault()).toEqual({ ...VAULT, name: "Thought Forge" });
    });

    it("saves nothing when the name is blanked", async () => {
      renderPage();
      await userEvent.clear(screen.getByRole("textbox", { name: "Vault name" }));
      await userEvent.tab();
      expect(mocked.updateSettings).not.toHaveBeenCalled();
    });
  });

  describe("daily notes and today's note", () => {
    it("shows Obsidian's daily-notes location when it was read from Obsidian", async () => {
      renderPage({}, info({ dailyNotes: { folder: "Journal", format: "YYYY-MM-DD", fromObsidian: true } }));
      expect(await screen.findByText("Journal/YYYY-MM-DD")).toBeInTheDocument();
      expect(screen.getByText("From Obsidian's Daily notes settings.")).toBeInTheDocument();
    });

    it("names the fallback as the source and shows a format without folder as is", async () => {
      renderPage({}, info({ dailyNotes: { folder: "", format: "YYYY-MM-DD", fromObsidian: false } }));
      expect(await screen.findByText("YYYY-MM-DD")).toBeInTheDocument();
      expect(screen.getByText(/^Fallback \(Settings/)).toBeInTheDocument();
    });

    it("shows today's note as found with its path", async () => {
      renderPage({}, info({ todayNote: "/Users/me/Obsidian/Vault/Journal/2026-10-24.md", todayNoteExists: true }));
      expect(await screen.findByText("~/Obsidian/Vault/Journal/2026-10-24.md")).toBeInTheDocument();
      expect(rowOf("Today's note").getByText("Found")).toBeInTheDocument();
    });

    it("shows today's note as not created yet", async () => {
      renderPage({}, info({ todayNote: "/Users/me/Obsidian/Vault/Journal/2026-10-24.md", todayNoteExists: false }));
      expect(await screen.findByText("Not created yet")).toBeInTheDocument();
    });

    it("shows no daily-notes or today's-note rows before the vault is read", () => {
      mocked.vaultInfo.mockReturnValue(new Promise(() => {}));
      resetStore({ settings: settings({ vault: VAULT }) });
      render(<SettingsWindow page="vault" onPage={() => {}} />);
      expect(screen.queryByText("Today's note")).not.toBeInTheDocument();
      expect(screen.queryByText("Daily notes")).not.toBeInTheDocument();
    });
  });

  describe("Obsidian CLI", () => {
    it("shows Found with the CLI's path", async () => {
      renderPage({}, info({ obsidianCli: "/Users/me/bin/obsidian" }));
      expect(await screen.findByText("~/bin/obsidian")).toBeInTheDocument();
      const statusRow = screen.getAllByText("Status").map((el) => el.closest<HTMLElement>(".settings-row")).find((row) => row !== null);
      expect(within(statusRow!).getByText("Found")).toBeInTheDocument();
    });

    it("shows Missing when the CLI was not found", async () => {
      renderPage({}, info({ obsidianCli: null }));
      expect(await screen.findByText("Missing")).toBeInTheDocument();
      expect(screen.getByText("Not found.")).toBeInTheDocument();
    });

    it("saves a custom path trimmed", async () => {
      renderPage();
      await userEvent.type(screen.getByRole("textbox", { name: "Custom path" }), "  /opt/obsidian ");
      await userEvent.tab();
      expect(savedVault()).toEqual({ ...VAULT, obsidianCliPath: "/opt/obsidian" });
    });

    it("saves an emptied custom path as empty, so the CLI is found automatically", async () => {
      renderPage({ vault: { ...VAULT, obsidianCliPath: "/opt/obsidian" } });
      await userEvent.clear(screen.getByRole("textbox", { name: "Custom path" }));
      await userEvent.tab();
      expect(savedVault().obsidianCliPath).toBe("");
    });
  });
});
