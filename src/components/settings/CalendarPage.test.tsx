import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type CalendarAccess } from "../../api";
import { resetStore } from "../../state/store";
import { settings } from "../../test/settings";
import { SettingsWindow } from "./SettingsWindow";

const mocked = vi.mocked(api, true);

const DENIED_HINT = "Turn on access for Daybook in System Settings → Privacy & Security → Calendars.";

function renderPage(access: CalendarAccess) {
  mocked.calendarInfo.mockResolvedValue({ access, calendars: access === "full-access" ? ["Work"] : [] });
  mocked.onCalendarChanged.mockResolvedValue(() => {});
  resetStore({ settings: settings() });
  render(<SettingsWindow page="calendar" onPage={() => {}} />);
}

describe("CalendarPage", () => {
  beforeEach(() => vi.clearAllMocks());

  it("shows Allowed and no button when Daybook has full access", async () => {
    renderPage("full-access");
    expect(await screen.findByText("Allowed")).toBeInTheDocument();
    expect(screen.getByText("Daybook can read your calendar events.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Allow Access|Open Privacy Settings/ })).not.toBeInTheDocument();
  });

  it("offers Allow Access when macOS has not asked yet", async () => {
    renderPage("not-determined");
    expect(await screen.findByRole("button", { name: "Allow Access…" })).toBeInTheDocument();
    expect(screen.getByText("macOS asks once.")).toBeInTheDocument();
  });

  it.each<CalendarAccess>(["denied", "restricted", "write-only"])("offers Open Privacy Settings with the System Settings hint when access is %s", async (access) => {
    renderPage(access);
    expect(await screen.findByRole("button", { name: "Open Privacy Settings" })).toBeInTheDocument();
    expect(screen.getByText(DENIED_HINT)).toBeInTheDocument();
  });

  it("requests access on click and shows the new state once it is granted", async () => {
    mocked.calendarRequestAccess.mockResolvedValue("full-access");
    renderPage("not-determined");
    mocked.calendarInfo.mockResolvedValue({ access: "full-access", calendars: ["Work"] });
    await userEvent.click(await screen.findByRole("button", { name: "Allow Access…" }));
    expect(await screen.findByText("Allowed")).toBeInTheDocument();
    expect(mocked.calendarRequestAccess).toHaveBeenCalled();
  });

  it("requests access from the Open Privacy Settings button too", async () => {
    mocked.calendarRequestAccess.mockResolvedValue("denied");
    renderPage("denied");
    await userEvent.click(await screen.findByRole("button", { name: "Open Privacy Settings" }));
    expect(mocked.calendarRequestAccess).toHaveBeenCalled();
  });

  it("keeps showing the page and reloads the access when the request fails", async () => {
    mocked.calendarRequestAccess.mockRejectedValue({ kind: "io", message: "No EventKit", code: null });
    renderPage("not-determined");
    mocked.calendarInfo.mockResolvedValue({ access: "denied", calendars: [] });
    await userEvent.click(await screen.findByRole("button", { name: "Allow Access…" }));
    expect(await screen.findByRole("button", { name: "Open Privacy Settings" })).toBeInTheDocument();
  });

  it("reads the access again when the calendar changes", async () => {
    let changed: () => void = () => {};
    mocked.onCalendarChanged.mockImplementation(async (handler) => {
      changed = handler;
      return () => {};
    });
    mocked.calendarInfo.mockResolvedValue({ access: "denied", calendars: [] });
    resetStore({ settings: settings() });
    render(<SettingsWindow page="calendar" onPage={() => {}} />);
    await screen.findByRole("button", { name: "Open Privacy Settings" });

    mocked.calendarInfo.mockResolvedValue({ access: "full-access", calendars: [] });
    changed();
    expect(await screen.findByText("Allowed")).toBeInTheDocument();
  });
});
