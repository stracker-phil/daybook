import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { SlackStatus } from "../../api";
import { SlackStatusField } from "./SlackStatusField";

const status = (patch: Partial<SlackStatus> = {}): SlackStatus => ({ keepStatus: false, emoji: ":white_check_mark:", text: "9:00 - 18:00", presence: "auto", ...patch });

function renderField(value: SlackStatus = status()) {
  const onChange = vi.fn();
  render(<SlackStatusField value={value} onChange={onChange} />);
  return onChange;
}

const edit = () => userEvent.click(screen.getByRole("button", { name: "Edit Slack status" }));
const presence = () => within(screen.getByRole("group", { name: "Presence" }));

describe("SlackStatusField", () => {
  describe("preview", () => {
    it("shows the emoji glyph, the text and the presence", () => {
      renderField();
      expect(screen.getByText("✅")).toBeInTheDocument();
      expect(screen.getByText("9:00 - 18:00")).toBeInTheDocument();
      expect(screen.getByText("Active")).toBeInTheDocument();
    });

    it("shows Away for away presence", () => {
      renderField(status({ presence: "away" }));
      expect(screen.getByText("Away")).toBeInTheDocument();
    });

    it("shows no presence when it stays unchanged", () => {
      renderField(status({ presence: "keep" }));
      expect(screen.queryByText("Active")).not.toBeInTheDocument();
      expect(screen.queryByText("Away")).not.toBeInTheDocument();
    });

    it("shows the shortcode of a custom emoji", () => {
      renderField(status({ emoji: ":party_parrot:" }));
      expect(screen.getByText(":party_parrot:")).toBeInTheDocument();
    });

    it("says Status unchanged instead of emoji and text while keeping the status", () => {
      renderField(status({ keepStatus: true }));
      expect(screen.getByText("Status unchanged")).toBeInTheDocument();
      expect(screen.queryByText("9:00 - 18:00")).not.toBeInTheDocument();
    });

    it("says No status for an empty text", () => {
      renderField(status({ text: "", emoji: "" }));
      expect(screen.getByText("No status")).toBeInTheDocument();
    });
  });

  describe("editing", () => {
    it("shows no rows until Edit… is clicked, then Done closes them", async () => {
      renderField();
      expect(screen.queryByLabelText("Text")).not.toBeInTheDocument();
      await edit();
      expect(screen.getByLabelText("Text")).toBeInTheDocument();
      await userEvent.click(screen.getByRole("button", { name: "Done Editing Slack status" }));
      expect(screen.queryByLabelText("Text")).not.toBeInTheDocument();
    });

    it("shows the current values in the rows", async () => {
      renderField();
      await edit();
      expect(screen.getByRole("switch", { name: "Keep the status" })).not.toBeChecked();
      expect(screen.getByLabelText("Emoji")).toHaveValue(":white_check_mark:");
      expect(screen.getByLabelText("Text")).toHaveValue("9:00 - 18:00");
      expect(presence().getByRole("button", { name: "Active" })).toHaveAttribute("aria-pressed", "true");
    });

    it("reports the full status with the edited text on Enter", async () => {
      const onChange = renderField();
      await edit();
      const text = screen.getByLabelText("Text");
      await userEvent.clear(text);
      await userEvent.type(text, "Heads down{Enter}");
      expect(onChange).toHaveBeenLastCalledWith(status({ text: "Heads down" }));
    });

    it("normalizes a typed emoji name to a shortcode", async () => {
      const onChange = renderField();
      await edit();
      const emoji = screen.getByLabelText("Emoji");
      await userEvent.clear(emoji);
      await userEvent.type(emoji, "coffee{Enter}");
      expect(onChange).toHaveBeenLastCalledWith(status({ emoji: ":coffee:" }));
    });

    it("reports an emptied emoji as empty", async () => {
      const onChange = renderField();
      await edit();
      await userEvent.clear(screen.getByLabelText("Emoji"));
      await userEvent.tab();
      expect(onChange).toHaveBeenLastCalledWith(status({ emoji: "" }));
    });

    it.each([
      ["Active", "auto"],
      ["Away", "away"],
      ["No change", "keep"],
    ] as const)("reports presence %s as %s", async (label, expected) => {
      const onChange = renderField(status({ presence: expected === "auto" ? "away" : "auto" }));
      await edit();
      await userEvent.click(presence().getByRole("button", { name: label }));
      expect(onChange).toHaveBeenLastCalledWith(status({ presence: expected }));
    });

    it("reports keepStatus from the switch", async () => {
      const onChange = renderField();
      await edit();
      await userEvent.click(screen.getByRole("switch", { name: "Keep the status" }));
      expect(onChange).toHaveBeenLastCalledWith(status({ keepStatus: true }));
    });

    it("hides Emoji and Text while keeping the status, but still offers Presence", async () => {
      renderField(status({ keepStatus: true }));
      await edit();
      expect(screen.queryByLabelText("Emoji")).not.toBeInTheDocument();
      expect(screen.queryByLabelText("Text")).not.toBeInTheDocument();
      expect(screen.getByRole("group", { name: "Presence" })).toBeInTheDocument();
    });
  });
});
