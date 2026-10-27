import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import type { Settings } from "../../api";
import { settings } from "../../test/settings";
import { AppearancePage } from "./AppearancePage";

const set = vi.fn<(patch: Partial<Settings>) => void>();

describe("AppearancePage", () => {
  beforeEach(() => vi.clearAllMocks());

  it("shows Look, Text and Day timeline followed by the Rail groups", () => {
    render(<AppearancePage s={settings()} set={set} />);
    for (const title of ["Look", "Text", "Day timeline", "Rail", "Showing the Rail", "Rail look", "Rail tooltips"]) {
      expect(screen.getByRole("heading", { name: title })).toBeInTheDocument();
    }
  });

  it("saves the dark mode scheme picked", async () => {
    render(<AppearancePage s={settings()} set={set} />);
    await userEvent.click(within(screen.getByRole("group", { name: "Dark mode scheme" })).getByRole("button", { name: "Light" }));
    expect(set).toHaveBeenCalledWith({ darkScheme: "light" });
  });

  it("saves a rail change through the same page", async () => {
    render(<AppearancePage s={settings()} set={set} />);
    await userEvent.click(screen.getByRole("switch", { name: "Captions" }));
    expect(set.mock.lastCall![0].rail).toMatchObject({ captions: !settings().rail.captions });
  });
});
