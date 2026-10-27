import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import type { Jira, Settings } from "../../api";
import { settings } from "../../test/settings";
import { JiraGroup } from "./JiraGroup";

const set = vi.fn<(patch: Partial<Settings>) => void>();

function renderGroup(jira: Partial<Jira> = {}) {
  render(<JiraGroup s={settings({ jira: { baseUrl: "", defaultPrefix: "", ...jira } })} set={set} />);
}

const saved = () => set.mock.lastCall![0].jira!;

describe("JiraGroup", () => {
  beforeEach(() => vi.clearAllMocks());

  it("is titled Jira", () => {
    renderGroup();
    expect(screen.getByRole("heading", { name: "Jira" })).toBeInTheDocument();
  });

  describe("Issue URL description", () => {
    it("explains the field without a URL", () => {
      renderGroup();
      expect(screen.getByText("The part of an issue link before the key.")).toBeInTheDocument();
    });

    it("shows where issue 6805 opens, with QRS when no project is set", () => {
      renderGroup({ baseUrl: "https://x.atlassian.net/browse/" });
      expect(screen.getByText("Issue 6805 opens https://x.atlassian.net/browse/QRS-6805")).toBeInTheDocument();
    });

    it("adds the missing slash and uses the default project", () => {
      renderGroup({ baseUrl: "https://x.atlassian.net/browse", defaultPrefix: "ABC" });
      expect(screen.getByText("Issue 6805 opens https://x.atlassian.net/browse/ABC-6805")).toBeInTheDocument();
    });
  });

  describe("saving", () => {
    it("saves the issue URL trimmed and keeps the default project", async () => {
      renderGroup({ defaultPrefix: "ABC" });
      await userEvent.type(screen.getByRole("textbox", { name: "Issue URL" }), "  https://x.atlassian.net/browse/  {Enter}");
      expect(saved()).toEqual({ baseUrl: "https://x.atlassian.net/browse/", defaultPrefix: "ABC" });
    });

    it("saves the default project trimmed and keeps the issue URL", async () => {
      renderGroup({ baseUrl: "https://x/browse/" });
      await userEvent.type(screen.getByRole("textbox", { name: "Default project" }), " QRS {Enter}");
      expect(saved()).toEqual({ baseUrl: "https://x/browse/", defaultPrefix: "QRS" });
    });
  });
});
