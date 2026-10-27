import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { resetStore } from "../../state/store";
import { settings } from "../../test/settings";
import { ConnectionsPage } from "./ConnectionsPage";

const mocked = vi.mocked(api, true);

describe("ConnectionsPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.vaultInfo.mockResolvedValue({ dailyNotes: null, todayNote: null, todayNoteExists: false, obsidianCli: null });
    mocked.calendarInfo.mockResolvedValue({ access: "full-access", calendars: [] });
    mocked.onCalendarChanged.mockResolvedValue(() => {});
    mocked.slackInfo.mockResolvedValue({ connected: false, identity: null });
    mocked.onSlackChanged.mockResolvedValue(() => {});
    resetStore({ settings: settings() });
  });

  it("renders the Vault, Obsidian CLI, Calendar, Slack, Jira and TimeTracker groups", async () => {
    render(<ConnectionsPage s={settings()} set={() => {}} />);
    for (const title of ["Vault", "Obsidian CLI", "Calendar", "Slack", "Jira", "TimeTracker"]) {
      expect(screen.getByRole("heading", { name: title })).toBeInTheDocument();
    }
    expect(await screen.findByText("Allowed")).toBeInTheDocument();
  });
});
