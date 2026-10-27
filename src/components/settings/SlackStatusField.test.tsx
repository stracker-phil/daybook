import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { SlackStatus } from "../../api";
import { SlackStatusEditor } from "./SlackStatusField";

const status = (patch: Partial<SlackStatus> = {}): SlackStatus => ({ keepStatus: false, emoji: ":white_check_mark:", text: "9:00 - 18:00", presence: "auto", ...patch });

function renderEditor(value: SlackStatus = status()) {
  const onChange = vi.fn();
  render(<SlackStatusEditor value={value} onChange={onChange} />);
  return onChange;
}

const bar = () => screen.getByRole("button", { name: "Edit Slack status" });
const edit = () => userEvent.click(bar());

describe("SlackStatusEditor", () => {
  describe("bar", () => {
    it("shows the emoji glyph, the text and the presence", () => {
      renderEditor();
      expect(screen.getByText("✅")).toBeInTheDocument();
      expect(screen.getByText("9:00 - 18:00")).toBeInTheDocument();
      expect(screen.getByText("Active")).toBeInTheDocument();
    });

    it("shows Away for away presence", () => {
      renderEditor(status({ presence: "away" }));
      expect(screen.getByText("Away")).toBeInTheDocument();
    });

    it("shows no presence when it stays unchanged", () => {
      renderEditor(status({ presence: "keep" }));
      expect(screen.queryByText("Active")).not.toBeInTheDocument();
      expect(screen.queryByText("Away")).not.toBeInTheDocument();
      expect(screen.queryByText("No change")).not.toBeInTheDocument();
    });

    it("shows the shortcode of a custom emoji", () => {
      renderEditor(status({ emoji: ":party_parrot:" }));
      expect(screen.getByText(":party_parrot:")).toBeInTheDocument();
    });

    it("says Status unchanged instead of emoji and text while keeping the status", () => {
      renderEditor(status({ keepStatus: true }));
      expect(screen.getByText("Status unchanged")).toBeInTheDocument();
      expect(screen.queryByText("9:00 - 18:00")).not.toBeInTheDocument();
    });

    it("says No status for an empty text", () => {
      renderEditor(status({ text: "", emoji: "" }));
      expect(screen.getByText("No status")).toBeInTheDocument();
    });
  });

  describe("editing", () => {
    it("opens the editor when the bar is clicked and closes it on a second click", async () => {
      renderEditor();
      expect(bar()).toHaveAttribute("aria-expanded", "false");
      expect(screen.queryByLabelText("Status text")).not.toBeInTheDocument();
      await edit();
      expect(bar()).toHaveAttribute("aria-expanded", "true");
      expect(screen.getByLabelText("Status text")).toBeInTheDocument();
      await edit();
      expect(screen.queryByLabelText("Status text")).not.toBeInTheDocument();
    });

    it("shows the current values", async () => {
      renderEditor();
      await edit();
      expect(screen.getByRole("checkbox", { name: "Keep the status" })).not.toBeChecked();
      expect(screen.getByLabelText("Emoji")).toHaveValue(":white_check_mark:");
      expect(screen.getByLabelText("Status text")).toHaveValue("9:00 - 18:00");
      expect(screen.getByRole("combobox", { name: "Presence" })).toHaveValue("auto");
    });

    it("saves the full status with the edited text on Enter", async () => {
      const onChange = renderEditor();
      await edit();
      const text = screen.getByLabelText("Status text");
      await userEvent.clear(text);
      await userEvent.type(text, "Heads down{Enter}");
      expect(onChange).toHaveBeenLastCalledWith(status({ text: "Heads down" }));
    });

    it("saves the edited text on blur", async () => {
      const onChange = renderEditor();
      await edit();
      const text = screen.getByLabelText("Status text");
      await userEvent.clear(text);
      await userEvent.type(text, "Lunch");
      expect(onChange).not.toHaveBeenCalled();
      await userEvent.tab();
      expect(onChange).toHaveBeenLastCalledWith(status({ text: "Lunch" }));
    });

    it("saves a typed emoji name as a shortcode on Enter", async () => {
      const onChange = renderEditor();
      await edit();
      const emoji = screen.getByLabelText("Emoji");
      await userEvent.clear(emoji);
      await userEvent.type(emoji, "coffee{Enter}");
      expect(onChange).toHaveBeenLastCalledWith(status({ emoji: ":coffee:" }));
    });

    it("saves a typed emoji name as a shortcode on blur", async () => {
      const onChange = renderEditor();
      await edit();
      const emoji = screen.getByLabelText("Emoji");
      await userEvent.clear(emoji);
      await userEvent.type(emoji, "computer");
      await userEvent.tab();
      expect(onChange).toHaveBeenLastCalledWith(status({ emoji: ":computer:" }));
    });

    it("saves an emptied emoji as empty", async () => {
      const onChange = renderEditor();
      await edit();
      await userEvent.clear(screen.getByLabelText("Emoji"));
      await userEvent.tab();
      expect(onChange).toHaveBeenLastCalledWith(status({ emoji: "" }));
    });

    it.each([
      ["Active", "auto"],
      ["Away", "away"],
      ["No change", "keep"],
    ] as const)("saves presence %s as %s", async (label, expected) => {
      const onChange = renderEditor(status({ presence: expected === "auto" ? "away" : "auto" }));
      await edit();
      await userEvent.selectOptions(screen.getByRole("combobox", { name: "Presence" }), label);
      expect(onChange).toHaveBeenLastCalledWith(status({ presence: expected }));
    });

    it("saves keepStatus from the checkbox", async () => {
      const onChange = renderEditor();
      await edit();
      await userEvent.click(screen.getByRole("checkbox", { name: "Keep the status" }));
      expect(onChange).toHaveBeenLastCalledWith(status({ keepStatus: true }));
    });

    it("disables Emoji and Status text while keeping the status, but still offers Presence", async () => {
      renderEditor(status({ keepStatus: true }));
      await edit();
      expect(screen.getByLabelText("Emoji")).toBeDisabled();
      expect(screen.getByLabelText("Status text")).toBeDisabled();
      expect(screen.getByRole("combobox", { name: "Presence" })).toBeEnabled();
    });
  });
});
