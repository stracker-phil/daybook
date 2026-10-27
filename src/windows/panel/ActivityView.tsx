import { useEffect, useRef, useState } from "react";
import { Timer } from "lucide-react";
import { api, toAppError, type ActivitySession, type ActivitySuggestion, type ErrorKind, type TimerChanged, type TtState } from "../../api";
import { useApiResult } from "../../lib/useApiResult";
import { GROUPS, ProjectKeys, ProjectPicker, SuggestionList, keepFocus, pickable } from "./ActivityParts";
import { fuzzyFilter } from "./fuzzy";
import { useActiveScroll } from "./listKeys";
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

function TtStatus({ tt, onOpen }: { tt: TtState; onOpen: () => void }) {
  if (tt === "missing") return <span className="palette-error">TimeTracker isn't installed (Settings → Connections → TimeTracker).</span>;
  return (
    <span className="activity-tt">
      <span className="palette-error">TimeTracker isn't running.</span>
      <button type="button" className="btn" onMouseDown={keepFocus} onClick={onOpen}>
        Open TimeTracker
      </button>
    </span>
  );
}

/** The footer's status: an error, else TimeTracker missing or not running, else a failed suggestion load. */
function ActivityStatus({ error, tt, loadError, onOpen }: { error: string | null; tt: TtState; loadError: string | null; onOpen: () => void }) {
  if (error) return <span className="palette-error">{error}</span>;
  if (tt !== "available") return <TtStatus tt={tt} onOpen={onOpen} />;
  return loadError ? <span className="palette-error">{loadError}</span> : null;
}

/** Like an address bar: ↑↓ preview a suggestion in the field; typing or picking a project makes the shown value your own. */
export function ActivityView({ session }: { session: ActivitySession }) {
  const checkIn = session.mode === "checkin";
  const choices = pickable(session.projects);
  const { tt, timer, setTt } = useTracking(session.tt);
  // Fetched again when TimeTracker comes or goes: its recent timers are among them.
  const suggestions = useApiResult(`activity\u0000${tt}`, api.activitySuggestions, { keepData: true });
  const [typed, setTyped] = useState(session.note);
  const [ownTt, setOwnTt] = useState(session.ttProjectId ?? "");
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
  const note = picked?.note ?? typed;
  const ttProjectId = picked?.ttProjectId ?? ownTt;
  const activeRef = useActiveScroll<HTMLLIElement>(current);
  // Display only; Rust decides whether the timer stays (`activity/pick.rs`).
  const unchanged = checkIn && !!timer && timer.projectId === ttProjectId && (timer.note ?? "").trim() === note.trim();
  useCheckInTimeout(checkIn, note);

  // Prefilled and selected: ↵ confirms, typing replaces it.
  useEffect(() => input.current?.select(), []);

  // TimeTracker's absence shows as its state, with a way to open it; the typed note stays.
  const failed = (e: unknown) => {
    const failure = toAppError(e);
    const state = TT_FAILURES[failure.kind];
    if (state) setTt(state);
    setError(state ? null : failure.message);
  };

  const submit = (text: string, forTt: string, from: ActivitySuggestion | null) => {
    // A tag belongs to its TimeTracker project.
    const [tag, tagTt] = from ? [from.tag, from.ttProjectId] : [session.tag, session.ttProjectId];
    api.activitySubmit(session.mode, { ttProjectId: forTt, note: text, tag: tagTt === forTt ? tag : null }).catch(failed);
  };

  /** What's shown becomes your own input. */
  const takeOver = () => {
    if (!picked) return;
    setTyped(picked.note);
    setOwnTt(ttProjectId);
    setOrigin(picked);
    setEdited(true);
    setHighlighted(-1);
  };

  const pickProject = (id: string) => {
    takeOver();
    setOwnTt(id);
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
    choices,
    ttProjectId,
    highlight: setHighlighted,
    pickProject,
    submit: () => submit(note, ttProjectId, picked ?? origin),
  });

  const hints = [choices.length > 1 && "⇥ next project", "↑↓ suggestions", unchanged ? "↵ still on it" : "↵ start", "esc cancel"].filter(Boolean).join(" · ");

  return (
    <div className="panel-card palette-view activity-view">
      <div className="panel-heading o2">{session.prompt}</div>
      <div className="palette-input" data-stage="panel:input" data-stage-label="Activity note" data-stage-role="field">
        <Timer size="1.077rem" aria-hidden="true" />
        <input
          ref={input}
          autoFocus
          aria-label="Note"
          placeholder={choices.length > 0 ? "Note, e.g. QRS-6805 review" : "What are you working on?"}
          value={note}
          onChange={(e) => {
            if (picked) {
              setOwnTt(ttProjectId);
              setOrigin(picked);
            }
            setTyped(e.target.value);
            setEdited(true);
            setHighlighted(-1);
            setError(null);
          }}
          onKeyDown={onKeyDown}
        />
      </div>
      <ProjectPicker projects={session.projects} picked={ttProjectId} onPick={pickProject} onHint={setError} />
      {suggestions.data && suggestions.data.length > 0 && (
        <SuggestionList items={items} highlighted={current} activeRef={activeRef} onPick={(suggestion) => submit(suggestion.note, suggestion.ttProjectId ?? ownTt, suggestion)} />
      )}
      <ProjectKeys projects={choices} />
      <div className="palette-foot o3 activity-foot">
        <ActivityStatus error={error} tt={tt} loadError={suggestions.error?.message ?? null} onOpen={() => void openTimeTracker()} />
        <span>{hints}</span>
      </div>
    </div>
  );
}
