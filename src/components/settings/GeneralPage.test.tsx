import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { resetStore } from "../../state/store";
import { settings } from "../../test/settings";
import { GeneralPage } from "./GeneralPage";

const mocked = vi.mocked(api, true);

describe("GeneralPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.autostartStatus.mockResolvedValue(true);
    resetStore({ settings: settings() });
  });

  it("shows Launch at login as installed, followed by the Tools group", async () => {
    render(<GeneralPage s={settings()} set={() => {}} />);
    expect(await screen.findByRole("switch", { name: "Launch at login" })).toBeChecked();
    expect(screen.getByRole("heading", { name: "Tools" })).toBeInTheDocument();
  });

  it("no longer offers the Accessibility row", () => {
    render(<GeneralPage s={settings()} set={() => {}} />);
    expect(screen.queryByText("Accessibility")).not.toBeInTheDocument();
  });
});
