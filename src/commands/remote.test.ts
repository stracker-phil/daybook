import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));

import { api } from "../api";
import { answerRemote } from "./remote";

const mocked = vi.mocked(api, true);

describe("answerRemote", () => {
  beforeEach(() => vi.clearAllMocks());

  it("replies with an error for an unknown request", async () => {
    await answerRemote({ id: "8", request: { type: "teleport" } } as never);
    expect(mocked.remoteReply).toHaveBeenCalledWith("8", null, expect.stringContaining("Unknown request"));
  });
});
