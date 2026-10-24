import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type EndOfDayCheck } from "../../api";
import { EndOfDayStep } from "./EndOfDayStep";

const mocked = vi.mocked(api, true);

const CHECK: EndOfDayCheck = { reminder: "Book your hours", question: "Is today fully tracked?", url: "https://example.com/timesheets" };

function renderStep(patch: Partial<EndOfDayCheck> = {}, props: { message?: string; error?: string | null } = {}) {
  const onAnswer = vi.fn();
  render(<EndOfDayStep check={{ ...CHECK, ...patch }} message={props.message ?? "bye"} error={props.error ?? null} onAnswer={onAnswer} />);
  return onAnswer;
}

describe("EndOfDayStep", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.panelKeepOpen.mockResolvedValue(undefined);
    mocked.openUrl.mockResolvedValue(undefined);
  });

  it("keeps the overlay open while the owner checks the link", () => {
    renderStep();
    expect(mocked.panelKeepOpen).toHaveBeenCalledWith(true);
  });

  it("focuses the Yes button", () => {
    renderStep();
    expect(screen.getByRole("button", { name: /Yes/ })).toHaveFocus();
  });

  it("titles the check with the message", () => {
    renderStep();
    expect(screen.getByText("Done for today · bye")).toBeInTheDocument();
  });

  it("titles the check without a message as Done for today only", () => {
    renderStep({}, { message: "" });
    expect(screen.getByText("Done for today")).toBeInTheDocument();
  });

  describe("answering", () => {
    it.each([
      ["y", "done"],
      ["Y", "done"],
      ["n", "incomplete"],
    ])("answers %s as %s", async (key, answer) => {
      const onAnswer = renderStep();
      await userEvent.keyboard(key);
      expect(onAnswer).toHaveBeenCalledWith(answer);
    });

    it("ignores Y with ⌘ held", async () => {
      const onAnswer = renderStep();
      await userEvent.keyboard("{Meta>}y{/Meta}");
      expect(onAnswer).not.toHaveBeenCalled();
    });

    it("answers done with the Yes button and incomplete with Not yet", async () => {
      const onAnswer = renderStep();
      await userEvent.click(screen.getByRole("button", { name: /Not yet/ }));
      expect(onAnswer).toHaveBeenLastCalledWith("incomplete");
      await userEvent.click(screen.getByRole("button", { name: /Yes/ }));
      expect(onAnswer).toHaveBeenLastCalledWith("done");
    });
  });

  describe("link", () => {
    it("opens the link", async () => {
      renderStep();
      await userEvent.click(screen.getByRole("button", { name: "Open" }));
      expect(mocked.openUrl).toHaveBeenCalledWith("https://example.com/timesheets");
    });

    it("has no Open button without a link", () => {
      renderStep({ url: "" });
      expect(screen.queryByRole("button", { name: "Open" })).not.toBeInTheDocument();
    });
  });

  describe("numbering", () => {
    it("numbers the reminder 1 and the question 2", () => {
      renderStep();
      expect(screen.getByText("Book your hours").closest("p")).toHaveTextContent("1.Book your hours");
      expect(screen.getByText("2.")).toBeInTheDocument();
    });

    it("shows no numbers without a reminder", () => {
      renderStep({ reminder: "" });
      expect(screen.getByText("Is today fully tracked?")).toBeInTheDocument();
      expect(screen.queryByText("1.")).not.toBeInTheDocument();
      expect(screen.queryByText("2.")).not.toBeInTheDocument();
    });
  });

  it("shows an error in place of the hint", () => {
    renderStep({}, { error: "Slack is unreachable" });
    expect(screen.getByText("Slack is unreachable")).toBeInTheDocument();
    expect(screen.queryByText("esc cancel, send nothing")).not.toBeInTheDocument();
  });
});
