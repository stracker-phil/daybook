import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type EndOfDayCheck, type StatusRequest } from "../../api";
import { StatusView } from "./StatusView";

const mocked = vi.mocked(api, true);

const CHECK: EndOfDayCheck = { reminder: "", question: "Is today fully tracked?", url: "" };

function request(patch: Partial<StatusRequest> = {}): StatusRequest {
  return { status: "working", current: null, messages: { working: "re", break: "afk", off: "bye" }, endOfDay: null, ...patch };
}

const message = () => screen.getByRole("textbox", { name: "Message" });
const selected = () => screen.getAllByRole("option").find((o) => o.getAttribute("aria-selected") === "true")?.textContent;
const option = (name: RegExp) => screen.getByRole("option", { name });

describe("StatusView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.statusSet.mockResolvedValue(undefined);
    mocked.panelKeepOpen.mockResolvedValue(undefined);
  });

  describe("opening", () => {
    it("preselects the requested status and suggests its message", () => {
      render(<StatusView request={request({ status: "break" })} />);
      expect(selected()).toContain("Taking a break");
      expect(message()).toHaveValue("afk");
      expect(message()).toHaveFocus();
    });

    it("lists the three statuses with their numbers", () => {
      render(<StatusView request={request()} />);
      expect(screen.getAllByRole("option").map((o) => o.textContent)).toEqual(["⌘1Working and present", "⌘2Taking a break", "⌘3Done for today"]);
    });

    it("marks today's status as current", () => {
      render(<StatusView request={request({ current: "break" })} />);
      expect(within(option(/Taking a break/)).getByText("Current")).toBeInTheDocument();
      expect(screen.getAllByText("Current")).toHaveLength(1);
    });

    it("marks nothing as current before the first status of the day", () => {
      render(<StatusView request={request({ current: null })} />);
      expect(screen.queryByText("Current")).not.toBeInTheDocument();
    });
  });

  describe("picking", () => {
    it("moves down with ArrowDown, wraps, and moves up with ArrowUp", async () => {
      render(<StatusView request={request()} />);
      await userEvent.keyboard("{ArrowDown}");
      expect(selected()).toContain("Taking a break");
      await userEvent.keyboard("{ArrowDown}{ArrowDown}");
      expect(selected()).toContain("Working and present");
      await userEvent.keyboard("{ArrowUp}");
      expect(selected()).toContain("Done for today");
    });

    it("picks with ⌘3 without setting the status", async () => {
      render(<StatusView request={request()} />);
      await userEvent.keyboard("{Meta>}3{/Meta}");
      expect(selected()).toContain("Done for today");
      expect(message()).toHaveValue("bye");
      expect(mocked.statusSet).not.toHaveBeenCalled();
    });

    it("ignores a number without ⌘", async () => {
      render(<StatusView request={request()} />);
      await userEvent.keyboard("3");
      expect(selected()).toContain("Working and present");
      expect(message()).toHaveValue("re3");
    });

    it("lets the suggested message follow the pick", async () => {
      render(<StatusView request={request()} />);
      await userEvent.keyboard("{ArrowDown}");
      expect(message()).toHaveValue("afk");
      await userEvent.keyboard("{ArrowDown}");
      expect(message()).toHaveValue("bye");
    });

    it("keeps typed text when the pick changes", async () => {
      render(<StatusView request={request()} />);
      await userEvent.clear(message());
      await userEvent.type(message(), "heads down");
      await userEvent.keyboard("{ArrowDown}");
      expect(selected()).toContain("Taking a break");
      expect(message()).toHaveValue("heads down");
    });

    it("suggests the message again after the field was emptied", async () => {
      render(<StatusView request={request()} />);
      await userEvent.clear(message());
      await userEvent.keyboard("{ArrowDown}");
      expect(message()).toHaveValue("afk");
    });
  });

  describe("setting", () => {
    it("sets the preselected status with its suggested message on Enter", async () => {
      render(<StatusView request={request()} />);
      await userEvent.keyboard("{Enter}");
      expect(mocked.statusSet).toHaveBeenCalledWith("working", "re", undefined);
    });

    it("sets the picked status with the typed message", async () => {
      render(<StatusView request={request()} />);
      await userEvent.keyboard("{ArrowDown}");
      await userEvent.clear(message());
      await userEvent.type(message(), "lunch{Enter}");
      expect(mocked.statusSet).toHaveBeenCalledWith("break", "lunch", undefined);
    });

    it("sets the clicked status with its suggested message", async () => {
      render(<StatusView request={request()} />);
      await userEvent.click(option(/Taking a break/));
      expect(mocked.statusSet).toHaveBeenCalledWith("break", "afk", undefined);
    });

    it("sets Off directly when there is no end-of-day check", async () => {
      render(<StatusView request={request()} />);
      await userEvent.keyboard("{Meta>}3{/Meta}{Enter}");
      expect(mocked.statusSet).toHaveBeenCalledWith("off", "bye", undefined);
    });

    it("shows the message of a rejected set", async () => {
      mocked.statusSet.mockRejectedValue({ kind: "io", message: "Slack is unreachable", code: null });
      render(<StatusView request={request()} />);
      await userEvent.keyboard("{Enter}");
      expect(await screen.findByText("Slack is unreachable")).toBeInTheDocument();
    });

    it("drops the error once the message is edited", async () => {
      mocked.statusSet.mockRejectedValue({ kind: "io", message: "Slack is unreachable", code: null });
      render(<StatusView request={request()} />);
      await userEvent.keyboard("{Enter}");
      await screen.findByText("Slack is unreachable");
      await userEvent.type(message(), "!");
      expect(screen.queryByText("Slack is unreachable")).not.toBeInTheDocument();
    });
  });

  describe("going Off with an end-of-day check", () => {
    it("asks the check instead of setting", async () => {
      render(<StatusView request={request({ endOfDay: CHECK })} />);
      await userEvent.keyboard("{Meta>}3{/Meta}{Enter}");
      expect(screen.getByText("Done for today · bye")).toBeInTheDocument();
      expect(screen.getByText("Is today fully tracked?")).toBeInTheDocument();
      expect(screen.queryByRole("textbox", { name: "Message" })).not.toBeInTheDocument();
      expect(mocked.statusSet).not.toHaveBeenCalled();
    });

    it("sets Off as done when answered with Y", async () => {
      render(<StatusView request={request({ endOfDay: CHECK })} />);
      await userEvent.keyboard("{Meta>}3{/Meta}{Enter}");
      await userEvent.keyboard("y");
      expect(mocked.statusSet).toHaveBeenCalledWith("off", "bye", "done");
    });

    it("sets Off as incomplete when answered with the Not yet button", async () => {
      render(<StatusView request={request({ endOfDay: CHECK })} />);
      await userEvent.click(option(/Done for today/));
      await userEvent.click(screen.getByRole("button", { name: /Not yet/ }));
      expect(mocked.statusSet).toHaveBeenCalledWith("off", "bye", "incomplete");
    });

    it("carries a typed message into the check and the answer", async () => {
      render(<StatusView request={request({ endOfDay: CHECK })} />);
      await userEvent.keyboard("{Meta>}3{/Meta}");
      await userEvent.clear(message());
      await userEvent.type(message(), "see you{Enter}");
      expect(screen.getByText("Done for today · see you")).toBeInTheDocument();
      await userEvent.keyboard("n");
      expect(mocked.statusSet).toHaveBeenCalledWith("off", "see you", "incomplete");
    });

    it("does not ask for the other statuses", async () => {
      render(<StatusView request={request({ endOfDay: CHECK })} />);
      await userEvent.keyboard("{Enter}");
      expect(mocked.statusSet).toHaveBeenCalledWith("working", "re", undefined);
      expect(screen.queryByText("Is today fully tracked?")).not.toBeInTheDocument();
    });

    it("shows the message of a rejected answer in the check", async () => {
      mocked.statusSet.mockRejectedValue({ kind: "io", message: "Slack is unreachable", code: null });
      render(<StatusView request={request({ endOfDay: CHECK })} />);
      await userEvent.keyboard("{Meta>}3{/Meta}{Enter}");
      await userEvent.keyboard("y");
      expect(await screen.findByText("Slack is unreachable")).toBeInTheDocument();
      expect(screen.getByText("Is today fully tracked?")).toBeInTheDocument();
    });
  });
});
