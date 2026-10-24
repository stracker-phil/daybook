import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { OfferOffView } from "./OfferOffView";

const mocked = vi.mocked(api, true);

describe("OfferOffView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.runCommand.mockResolvedValue(undefined);
    mocked.panelClose.mockResolvedValue(undefined);
  });

  it("names the context switched to and asks whether to set Off", () => {
    render(<OfferOffView offer={{ to: "🐈 Personal" }} />);
    expect(screen.getByText("Switched to 🐈 Personal")).toBeInTheDocument();
    expect(screen.getByText(/shows no status, but you’re still set to Working/)).toBeInTheDocument();
  });

  it("focuses Set to Off", () => {
    render(<OfferOffView offer={{ to: "🐈 Personal" }} />);
    expect(screen.getByRole("button", { name: "Set to Off" })).toHaveFocus();
  });

  it("runs the status:off command from Set to Off, also with Enter", async () => {
    render(<OfferOffView offer={{ to: "🐈 Personal" }} />);
    await userEvent.keyboard("{Enter}");
    expect(mocked.runCommand).toHaveBeenCalledWith("status:off");
  });

  it("closes the overlay from Keep Working without changing the status", async () => {
    render(<OfferOffView offer={{ to: "🐈 Personal" }} />);
    await userEvent.click(screen.getByRole("button", { name: "Keep Working" }));
    expect(mocked.panelClose).toHaveBeenCalled();
    expect(mocked.runCommand).not.toHaveBeenCalled();
  });

  it("shows the message of a rejected command", async () => {
    mocked.runCommand.mockRejectedValue({ kind: "io", message: "Slack is unreachable", code: null });
    render(<OfferOffView offer={{ to: "🐈 Personal" }} />);
    await userEvent.click(screen.getByRole("button", { name: "Set to Off" }));
    expect(await screen.findByText("Slack is unreachable")).toBeInTheDocument();
  });
});
