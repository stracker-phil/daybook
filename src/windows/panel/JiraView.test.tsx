import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { JiraView } from "./JiraView";

const mocked = vi.mocked(api, true);

async function renderView(suggestions: string[]) {
  mocked.jiraSuggestions.mockResolvedValue(suggestions);
  render(<JiraView />);
  await vi.waitFor(() => expect(mocked.jiraSuggestions).toHaveBeenCalled());
  if (suggestions.length > 0) await screen.findByRole("listbox", { name: "Last 7 days" });
}

const input = () => screen.getByRole("textbox", { name: "Open Jira issue" });
const keys = () => screen.queryAllByRole("option").map((o) => o.textContent);
const selected = () => screen.queryAllByRole("option").find((o) => o.getAttribute("aria-selected") === "true")?.textContent;

describe("JiraView", () => {
  beforeEach(() => vi.clearAllMocks());

  it("shows no list without suggestions", async () => {
    await renderView([]);
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });

  it("lists the suggestions under a heading with nothing highlighted", async () => {
    await renderView(["QRS-1", "QRS-2"]);
    expect(screen.getByText("Last 7 Days")).toBeInTheDocument();
    expect(keys()).toEqual(["QRS-1", "QRS-2"]);
    expect(selected()).toBeUndefined();
  });

  it("shows at most eight suggestions", async () => {
    await renderView(Array.from({ length: 12 }, (_, i) => `QRS-${i + 1}`));
    expect(screen.getAllByRole("option")).toHaveLength(8);
  });

  describe("typing", () => {
    it("narrows the suggestions fuzzily, best match first", async () => {
      await renderView(["ABC-681", "QRS-6805", "QRS-12"]);
      await userEvent.type(input(), "681");
      expect(keys()).toEqual(["ABC-681"]);
      await userEvent.clear(input());
      await userEvent.type(input(), "qrs");
      expect(keys()).toEqual(["QRS-6805", "QRS-12"]);
    });

    it("offers to open the text when every suggestion is hidden", async () => {
      await renderView(["QRS-1"]);
      await userEvent.type(input(), "zzz");
      expect(screen.getByText("Press ↵ to open it.")).toBeInTheDocument();
      expect(screen.queryAllByRole("option")).toHaveLength(0);
    });
  });

  describe("arrow keys", () => {
    it("put the suggestion's key into the field and return to your own text from the top", async () => {
      await renderView(["QRS-1", "QRS-2"]);
      await userEvent.type(input(), "q");
      await userEvent.keyboard("{ArrowDown}");
      expect(input()).toHaveValue("QRS-1");
      expect(selected()).toBe("QRS-1");
      await userEvent.keyboard("{ArrowUp}");
      expect(input()).toHaveValue("q");
      expect(selected()).toBeUndefined();
    });

    it("wrap from your own text to the last suggestion and back", async () => {
      await renderView(["QRS-1", "QRS-2"]);
      await userEvent.keyboard("{ArrowUp}");
      expect(input()).toHaveValue("QRS-2");
      await userEvent.keyboard("{ArrowDown}");
      expect(input()).toHaveValue("");
    });
  });

  describe("opening", () => {
    it("opens what the field shows with Enter", async () => {
      await renderView(["QRS-1"]);
      await userEvent.type(input(), "6805{Enter}");
      expect(mocked.jiraOpen).toHaveBeenCalledWith("6805");
    });

    it("opens the highlighted suggestion with Enter", async () => {
      await renderView(["QRS-1", "QRS-2"]);
      await userEvent.keyboard("{ArrowDown}{ArrowDown}{Enter}");
      expect(mocked.jiraOpen).toHaveBeenCalledWith("QRS-2");
    });

    it("does nothing for a blank field", async () => {
      await renderView([]);
      await userEvent.type(input(), "   {Enter}");
      expect(mocked.jiraOpen).not.toHaveBeenCalled();
    });

    it("opens a clicked suggestion", async () => {
      await renderView(["QRS-1", "QRS-2"]);
      await userEvent.click(screen.getByRole("option", { name: "QRS-2" }));
      expect(mocked.jiraOpen).toHaveBeenCalledWith("QRS-2");
    });
  });

  describe("a rejected open", () => {
    it("shows its message in the footer until the next keystroke", async () => {
      mocked.jiraOpen.mockRejectedValue({ kind: "not_found", message: "No such issue", code: null });
      await renderView([]);
      await userEvent.type(input(), "9{Enter}");
      expect(await screen.findByText("No such issue")).toBeInTheDocument();
      await userEvent.type(input(), "1");
      expect(screen.queryByText("No such issue")).not.toBeInTheDocument();
    });
  });
});
