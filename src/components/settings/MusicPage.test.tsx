import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import { api, type ArtistSearch, type MusicEntry, type Settings } from "../../api";
import { settings } from "../../test/settings";
import { MusicPage } from "./MusicPage";

const mocked = vi.mocked(api, true);
const save = vi.fn<(patch: Partial<Settings>) => void>();

const entry = (patch: Partial<MusicEntry> = {}): MusicEntry => ({
  id: "a",
  name: "Focus",
  kind: "playlist",
  source: "Deep Focus",
  shuffle: true,
  artistId: "",
  storefront: "",
  ...patch,
});

function Harness({ initial }: { initial: Settings }) {
  const [s, setS] = useState(initial);
  return (
    <MusicPage
      s={s}
      set={(patch) => {
        save(patch);
        setS((current) => ({ ...current, ...patch }));
      }}
    />
  );
}

function renderPage(music: MusicEntry[]) {
  render(<Harness initial={settings({ music })} />);
}

const savedEntries = () => save.mock.lastCall![0].music!;
const savedEntry = (i = 0) => savedEntries()[i];
const search = (artists: ArtistSearch["artists"], storefront = "us"): ArtistSearch => ({ storefront, artists });
const deferred = <T,>() => {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
};

describe("MusicPage", () => {
  beforeEach(() => vi.clearAllMocks());

  describe("entry card", () => {
    it("saves a renamed entry", async () => {
      renderPage([entry()]);
      await userEvent.clear(screen.getByRole("textbox", { name: "Name" }));
      await userEvent.type(screen.getByRole("textbox", { name: "Name" }), "Deep Work{Enter}");
      expect(savedEntry()).toMatchObject({ name: "Deep Work", source: "Deep Focus" });
    });

    it("ignores an emptied name and shows the saved one again", async () => {
      renderPage([entry()]);
      await userEvent.clear(screen.getByRole("textbox", { name: "Name" }));
      await userEvent.tab();
      expect(save).not.toHaveBeenCalled();
      expect(screen.getByRole("textbox", { name: "Name" })).toHaveValue("Focus");
    });

    it("resets source, artist and storefront when the kind changes", async () => {
      renderPage([entry({ kind: "artist", source: "Muse", artistId: "7", storefront: "de" })]);
      await userEvent.selectOptions(screen.getByRole("combobox", { name: "Kind" }), "Link");
      expect(savedEntry()).toMatchObject({ kind: "link", source: "", artistId: "", storefront: "" });
      expect(screen.getByRole("textbox", { name: "Apple Music link" })).toHaveValue("");
    });

    it("offers the Shuffle switch for playlists only and saves it", async () => {
      renderPage([entry(), entry({ id: "b", kind: "link", source: "https://music.apple.com/x" })]);
      expect(screen.getAllByRole("switch", { name: "Shuffle" })).toHaveLength(1);
      await userEvent.click(screen.getByRole("switch", { name: "Shuffle" }));
      expect(savedEntry().shuffle).toBe(false);
    });

    it("saves a link entry's Apple Music link", async () => {
      renderPage([entry({ kind: "link", source: "" })]);
      await userEvent.type(screen.getByRole("textbox", { name: "Apple Music link" }), "https://music.apple.com/pl/1{Enter}");
      expect(savedEntry().source).toBe("https://music.apple.com/pl/1");
    });
  });

  describe("list actions", () => {
    it("adds a New entry playlist", async () => {
      renderPage([entry()]);
      await userEvent.click(screen.getByRole("button", { name: "Add Entry" }));
      expect(savedEntry(1)).toMatchObject({ name: "New entry", kind: "playlist", source: "" });
      expect(screen.getAllByRole("textbox", { name: "Name" })).toHaveLength(2);
    });

    it("moves an entry down", async () => {
      renderPage([entry({ id: "a", name: "A" }), entry({ id: "b", name: "B" })]);
      await userEvent.click(screen.getAllByRole("button", { name: "Move Down" })[0]);
      expect(savedEntries().map((e) => e.name)).toEqual(["B", "A"]);
    });

    it("moves an entry up", async () => {
      renderPage([entry({ id: "a", name: "A" }), entry({ id: "b", name: "B" })]);
      await userEvent.click(screen.getAllByRole("button", { name: "Move Up" })[1]);
      expect(savedEntries().map((e) => e.name)).toEqual(["B", "A"]);
    });

    it("removes an entry", async () => {
      renderPage([entry({ id: "a", name: "A" }), entry({ id: "b", name: "B" })]);
      await userEvent.click(screen.getAllByRole("button", { name: "Remove Entry" })[0]);
      expect(savedEntries().map((e) => e.name)).toEqual(["B"]);
    });
  });

  describe("playlists", () => {
    it("starts with a text field and saves the typed playlist name", async () => {
      renderPage([entry({ source: "" })]);
      await userEvent.type(screen.getByRole("textbox", { name: "Playlist" }), "Chill{Enter}");
      expect(savedEntry().source).toBe("Chill");
    });

    it("shows Asking Music… while loading, then offers a select and Reload My Playlists", async () => {
      const pending = deferred<string[]>();
      mocked.musicPlaylists.mockReturnValue(pending.promise);
      renderPage([entry({ source: "B" })]);
      await userEvent.click(screen.getByRole("button", { name: "Load My Playlists" }));
      expect(screen.getByRole("button", { name: "Asking Music…" })).toBeDisabled();
      pending.resolve(["A", "B"]);
      expect(await screen.findByRole("button", { name: "Reload My Playlists" })).toBeEnabled();
      const select = screen.getByRole("combobox", { name: "Playlist" });
      expect(within(select).getAllByRole("option").map((o) => o.textContent)).toEqual(["A", "B"]);
      expect(select).toHaveValue("B");
    });

    it("saves the playlist picked from the library", async () => {
      mocked.musicPlaylists.mockResolvedValue(["A", "B"]);
      renderPage([entry({ source: "A" })]);
      await userEvent.click(screen.getByRole("button", { name: "Load My Playlists" }));
      await userEvent.selectOptions(await screen.findByRole("combobox", { name: "Playlist" }), "B");
      expect(savedEntry().source).toBe("B");
    });

    it("keeps a current playlist missing from the library at the top of the select", async () => {
      mocked.musicPlaylists.mockResolvedValue(["A", "B"]);
      renderPage([entry({ source: "Gone" })]);
      await userEvent.click(screen.getByRole("button", { name: "Load My Playlists" }));
      const select = await screen.findByRole("combobox", { name: "Playlist" });
      expect(within(select).getAllByRole("option").map((o) => o.textContent)).toEqual(["Gone", "A", "B"]);
      expect(select).toHaveValue("Gone");
    });

    it("asks to choose a playlist for an entry without one", async () => {
      mocked.musicPlaylists.mockResolvedValue(["A"]);
      renderPage([entry({ source: "" })]);
      await userEvent.click(screen.getByRole("button", { name: "Load My Playlists" }));
      expect(await screen.findByRole("option", { name: "Choose a playlist" })).toBeDisabled();
    });

    it("shows the error of a failed load inline and keeps the text field", async () => {
      mocked.musicPlaylists.mockRejectedValue({ kind: "io", message: "Music is not responding", code: null });
      renderPage([entry()]);
      await userEvent.click(screen.getByRole("button", { name: "Load My Playlists" }));
      expect(await screen.findByRole("alert")).toHaveTextContent("Music is not responding");
      expect(screen.getByRole("textbox", { name: "Playlist" })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Load My Playlists" })).toBeEnabled();
    });
  });

  describe("artist radio", () => {
    const artistEntry = (patch: Partial<MusicEntry> = {}) => entry({ kind: "artist", source: "", ...patch });
    const typeArtist = (text: string) => userEvent.type(screen.getByRole("textbox", { name: "Artist" }), `${text}{Enter}`);

    it("asks for a name while no artist is picked", () => {
      renderPage([artistEntry()]);
      expect(screen.getByText("Type a name, then ↵")).toBeInTheDocument();
    });

    it("shows Searching… and takes the first match with its storefront", async () => {
      const pending = deferred<ArtistSearch>();
      mocked.musicFindArtists.mockReturnValue(pending.promise);
      renderPage([artistEntry()]);
      await typeArtist("mus");
      expect(mocked.musicFindArtists).toHaveBeenCalledWith("mus");
      expect(screen.getByText("Searching…")).toBeInTheDocument();
      pending.resolve(search([{ id: "7", name: "Muse", genre: "Rock" }], "de"));
      await vi.waitFor(() => expect(savedEntry()).toMatchObject({ source: "Muse", artistId: "7", storefront: "de" }));
      expect(screen.queryByText("Searching…")).not.toBeInTheDocument();
      expect(screen.queryByText("Type a name, then ↵")).not.toBeInTheDocument();
    });

    it("does not search for an emptied name", async () => {
      renderPage([artistEntry({ source: "Muse", artistId: "7" })]);
      await userEvent.clear(screen.getByRole("textbox", { name: "Artist" }));
      await userEvent.tab();
      expect(mocked.musicFindArtists).not.toHaveBeenCalled();
      expect(save).not.toHaveBeenCalled();
    });

    it("says No artist found and stores the typed name without an artist id", async () => {
      mocked.musicFindArtists.mockResolvedValue(search([]));
      renderPage([artistEntry({ artistId: "old" })]);
      await typeArtist("Nobody");
      expect(await screen.findByText("No artist found")).toBeInTheDocument();
      expect(savedEntry()).toMatchObject({ source: "Nobody", artistId: "" });
    });

    it("shows the message of a failed search", async () => {
      mocked.musicFindArtists.mockRejectedValue({ kind: "network", message: "Offline", code: null });
      renderPage([artistEntry()]);
      await typeArtist("Muse");
      expect(await screen.findByText("Offline")).toBeInTheDocument();
      expect(save).not.toHaveBeenCalled();
    });

    it("lists several matches as name · genre and saves the picked artist", async () => {
      mocked.musicFindArtists.mockResolvedValue(
        search(
          [
            { id: "1", name: "Muse", genre: "Rock" },
            { id: "2", name: "Muse", genre: "Jazz" },
            { id: "3", name: "Muse Tribute", genre: "" },
          ],
          "fr",
        ),
      );
      renderPage([artistEntry()]);
      await typeArtist("Muse");
      const select = await screen.findByRole("combobox", { name: "Which artist" });
      expect(within(select).getAllByRole("option").map((o) => o.textContent)).toEqual(["Muse · Rock", "Muse · Jazz", "Muse Tribute"]);
      await userEvent.selectOptions(select, "Muse · Jazz");
      expect(savedEntry()).toMatchObject({ source: "Muse", artistId: "2", storefront: "fr" });
    });

    it("offers no artist choice for a single match", async () => {
      mocked.musicFindArtists.mockResolvedValue(search([{ id: "1", name: "Muse", genre: "Rock" }]));
      renderPage([artistEntry()]);
      await typeArtist("Muse");
      await vi.waitFor(() => expect(savedEntry().artistId).toBe("1"));
      expect(screen.queryByRole("combobox", { name: "Which artist" })).not.toBeInTheDocument();
    });
  });
});
