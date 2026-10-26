import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type LinkItem } from "../../api";
import { LinksView } from "./LinksView";

const mocked = vi.mocked(api, true);

const link = (title: string): LinkItem => ({ title, target: `https://example.com/${title}`, command: `link:${title}` });
const HINTS = "↑↓ link · ↵ open · ⌘1–9 open · esc close";

async function renderView(now: LinkItem[], more: LinkItem[]) {
  mocked.linksView.mockResolvedValue({ now, more });
  render(<LinksView />);
  await screen.findByRole("listbox", { name: "Links" });
}

const input = () => screen.getByRole("textbox", { name: "Link" });
const titles = () => screen.queryAllByRole("option").map((o) => o.querySelector(".grow-text")?.textContent);
const selected = () => screen.getAllByRole("option").find((o) => o.getAttribute("aria-selected") === "true")?.textContent;

describe("LinksView", () => {
  beforeEach(() => vi.clearAllMocks());

  it("lists the links that show now and the other links under their headings, numbering the rows", async () => {
    await renderView([link("Board")], [link("Wiki"), link("Docs")]);
    expect(await screen.findByText("Now")).toBeInTheDocument();
    expect(screen.getByText("More links")).toBeInTheDocument();
    expect(screen.getAllByRole("option").map((o) => o.textContent)).toEqual(["⌘1Board", "⌘2Wiki", "⌘3Docs"]);
    expect(screen.getByText(HINTS)).toBeInTheDocument();
  });

  it("numbers only the first nine rows", async () => {
    const many = Array.from({ length: 10 }, (_, i) => link(`Link${i + 1}`));
    await renderView(many, []);
    const options = await screen.findAllByRole("option");
    expect(options[8]).toHaveTextContent("⌘9Link9");
    expect(options[9]).toHaveTextContent(/^Link10$/);
  });

  it("opens the selected row's command with Enter", async () => {
    await renderView([link("Board")], [link("Wiki")]);
    await userEvent.keyboard("{ArrowDown}{Enter}");
    expect(mocked.runCommand).toHaveBeenCalledWith("link:Wiki");
  });

  describe("selection", () => {
    it("wraps with the arrow keys", async () => {
      await renderView([link("Board")], [link("Wiki")]);
      await userEvent.keyboard("{ArrowUp}");
      expect(selected()).toBe("⌘2Wiki");
      await userEvent.keyboard("{ArrowDown}");
      expect(selected()).toBe("⌘1Board");
    });

    it("selects an unselected row on click and opens it on the next click", async () => {
      await renderView([link("Board")], [link("Wiki")]);
      await userEvent.click(await screen.findByRole("option", { name: /Wiki/ }));
      expect(selected()).toBe("⌘2Wiki");
      expect(mocked.runCommand).not.toHaveBeenCalled();
      await userEvent.click(screen.getByRole("option", { name: /Wiki/ }));
      expect(mocked.runCommand).toHaveBeenCalledWith("link:Wiki");
    });
  });

  describe("filtering", () => {
    it("keeps the rows whose title has every typed word and selects the first again", async () => {
      await renderView([link("Team Board")], [link("Team Wiki"), link("Docs Board")]);
      await userEvent.keyboard("{ArrowDown}");
      await userEvent.type(input(), "board team");
      expect(titles()).toEqual(["Team Board"]);
      expect(selected()).toBe("⌘1Team Board");
    });

    it("says so when nothing matches", async () => {
      await renderView([link("Board")], []);
      await userEvent.type(input(), "zzz");
      expect(screen.getByText("No matching links.")).toBeInTheDocument();
    });
  });

  it("explains where to add links when there are none", async () => {
    await renderView([], []);
    expect(await screen.findByText("No links yet. Add them in Settings → Links.")).toBeInTheDocument();
  });

  describe("⌘ plus a digit", () => {
    it("opens that row", async () => {
      await renderView([link("Board")], [link("Wiki")]);
      await userEvent.keyboard("{Meta>}2{/Meta}");
      expect(mocked.runCommand).toHaveBeenCalledWith("link:Wiki");
    });

    it("does nothing past the last row", async () => {
      await renderView([link("Board")], []);
      await userEvent.keyboard("{Meta>}2{/Meta}");
      expect(mocked.runCommand).not.toHaveBeenCalled();
    });

    it.each(["Control", "Alt"])("is ignored together with %s", async (modifier) => {
      await renderView([link("Board")], []);
      await userEvent.keyboard(`{${modifier}>}{Meta>}1{/Meta}{/${modifier}}`);
      expect(mocked.runCommand).not.toHaveBeenCalled();
    });
  });

  describe("errors", () => {
    it("shows the message of a failed command in the footer instead of the hints", async () => {
      mocked.runCommand.mockRejectedValue({ kind: "io", message: "Cannot open", code: null });
      await renderView([link("Board")], []);
      await userEvent.keyboard("{Enter}");
      expect(await screen.findByText("Cannot open")).toBeInTheDocument();
      expect(screen.queryByText(HINTS)).not.toBeInTheDocument();
    });

    it("shows the message of a failed load in the footer", async () => {
      mocked.linksView.mockRejectedValue({ kind: "io", message: "Settings unreadable", code: null });
      render(<LinksView />);
      expect(await screen.findByText("Settings unreadable")).toBeInTheDocument();
    });
  });
});
