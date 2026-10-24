import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type Context, type Settings } from "../../api";
import { resetStore } from "../../state/store";
import { settings } from "../../test/settings";
import { SettingsWindow } from "./SettingsWindow";

const mocked = vi.mocked(api, true);

function renderPage(patch: Partial<Settings> = {}) {
  const s = settings(patch);
  resetStore({ settings: s });
  render(<SettingsWindow page="contexts" onPage={() => {}} />);
  return s;
}

const card = (title: string) => within(screen.getByRole("heading", { name: title }).closest("section")!);
const work = () => card("[W] Work");
const priv = () => card("[P] Private");

const saved = () => mocked.updateSettings.mock.lastCall![0].contexts;
const savedContext = (id: string) => saved().find((c) => c.id === id)!;

describe("ContextsPage", () => {
  beforeEach(() => vi.clearAllMocks());

  describe("cards", () => {
    it("shows one card per context, titled with its emoji and name", () => {
      renderPage();
      expect(screen.getAllByRole("heading", { level: 3 }).map((h) => h.textContent)).toEqual(["[W] Work", "[P] Private"]);
    });

    it("fills each card with the context's values", () => {
      renderPage();
      expect(work().getByRole("textbox", { name: "Name" })).toHaveValue("Work");
      expect(work().getByRole("textbox", { name: "Emoji" })).toHaveValue("[W]");
      expect(work().getByRole("checkbox", { name: "Jira" })).toBeChecked();
      expect(priv().getByRole("checkbox", { name: "Jira" })).not.toBeChecked();
      expect(work().getByRole("radio", { name: "blue" })).toBeChecked();
      expect(priv().getByRole("radio", { name: "green" })).toBeChecked();
    });

    it("offers all and the search scopes once, with the context's scope selected", () => {
      renderPage();
      const select = work().getByRole("combobox", { name: "Search scope" });
      expect(within(select).getAllByRole("option").map((o) => o.textContent)).toEqual(["all", "journal", "work", "private"]);
      expect(select).toHaveValue("work");
    });

    it("offers the work hours switch with days, times and auto-switch only when work hours are on", () => {
      renderPage();
      expect(work().getByRole("switch", { name: "Work hours" })).toBeChecked();
      expect(work().getByRole("group", { name: "Work days" })).toBeInTheDocument();
      expect(work().getByLabelText("Work hours from")).toHaveValue("09:00");
      expect(work().getByRole("switch", { name: "Switch automatically during work hours" })).toBeChecked();

      expect(priv().getByRole("switch", { name: "Work hours" })).not.toBeChecked();
      expect(priv().queryByRole("group", { name: "Work days" })).not.toBeInTheDocument();
      expect(priv().queryByRole("switch", { name: "Switch automatically during work hours" })).not.toBeInTheDocument();
    });
  });

  describe("editing", () => {
    it("saves a ticked feature on that context only", async () => {
      renderPage();
      await userEvent.click(priv().getByRole("checkbox", { name: "Jira" }));
      expect(savedContext("private").features).toEqual(["jira"]);
      expect(savedContext("work").features).toEqual(["status", "slack", "jira", "routines"]);
    });

    it("saves an unticked feature", async () => {
      renderPage();
      await userEvent.click(work().getByRole("checkbox", { name: "Jira" }));
      expect(savedContext("work").features).toEqual(["status", "slack", "routines"]);
    });

    it("saves a renamed context on blur, trimmed", async () => {
      renderPage();
      await userEvent.clear(work().getByRole("textbox", { name: "Name" }));
      await userEvent.type(work().getByRole("textbox", { name: "Name" }), "  Job  ");
      await userEvent.tab();
      expect(savedContext("work").name).toBe("Job");
    });

    it("keeps the name when it is emptied", async () => {
      renderPage();
      await userEvent.clear(work().getByRole("textbox", { name: "Name" }));
      await userEvent.tab();
      expect(mocked.updateSettings).not.toHaveBeenCalled();
    });

    it("saves a new emoji on Enter", async () => {
      renderPage();
      await userEvent.clear(priv().getByRole("textbox", { name: "Emoji" }));
      await userEvent.type(priv().getByRole("textbox", { name: "Emoji" }), "🏠{Enter}");
      expect(savedContext("private").label).toBe("🏠");
    });

    it("saves the clicked colour", async () => {
      renderPage();
      await userEvent.click(priv().getByRole("radio", { name: "pink" }));
      expect(savedContext("private").color).toBe("pink");
    });

    it("saves the chosen search scope", async () => {
      renderPage();
      await userEvent.selectOptions(work().getByRole("combobox", { name: "Search scope" }), "journal");
      expect(savedContext("work").searchScope).toBe("journal");
    });

    it("saves toggled work days", async () => {
      renderPage();
      await userEvent.click(work().getByRole("button", { name: "Mo" }));
      expect(savedContext("work").workHours).toEqual({ days: [2, 3, 4, 5], from: "09:00", to: "18:00" });
    });

    it("saves a changed start time on blur", async () => {
      renderPage();
      await userEvent.click(work().getByLabelText("Work hours from"));
      fireEvent.change(work().getByLabelText("Work hours from"), { target: { value: "08:00" } });
      await userEvent.tab();
      expect(savedContext("work").workHours).toEqual({ days: [1, 2, 3, 4, 5], from: "08:00", to: "18:00" });
    });

    it("turns work hours on with a weekday default", async () => {
      renderPage();
      await userEvent.click(priv().getByRole("switch", { name: "Work hours" }));
      expect(savedContext("private").workHours).toEqual({ days: [1, 2, 3, 4, 5], from: "09:00", to: "18:00" });
    });

    it("saves no work hours when the switch is turned off and hides the days, hours and auto-switch rows", async () => {
      renderPage();
      await userEvent.click(work().getByRole("switch", { name: "Work hours" }));
      expect(savedContext("work").workHours).toBeNull();
      await vi.waitFor(() => expect(work().queryByRole("switch", { name: "Switch automatically during work hours" })).not.toBeInTheDocument());
      expect(work().queryByRole("group", { name: "Work days" })).not.toBeInTheDocument();
      expect(work().queryByLabelText("Work hours from")).not.toBeInTheDocument();
    });

    it("saves the auto-switch switch", async () => {
      renderPage();
      await userEvent.click(work().getByRole("switch", { name: "Switch automatically during work hours" }));
      expect(savedContext("work").autoSwitch).toBe(false);
    });
  });

  describe("list actions", () => {
    it("disables Move Up on the first card and Move Down on the last", () => {
      renderPage();
      expect(work().getByRole("button", { name: "Move Up" })).toBeDisabled();
      expect(priv().getByRole("button", { name: "Move Down" })).toBeDisabled();
    });

    it("saves the order after Move Down", async () => {
      renderPage();
      await userEvent.click(work().getByRole("button", { name: "Move Down" }));
      expect(saved().map((c) => c.id)).toEqual(["private", "work"]);
    });

    it("saves the order after Move Up", async () => {
      renderPage();
      await userEvent.click(priv().getByRole("button", { name: "Move Up" }));
      expect(saved().map((c) => c.id)).toEqual(["private", "work"]);
    });

    it("saves the list without the removed context", async () => {
      renderPage();
      await userEvent.click(work().getByRole("button", { name: "Remove Context" }));
      expect(saved().map((c) => c.id)).toEqual(["private"]);
    });

    it("appends a New context card with the first unused colour", async () => {
      renderPage();
      await userEvent.click(screen.getByRole("button", { name: "Add Context" }));
      expect(saved()).toHaveLength(3);
      expect(saved()[2]).toEqual(
        expect.objectContaining<Partial<Context>>({ name: "New context", label: "", color: "purple", features: [], searchScope: "all", workHours: null, autoSwitch: false }),
      );
      expect(await screen.findByRole("heading", { name: "New context" })).toBeInTheDocument();
    });

    it("falls back to blue when every colour is in use", async () => {
      const base = settings().contexts[0];
      const colors = ["blue", "purple", "pink", "red", "orange", "yellow", "green", "teal", "graphite"] as const;
      renderPage({ contexts: colors.map((color) => ({ ...base, id: color, name: color, label: "", color })) });
      await userEvent.click(screen.getByRole("button", { name: "Add Context" }));
      expect(saved()[9].color).toBe("blue");
    });
  });
});
