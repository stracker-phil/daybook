import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../../test/apiMock")).createApiMock() }));

import { api } from "../../../api";
import { searchResponse, searchResult } from "../../../test/search";
import { SearchView } from "./SearchView";

const mocked = vi.mocked(api, true);

const preview = () => screen.findByRole("region", { name: "Preview" });

/** Renders the overlay with one result whose note has `markdown`. */
async function previewOf(markdown: string, path = "Home.md") {
  mocked.searchQuery.mockResolvedValue(searchResponse([searchResult(path)]));
  mocked.searchNote.mockImplementation(async (p: string) => ({ path: p, name: p, folder: "", markdown: p === path ? markdown : `Body of ${p}` }));
  render(<SearchView scope={null} />);
  return within(await preview());
}

describe("NotePreview in the search overlay", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.searchQuery.mockResolvedValue(searchResponse([]));
    mocked.searchScopes.mockResolvedValue({ names: ["all"], problems: [] });
    mocked.searchAsset.mockResolvedValue("data:image/png;base64,AAAA");
    mocked.searchOpenUrl.mockResolvedValue(undefined);
  });

  it("renders the note's markdown including tables and task lists", async () => {
    const view = await previewOf("# Title\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n- [x] done\n- [ ] todo");
    expect(await view.findByRole("heading", { name: "Title" })).toBeInTheDocument();
    expect(view.getByRole("table")).toBeInTheDocument();
    expect(view.getByRole("cell", { name: "2" })).toBeInTheDocument();
    const boxes = view.getAllByRole("checkbox");
    expect(boxes.map((box) => (box as HTMLInputElement).checked)).toEqual([true, false]);
  });

  it("loads the linked note when a #note= link is clicked", async () => {
    const view = await previewOf("[Plan](#note=2-Work%2FPlan.md)");
    await userEvent.click(await view.findByRole("link", { name: "Plan" }));
    expect(mocked.searchNote).toHaveBeenCalledWith("2-Work/Plan.md");
    expect(await view.findByText("Body of 2-Work/Plan.md")).toBeInTheDocument();
  });

  it("renders an unresolved #missing= link as plain text", async () => {
    const view = await previewOf("See [Ghost](#missing=Ghost) here");
    expect(await view.findByText("Ghost")).toBeInTheDocument();
    expect(view.queryByRole("link")).not.toBeInTheDocument();
  });

  it("opens other links through the backend", async () => {
    const view = await previewOf("[Docs](https://example.com/docs)");
    await userEvent.click(await view.findByRole("link", { name: "Docs" }));
    expect(mocked.searchOpenUrl).toHaveBeenCalledWith("https://example.com/docs");
  });

  it("opens obsidian: links through the backend", async () => {
    const view = await previewOf("[Vault](obsidian://open?vault=x)");
    await userEvent.click(await view.findByRole("link", { name: "Vault" }));
    expect(mocked.searchOpenUrl).toHaveBeenCalledWith("obsidian://open?vault=x");
  });

  it("shows the footer error when opening a link fails", async () => {
    mocked.searchOpenUrl.mockRejectedValue({ kind: "io", message: "No browser", code: null });
    const view = await previewOf("[Docs](https://example.com/docs)");
    await userEvent.click(await view.findByRole("link", { name: "Docs" }));
    expect(await screen.findByText("No browser")).toBeInTheDocument();
  });

  describe("images", () => {
    it("renders a vault image from the decoded #asset= path", async () => {
      const view = await previewOf("![Diagram](#asset=img%2Fmy%20diagram.png)");
      const image = await view.findByRole("img", { name: "Diagram" });
      expect(image).toHaveAttribute("src", "data:image/png;base64,AAAA");
      expect(mocked.searchAsset).toHaveBeenCalledWith("img/my diagram.png");
    });

    it("renders nothing for a vault image that fails to load", async () => {
      mocked.searchAsset.mockRejectedValue({ kind: "notFound", message: "Missing", code: null });
      const view = await previewOf("Loaded\n\n![Broken](#asset=img%2Fbroken.png)");
      await view.findByText("Loaded");
      await vi.waitFor(() => expect(mocked.searchAsset).toHaveBeenCalledWith("img/broken.png"));
      expect(view.queryByRole("img")).not.toBeInTheDocument();
    });

    it("renders an https image directly", async () => {
      const view = await previewOf("![Remote](https://example.com/a.png)");
      expect(await view.findByRole("img", { name: "Remote" })).toHaveAttribute("src", "https://example.com/a.png");
      expect(mocked.searchAsset).not.toHaveBeenCalled();
    });

    it("renders nothing for any other image source", async () => {
      const view = await previewOf("Loaded\n\n![Plain](http://example.com/a.png)\n\n![Local](file:///etc/hosts)");
      await view.findByText("Loaded");
      expect(view.queryByRole("img")).not.toBeInTheDocument();
    });
  });

  it("keeps the previous note visible while the next one loads", async () => {
    mocked.searchQuery.mockResolvedValue(searchResponse([searchResult("A.md"), searchResult("B.md")]));
    let finishB: (markdown: string) => void = () => {};
    mocked.searchNote.mockImplementation((path: string) =>
      path === "A.md"
        ? Promise.resolve({ path, name: path, folder: "", markdown: "Body of A" })
        : new Promise((resolve) => {
            finishB = (markdown) => resolve({ path, name: path, folder: "", markdown });
          }),
    );
    render(<SearchView scope={null} />);
    const view = within(await preview());
    await view.findByText("Body of A");
    await userEvent.keyboard("{ArrowDown}");
    expect(view.getByText("Body of A")).toBeInTheDocument();
    finishB("Body of B");
    expect(await view.findByText("Body of B")).toBeInTheDocument();
    expect(view.queryByText("Body of A")).not.toBeInTheDocument();
  });
});
