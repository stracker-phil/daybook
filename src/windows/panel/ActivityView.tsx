import { useEffect, useRef, useState } from "react";
import { api, toAppError, type ActivitySession, type ActivitySuggestion, type ErrorKind, type TimerChanged, type TtState } from "../../api";
import { useApiResult } from "../../lib/useApiResult";
import { ActivityIcon, GROUPS, ProjectPicker, SuggestionList, TimerRow, keepFocus, timerChoices } from "./ActivityParts";
import { fuzzyFilter } from "./fuzzy";
import { useActiveScroll } from "./listKeys";
import { ESCAPE_HINT, PanelFooter, PanelInput, PanelPrompt, ProjectKeys, type PanelHint } from "./parts";
import { useActivityKeys } from "./useActivityKeys";
import { useCheckInTimeout } from "./useCheckInTimeout";

const MAX_VISIBLE = 8;
const TT_FAILURES: Partial<Record<ErrorKind, TtState>> = { timetracker_not_running: "notRunning", timetracker_missing: "missing" };

/** TimeTracker's state and its timer, kept current by `timer_changed`; the session's state until the first answer. */
function useTracking(initial: TtState) {
  const [tracking, setTracking] = useState<TimerChanged>({ tt: initial, timer: null });
  useEffect(() => {
    void api.timetrackerTimer().then(setTracking, () => {});
    const unlisten = api.onTimerChanged(setTracking);
    return () => void unlisten.then((fn) => fn()).catch(() => {});
  }, []);
  const setTt = (tt: TtState) => setTracking((current) => ({ ...current, tt }));
  return { ...tracking, setTt };
}

/** TimeTracker missing or not running, with a way to open it; else a failed suggestion load. */
function ActivityStatus({ tt, loadError, onOpen }: { tt: TtState; loadError: string | null; onOpen: () => void }) {
  if (tt === "missing") return <span className="palette-error">TimeTracker isn't installed (Settings → Connections → TimeTracker).</span>;
  if (tt === "available") return loadError ? <span className="palette-error">{loadError}</span> : null;
  return (
    <span className="activity-tt">
      <span className="palette-error">TimeTracker isn't running.</span>
      <button type="button" className="btn" onMouseDown={keepFocus} onClick={onOpen}>
        Open TimeTracker
      </button>
    </span>
  );
}

interface Pick {
  project: string;
  /** The timer row's choice, as picked; Rust resolves the project's links (`activity/pick.rs`). */
  ttProjectId: string | null;
}

/** Like an address bar: ↑↓ preview a suggestion in the fields; typing or picking makes the shown value your own. */
export function ActivityView({ session }: { session: ActivitySession }) {
  const checkIn = session.mode === "checkin";
  const names = session.projects.map((project) => project.name);
  const { tt, timer, setTt } = useTracking(session.tt);
  // Fetched again when TimeTracker comes or goes: its recent timers are among them.
  const suggestions = useApiResult(`activity\u0000${tt}`, api.activitySuggestions, { keepData: true });
  const [typed, setTyped] = useState(session.note);
  const [own, setOwn] = useState<Pick>({ project: session.project ?? "", ttProjectId: session.ttProjectId });
  // The suggestion your input came from; its tag goes along.
  const [origin, setOrigin] = useState<ActivitySuggestion | null>(null);
  const [edited, setEdited] = useState(false);
  const [highlighted, setHighlighted] = useState(-1);
  const [error, setError] = useState<string | null>(null);
  const input = useRef<HTMLInputElement>(null);

  // Until you type, the prefilled note doesn't hide the other suggestions.
  const visible = fuzzyFilter(suggestions.data ?? [], edited ? typed : "", (suggestion) => suggestion.display).slice(0, MAX_VISIBLE);
  // In the order they show, so ↑↓ follow the groups.
  const items = GROUPS.flatMap(({ group }) => visible.filter((suggestion) => suggestion.group === group));
  const current = highlighted < items.length ? highlighted : -1;
  const picked = current >= 0 ? items[current] : null;

  /** A suggestion's project and timer; one without a project keeps yours. */
  const pickOf = (suggestion: ActivitySuggestion | null): Pick =>
    suggestion?.project
      ? { project: suggestion.project, ttProjectId: suggestion.ttProjectId }
      : { project: own.project, ttProjectId: suggestion?.ttProjectId ?? own.ttProjectId };

  const shown = pickOf(picked);
  const note = picked?.note ?? typed;
  const shownProject = session.projects.find((project) => project.name === shown.project);
  const activeRef = useActiveScroll<HTMLLIElement>(current);
  // Display only; Rust decides what happens (`activity/pick.rs`).
  const sameNote = (other: string | null | undefined) => (other ?? "").trim() === note.trim();
  const sameTimer = !shownProject?.timerRow || shown.ttProjectId === (timer?.projectId ?? null);
  const unchanged = checkIn && shown.project === (session.project ?? "") && sameNote(session.note) && sameTimer;
  // Any change of the pick restarts the check-in's minute.
  useCheckInTimeout(checkIn, shown.project, shown.ttProjectId, note);

  // Prefilled and selected: ↵ confirms, typing replaces it.
  useEffect(() => input.current?.select(), []);

  // TimeTracker's absence shows as its state, with a way to open it; the typed note stays.
  const failed = (e: unknown) => {
    const failure = toAppError(e);
    const state = TT_FAILURES[failure.kind];
    if (state) setTt(state);
    setError(state ? null : failure.message);
  };

  const submit = (text: string, pick: Pick, from: ActivitySuggestion | null) => {
    // Without the timer row there is no choice to send.
    const timerRow = session.projects.some((project) => project.name === pick.project && project.timerRow);
    // A tag goes along only while project, timer choice and note are what its suggestion (or the prefill) set.
    const source = from ?? { project: session.project ?? "", ttProjectId: session.ttProjectId, note: session.note, tag: session.tag };
    const keepsTag =
      (source.project === null || source.project === pick.project) &&
      source.ttProjectId === pick.ttProjectId &&
      source.note.trim() === text.trim();
    api
      .activitySubmit(session.mode, { project: pick.project, ttProjectId: timerRow ? pick.ttProjectId : null, note: text, tag: keepsTag ? source.tag : null })
      .catch(failed);
  };

  /** What's shown becomes your own input. */
  const takeOver = () => {
    if (!picked) return;
    setTyped(picked.note);
    setOwn(shown);
    setOrigin(picked);
    setEdited(true);
    setHighlighted(-1);
  };

  // Another project starts its timer row on "None".
  const pickProject = (name: string) => {
    takeOver();
    setOwn(name === shown.project ? shown : { project: name, ttProjectId: null });
    setError(null);
  };

  const pickTimer = (id: string | null) => {
    takeOver();
    setOwn({ project: shown.project, ttProjectId: id });
    setError(null);
  };

  const openTimeTracker = () =>
    api.timetrackerFocus().then(
      () => {
        setTt("available");
        setError(null);
      },
      (e) => setError(toAppError(e).message),
    );

  const onKeyDown = useActivityKeys({
    items,
    current,
    projects: names,
    project: shown.project,
    timers: timerChoices(shownProject),
    highlight: setHighlighted,
    pickProject,
    pickTimer,
    submit: () => submit(note, shown, picked ?? origin),
  });

  const hints: PanelHint[] = [
    ...(names.length > 1 ? [{ keys: ["⇥"], label: "Next project" }] : []),
    { keys: ["↑", "↓"], label: "Suggestions" },
    { keys: ["↵"], label: unchanged ? "Still on it" : "Start" },
    ESCAPE_HINT,
  ];

  return (
    <div className="panel-card palette-view activity-view">
      <PanelPrompt>{session.prompt}</PanelPrompt>
      <PanelInput
        ref={input}
        icon={ActivityIcon}
        leading={<ProjectPicker projects={session.projects} picked={shown.project} onPick={pickProject} />}
        stageLabel="Activity note"
        aria-label="Note"
        placeholder={names.length > 0 ? "Note, e.g. QRS-6805 review" : "What are you working on?"}
        value={note}
        onChange={(e) => {
          if (picked) {
            setOwn(shown);
            setOrigin(picked);
          }
          setTyped(e.target.value);
          setEdited(true);
          setHighlighted(-1);
          setError(null);
        }}
        onKeyDown={onKeyDown}
      />
      {shownProject?.timerRow && <TimerRow project={shownProject} picked={shown.ttProjectId} onPick={pickTimer} />}
      {suggestions.data && suggestions.data.length > 0 && (
        <SuggestionList items={items} highlighted={current} activeRef={activeRef} onPick={(suggestion) => submit(suggestion.note, pickOf(suggestion), suggestion)} />
      )}
      <ProjectKeys names={names} />
      <PanelFooter
        hints={hints}
        error={error}
        status={<ActivityStatus tt={tt} loadError={suggestions.error?.message ?? null} onOpen={() => void openTimeTracker()} />}
      />
    </div>
  );
}
