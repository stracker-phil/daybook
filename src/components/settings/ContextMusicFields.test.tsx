import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../../test/apiMock")).createApiMock() }));

import type { Context, ContextMusic, MusicEntry } from "../../api";
import { settings } from "../../test/settings";
import { ContextMusicFields } from "./ContextMusicFields";

const change = vi.fn<(patch: Partial<Context>) => void>();

const entry = (patch: Partial<MusicEntry> = {}): MusicEntry => ({
  id: "m1",
  name: "Focus",
  kind: "playlist",
  source: "Deep Focus",
  shuffle: true,
  artistId: "",
  storefront: "",
  ...patch,
});

const [work, priv] = settings().contexts;

function Harness({ initial, entries }: { initial: Context; entries: MusicEntry[] }) {
  const [context, setContext] = useState(initial);
  return (
    <ContextMusicFields
      context={context}
      entries={entries}
      onChange={(patch) => {
        change(patch);
        setContext((current) => ({ ...current, ...patch }));
      }}
    />
  );
}

function renderFields(context: Context, music: Partial<ContextMusic> = {}, entries: MusicEntry[] = [entry()]) {
  render(<Harness initial={{ ...context, music: { ...context.music, ...music } }} entries={entries} />);
}

const savedMusic = () => change.mock.lastCall![0].music!;
const volume = () => screen.getByRole("textbox", { name: "Volume during a call in percent" });

describe("ContextMusicFields", () => {
  beforeEach(() => vi.clearAllMocks());

  it("says Not set up and offers nothing else without music entries", () => {
    renderFields(work, {}, []);
    expect(screen.getByText("Music")).toBeInTheDocument();
    expect(screen.getByText("Not set up")).toBeInTheDocument();
    expect(screen.queryByRole("combobox", { name: "Music during a call" })).not.toBeInTheDocument();
  });

  describe("during a call", () => {
    it("offers Keep playing, Pause and Lower the volume", () => {
      renderFields(work);
      const select = screen.getByRole("combobox", { name: "Music during a call" });
      expect(within(select).getAllByRole("option").map((o) => o.textContent)).toEqual(["Keep playing", "Pause", "Lower the volume"]);
      expect(select).toHaveValue("lower");
    });

    it.each([
      ["Keep playing", "nothing"],
      ["Pause", "pause"],
    ])("saves %s and hides the volume", async (label, value) => {
      renderFields(work);
      await userEvent.selectOptions(screen.getByRole("combobox", { name: "Music during a call" }), label);
      expect(savedMusic()).toMatchObject({ onCall: value, resume: true });
      expect(screen.queryByRole("textbox", { name: "Volume during a call in percent" })).not.toBeInTheDocument();
    });

    it("shows the volume for Lower the volume", () => {
      renderFields(work, { onCall: "lower", callVolume: 2 });
      expect(volume()).toHaveValue("2");
    });

    it("saves a volume from 0 to 100, rounded", async () => {
      renderFields(work);
      await userEvent.clear(volume());
      await userEvent.type(volume(), "40.6{Enter}");
      expect(savedMusic().callVolume).toBe(41);
      await userEvent.clear(volume());
      await userEvent.type(volume(), "0{Enter}");
      expect(savedMusic().callVolume).toBe(0);
    });

    it.each(["101", "-1", "loud", ""])("ignores the volume %j", async (text) => {
      renderFields(work);
      await userEvent.clear(volume());
      if (text) await userEvent.type(volume(), text);
      await userEvent.tab();
      expect(change).not.toHaveBeenCalled();
    });
  });

  describe("with the status feature", () => {
    it("saves the pause on Break and Done for today", async () => {
      renderFields(work);
      const toggle = screen.getByRole("switch", { name: "Pause music on Break and Done for today" });
      expect(toggle).toBeChecked();
      await userEvent.click(toggle);
      expect(savedMusic().pauseWhenAway).toBe(false);
    });

    it("lists Nothing and the entries with an id for starting with the workday", () => {
      renderFields(work, {}, [entry(), entry({ id: "", name: "Unsaved" }), entry({ id: "m2", name: "", source: "Muse" })]);
      const select = screen.getByRole("combobox", { name: "Start music with the workday" });
      expect(within(select).getAllByRole("option").map((o) => o.textContent)).toEqual(["Nothing", "Focus", "Muse"]);
    });

    it("saves the entry to start with the workday and Nothing again", async () => {
      renderFields(work);
      const select = screen.getByRole("combobox", { name: "Start music with the workday" });
      await userEvent.selectOptions(select, "Focus");
      expect(savedMusic().startWith).toBe("m1");
      await userEvent.selectOptions(select, "Nothing");
      expect(savedMusic().startWith).toBe("");
    });
  });

  describe("without the status feature", () => {
    it("hides the Break and workday settings but keeps the call and resume ones", () => {
      renderFields(priv);
      expect(screen.queryByRole("switch", { name: "Pause music on Break and Done for today" })).not.toBeInTheDocument();
      expect(screen.queryByRole("combobox", { name: "Start music with the workday" })).not.toBeInTheDocument();
      expect(screen.getByRole("combobox", { name: "Music during a call" })).toBeInTheDocument();
      expect(screen.getByRole("switch", { name: "Resume music when back" })).toBeInTheDocument();
    });
  });

  it("saves the resume switch", async () => {
    renderFields(priv);
    await userEvent.click(screen.getByRole("switch", { name: "Resume music when back" }));
    expect(savedMusic().resume).toBe(false);
  });
});
