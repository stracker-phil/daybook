import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type MusicState } from "../../api";
import { settings } from "../../test/settings";
import { MusicView } from "./MusicView";

const mocked = vi.mocked(api, true);

const state = (patch: Partial<MusicState> = {}): MusicState => ({
  entries: [
    { id: "a", name: "Focus" },
    { id: "b", name: "Deep Work" },
    { id: "c", name: "Jazz Focus" },
  ],
  player: "closed",
  track: null,
  volume: null,
  entry: null,
  ...patch,
});

async function renderView(patch: Partial<MusicState> = {}) {
  mocked.musicState.mockResolvedValue(state(patch));
  render(<MusicView />);
  await screen.findAllByRole("option");
}

const input = () => screen.getByRole("textbox", { name: "Entry" });
const names = () => screen.queryAllByRole("option").map((o) => o.querySelector(".grow-text")?.textContent);
const selected = () => screen.getAllByRole("option").find((o) => o.getAttribute("aria-selected") === "true")?.textContent;
const error = (message: string) => ({ kind: "io", message, code: null });

describe("MusicView", () => {
  beforeEach(() => vi.clearAllMocks());

  it("lists the entries numbered with the footer hints", async () => {
    await renderView();
    expect(screen.getAllByRole("option").map((o) => o.textContent)).toEqual(["⌘1Focus", "⌘2Deep Work", "⌘3Jazz Focus"]);
    expect(screen.getByText("↑↓ entry · ↵ play · ⌘1–9 play · ⌘P play · esc close")).toBeInTheDocument();
  });

  it("numbers only the first nine rows", async () => {
    const entries = Array.from({ length: 10 }, (_, i) => ({ id: `e${i}`, name: `Focus${i + 1}` }));
    await renderView({ entries });
    const options = screen.getAllByRole("option");
    expect(options[8]).toHaveTextContent("⌘9Focus9");
    expect(options[9]).toHaveTextContent(/^Focus10$/);
  });

  describe("state row", () => {
    it.each([
      [{ player: "playing", track: "Song" }, "Playing · Song"],
      [{ player: "paused", track: "Song" }, "Paused · Song"],
      [{ player: "playing", track: null, entry: "b" }, "Playing · Deep Work"],
      [{ player: "paused", track: null, entry: "b" }, "Paused · Deep Work"],
      [{ player: "playing", track: null, entry: null }, "Playing"],
      [{ player: "paused", track: null, entry: null }, "Paused"],
      [{ player: "closed" }, "Nothing is playing"],
    ] as [Partial<MusicState>, string][])("shows %o as %s", async (patch, line) => {
      await renderView(patch);
      expect(screen.getByText(line)).toBeInTheDocument();
    });

    it("shows the volume bar only when the volume is known", async () => {
      await renderView({ volume: 40 });
      expect(screen.getByLabelText("Volume 40 %")).toBeInTheDocument();
    });

    it("has no volume bar when the volume is unknown", async () => {
      await renderView({ volume: null });
      expect(screen.queryByLabelText(/Volume/)).not.toBeInTheDocument();
    });

    it("marks the current entry with a pill", async () => {
      await renderView({ player: "playing", entry: "b" });
      expect(screen.getByRole("option", { name: /Deep Work/ })).toHaveTextContent("Current");
      expect(screen.getAllByText("Current")).toHaveLength(1);
    });
  });

  describe("hints", () => {
    it.each([
      ["playing", "↑↓ entry · ↵ play · ⌘1–9 play · ⌘P pause · ⌘↑↓ volume · esc close"],
      ["paused", "↑↓ entry · ↵ play · ⌘1–9 play · ⌘P resume · ⌘↑↓ volume · esc close"],
    ] as const)("offer the volume and %s toggle when the volume is known", async (player, hints) => {
      await renderView({ player, volume: 50 });
      expect(screen.getByText(hints)).toBeInTheDocument();
    });

    it("drop the entry hints when no row shows", async () => {
      await renderView();
      await userEvent.type(input(), "zzz");
      expect(screen.getByText("⌘P play · esc close")).toBeInTheDocument();
    });
  });

  describe("filtering", () => {
    it("keeps the entries whose name has every typed word and selects the first again", async () => {
      await renderView();
      await userEvent.keyboard("{ArrowDown}");
      await userEvent.type(input(), "focus jazz");
      expect(names()).toEqual(["Jazz Focus"]);
      expect(selected()).toBe("⌘1Jazz Focus");
    });

    it("says so when nothing matches", async () => {
      await renderView();
      await userEvent.type(input(), "zzz");
      expect(screen.getByText("No matching entry.")).toBeInTheDocument();
    });
  });

  it("explains where to add music when there are no entries", async () => {
    mocked.musicState.mockResolvedValue(state({ entries: [] }));
    render(<MusicView />);
    expect(await screen.findByText("No music yet. Add playlists in Settings → Music.")).toBeInTheDocument();
  });

  describe("selection", () => {
    it("wraps with the arrow keys", async () => {
      await renderView();
      await userEvent.keyboard("{ArrowUp}");
      expect(selected()).toBe("⌘3Jazz Focus");
      await userEvent.keyboard("{ArrowDown}");
      expect(selected()).toBe("⌘1Focus");
    });

    it("selects an unselected row on click and plays it on the next click", async () => {
      mocked.musicPlay.mockResolvedValue(state());
      await renderView();
      await userEvent.click(screen.getByRole("option", { name: /Deep Work/ }));
      expect(selected()).toBe("⌘2Deep Work");
      expect(mocked.musicPlay).not.toHaveBeenCalled();
      await userEvent.click(screen.getByRole("option", { name: /Deep Work/ }));
      expect(mocked.musicPlay).toHaveBeenCalledWith("b");
    });
  });

  describe("playing", () => {
    it("plays the selected entry with Enter and closes the overlay", async () => {
      mocked.musicPlay.mockResolvedValue(state());
      await renderView();
      await userEvent.keyboard("{ArrowDown}{Enter}");
      expect(mocked.musicPlay).toHaveBeenCalledWith("b");
      await vi.waitFor(() => expect(mocked.panelClose).toHaveBeenCalled());
    });

    it("plays the numbered row with ⌘ plus a digit", async () => {
      await renderView();
      await userEvent.keyboard("{Meta>}3{/Meta}");
      expect(mocked.musicPlay).toHaveBeenCalledWith("c");
    });

    it("does nothing for a number past the last row", async () => {
      await renderView();
      await userEvent.keyboard("{Meta>}9{/Meta}");
      expect(mocked.musicPlay).not.toHaveBeenCalled();
    });

    it("shows Starting meanwhile and ignores a second play", async () => {
      mocked.musicPlay.mockReturnValue(new Promise(() => {}));
      await renderView({ player: "playing", track: "Song" });
      await userEvent.keyboard("{Enter}{ArrowDown}{Enter}");
      expect(screen.getByText("Starting Focus…")).toBeInTheDocument();
      expect(mocked.musicPlay).not.toHaveBeenCalledWith("b");
    });

    it("shows the error of a failed play, clears Starting and does not close", async () => {
      mocked.musicPlay.mockRejectedValue(error("Music is not installed"));
      await renderView();
      await userEvent.keyboard("{Enter}");
      expect(await screen.findByText("Music is not installed")).toBeInTheDocument();
      expect(screen.queryByText(/Starting/)).not.toBeInTheDocument();
      expect(mocked.panelClose).not.toHaveBeenCalled();
    });
  });

  describe("⌘P", () => {
    it("toggles playback, shows the answer and stays open", async () => {
      mocked.musicToggle.mockResolvedValue(state({ player: "paused", track: "Song" }));
      await renderView({ player: "playing", track: "Song" });
      await userEvent.keyboard("{Meta>}p{/Meta}");
      expect(await screen.findByText("Paused · Song")).toBeInTheDocument();
      expect(mocked.panelClose).not.toHaveBeenCalled();
    });
  });

  describe("volume", () => {
    it.each([
      ["{ArrowUp}", 50, 60],
      ["{ArrowDown}", 50, 40],
      ["{ArrowUp}", 95, 100],
      ["{ArrowDown}", 5, 0],
    ])("%s from %i sets %i", async (key, from, to) => {
      mocked.musicVolume.mockResolvedValue(state({ volume: to }));
      await renderView({ volume: from });
      await userEvent.keyboard(`{Meta>}${key}{/Meta}`);
      expect(mocked.musicVolume).toHaveBeenCalledWith(to);
      expect(await screen.findByLabelText(`Volume ${to} %`)).toBeInTheDocument();
    });

    it("does nothing when the volume is unknown", async () => {
      await renderView({ volume: null });
      await userEvent.keyboard("{Meta>}{ArrowUp}{/Meta}");
      expect(mocked.musicVolume).not.toHaveBeenCalled();
    });
  });

  describe("errors", () => {
    it("shows a failed action in the footer instead of the hints and clears it on the next success", async () => {
      mocked.musicToggle.mockRejectedValueOnce(error("Music does not answer")).mockResolvedValue(state({ player: "playing" }));
      await renderView();
      await userEvent.keyboard("{Meta>}p{/Meta}");
      expect(await screen.findByText("Music does not answer")).toBeInTheDocument();
      expect(screen.queryByText(/esc close/)).not.toBeInTheDocument();
      await userEvent.keyboard("{Meta>}p{/Meta}");
      expect(await screen.findByText(/⌘P pause/)).toBeInTheDocument();
      expect(screen.queryByText("Music does not answer")).not.toBeInTheDocument();
    });

    it("shows the error of a failed load and lists the saved entries from the settings", async () => {
      mocked.musicState.mockRejectedValue(error("Music is not running"));
      mocked.getSettings.mockResolvedValue(
        settings({
          music: [
            { id: "s1", name: "Saved", kind: "playlist", source: "src", artistId: "", storefront: "", shuffle: false },
            { id: "s2", name: "", kind: "link", source: "https://example.com/radio", artistId: "", storefront: "", shuffle: false },
          ],
        }),
      );
      render(<MusicView />);
      expect(await screen.findByText("Music is not running")).toBeInTheDocument();
      expect(await screen.findByRole("option", { name: /Saved/ })).toBeInTheDocument();
      expect(screen.getAllByRole("option")).toHaveLength(2);
    });
  });

  it("updates the view from a music_changed payload", async () => {
    await renderView();
    const handler = mocked.onMusicChanged.mock.calls[0][0];
    act(() => handler(state({ player: "playing", track: "Fresh", volume: 70, entry: "a" })));
    expect(screen.getByText("Playing · Fresh")).toBeInTheDocument();
    expect(screen.getByLabelText("Volume 70 %")).toBeInTheDocument();
    expect(screen.getAllByRole("option")[0]).toHaveTextContent("Current");
  });
});
