import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type DailyNote } from "../../api";
import { resetStore } from "../../state/store";
import { settings } from "../../test/settings";
import { SettingsWindow } from "./SettingsWindow";

const mocked = vi.mocked(api, true);

function renderPage(patch: Partial<DailyNote> = {}) {
  resetStore({ settings: settings({ dailyNote: { ...settings().dailyNote, ...patch } }) });
  render(<SettingsWindow page="daily-note" onPage={() => {}} />);
}

const saved = () => mocked.updateSettings.mock.lastCall![0].dailyNote;
const level = (section: string) => screen.getByRole("combobox", { name: `${section} heading level` });
const text = (section: string) => screen.getByRole("textbox", { name: `${section} heading text` });
const rowOf = (section: string) => within(level(section).closest<HTMLElement>(".settings-row")!);

describe("DailyNotePage", () => {
  beforeEach(() => vi.clearAllMocks());

  describe("fallback", () => {
    it("shows the fallback folder and format", () => {
      renderPage();
      expect(screen.getByRole("textbox", { name: "Folder" })).toHaveValue("1-Journal");
      expect(screen.getByRole("textbox", { name: "Format" })).toHaveValue("YYYY-MM-DD");
    });

    it("saves an edited folder trimmed, and an emptied one as the vault root", async () => {
      renderPage();
      await userEvent.clear(screen.getByRole("textbox", { name: "Folder" }));
      await userEvent.type(screen.getByRole("textbox", { name: "Folder" }), " Daily ");
      await userEvent.tab();
      expect(saved().fallbackFolder).toBe("Daily");

      await userEvent.clear(screen.getByRole("textbox", { name: "Folder" }));
      await userEvent.tab();
      expect(saved().fallbackFolder).toBe("");
    });

    it("saves an edited format and ignores a blank one", async () => {
      renderPage();
      await userEvent.clear(screen.getByRole("textbox", { name: "Format" }));
      await userEvent.type(screen.getByRole("textbox", { name: "Format" }), "YYYY/MM/DD");
      await userEvent.tab();
      expect(saved().fallbackFormat).toBe("YYYY/MM/DD");

      mocked.updateSettings.mockClear();
      await userEvent.clear(screen.getByRole("textbox", { name: "Format" }));
      await userEvent.tab();
      expect(mocked.updateSettings).not.toHaveBeenCalled();
    });
  });

  describe("sections", () => {
    it("lists the sections in the saved order with their level and text", () => {
      renderPage({ order: ["notes", "journal", "plan", "focus"] });
      expect(screen.getAllByRole("combobox").map((c) => c.getAttribute("aria-label"))).toEqual([
        "Notes heading level",
        "Journal heading level",
        "Routine steps heading level",
        "Focus heading level",
      ]);
      expect(level("Journal")).toHaveValue("##");
      expect(text("Journal")).toHaveValue("Journal");
      expect(text("Routine steps")).toHaveValue("Plan for Today");
    });

    it("reads the notes heading line as level and text", () => {
      renderPage({ notes: "### Scratch" });
      expect(level("Notes")).toHaveValue("###");
      expect(text("Notes")).toHaveValue("Scratch");
    });

    it("shows a notes heading without a level as ## Notes", () => {
      renderPage({ notes: "" });
      expect(level("Notes")).toHaveValue("##");
      expect(text("Notes")).toHaveValue("Notes");
    });

    it("saves a changed level for a section and keeps its text", async () => {
      renderPage();
      await userEvent.selectOptions(level("Focus"), "###");
      expect(saved().focus).toEqual({ level: "###", text: "Focus" });
    });

    it("saves changed heading text trimmed and ignores blank text", async () => {
      renderPage();
      await userEvent.clear(text("Journal"));
      await userEvent.type(text("Journal"), "  Reflection ");
      await userEvent.tab();
      expect(saved().journal).toEqual({ level: "##", text: "Reflection" });

      mocked.updateSettings.mockClear();
      await userEvent.clear(text("Focus"));
      await userEvent.tab();
      expect(mocked.updateSettings).not.toHaveBeenCalled();
      expect(text("Focus")).toHaveValue("Focus");
    });

    it("stores the notes heading as one line", async () => {
      renderPage();
      await userEvent.selectOptions(level("Notes"), "####");
      expect(saved().notes).toBe("#### Notes");

      await userEvent.clear(text("Notes"));
      await userEvent.type(text("Notes"), "Scratch{Enter}");
      expect(saved().notes).toBe("#### Scratch");
    });
  });

  describe("order", () => {
    it("disables Move Up on the first section and Move Down on the last", () => {
      renderPage();
      expect(rowOf("Journal").getByRole("button", { name: "Move Up" })).toBeDisabled();
      expect(rowOf("Notes").getByRole("button", { name: "Move Down" })).toBeDisabled();
    });

    it("saves the order after Move Down", async () => {
      renderPage();
      await userEvent.click(rowOf("Journal").getByRole("button", { name: "Move Down" }));
      expect(saved().order).toEqual(["focus", "journal", "plan", "notes"]);
    });

    it("saves the order after Move Up and shows the sections in it", async () => {
      renderPage();
      await userEvent.click(rowOf("Notes").getByRole("button", { name: "Move Up" }));
      expect(saved().order).toEqual(["journal", "focus", "notes", "plan"]);
      await vi.waitFor(() =>
        expect(screen.getAllByRole("combobox").map((c) => c.getAttribute("aria-label"))).toEqual([
          "Journal heading level",
          "Focus heading level",
          "Notes heading level",
          "Routine steps heading level",
        ]),
      );
    });
  });
});
