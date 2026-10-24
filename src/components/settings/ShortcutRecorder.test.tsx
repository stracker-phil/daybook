import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api } from "../../api";
import { ShortcutRecorder } from "./ShortcutRecorder";

const mocked = vi.mocked(api, true);

function renderRecorder(value: string | null = "a", parse: (e: unknown) => string | null = () => "b") {
  const onChange = vi.fn();
  render(<ShortcutRecorder name="Thing" value={value} parse={parse} label={(s) => `<${s}>`} onChange={onChange} />);
  return { onChange, field: () => screen.getByRole("button", { name: /shortcut for Thing/ }) };
}

describe("ShortcutRecorder", () => {
  beforeEach(() => vi.clearAllMocks());

  it("shows the shortcut as labelled, or unbound", () => {
    renderRecorder("a");
    expect(screen.getByRole("button", { name: "Record shortcut for Thing" })).toHaveTextContent("<a>");
  });

  it("shows unbound without a shortcut", () => {
    renderRecorder(null);
    expect(screen.getByRole("button", { name: "Record shortcut for Thing" })).toHaveTextContent("unbound");
  });

  it("reports the parsed key and stops recording", async () => {
    const { onChange, field } = renderRecorder();
    await userEvent.click(field());
    expect(field()).toHaveTextContent("Press keys…");
    await userEvent.keyboard("x");
    expect(onChange).toHaveBeenCalledWith("b");
    expect(field()).toHaveTextContent("<a>");
  });

  it("keeps listening when the key press is no shortcut", async () => {
    const { onChange, field } = renderRecorder("a", () => null);
    await userEvent.click(field());
    await userEvent.keyboard("x");
    expect(onChange).not.toHaveBeenCalled();
    expect(field()).toHaveTextContent("Press keys…");
  });

  it("reports null on Backspace and on Delete", async () => {
    const { onChange, field } = renderRecorder();
    await userEvent.click(field());
    await userEvent.keyboard("{Backspace}");
    await userEvent.click(field());
    await userEvent.keyboard("{Delete}");
    expect(onChange.mock.calls).toEqual([[null], [null]]);
  });

  it("cancels on Escape and when focus leaves", async () => {
    const { onChange, field } = renderRecorder();
    await userEvent.click(field());
    await userEvent.keyboard("{Escape}");
    expect(field()).toHaveTextContent("<a>");
    await userEvent.click(field());
    await userEvent.click(document.body);
    expect(field()).toHaveTextContent("<a>");
    expect(onChange).not.toHaveBeenCalled();
  });

  it("pauses the global shortcuts while recording and resumes afterwards", async () => {
    const { field } = renderRecorder();
    await userEvent.click(field());
    expect(mocked.shortcutsPause).toHaveBeenLastCalledWith(true);
    await userEvent.keyboard("{Escape}");
    expect(mocked.shortcutsPause).toHaveBeenLastCalledWith(false);
  });
});
