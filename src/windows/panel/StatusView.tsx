import { useState, type KeyboardEvent } from "react";
import { CircleCheck, Coffee, Moon, type LucideIcon } from "lucide-react";
import { api, toAppError, type EndOfDayAnswer, type Status, type StatusMap, type StatusRequest } from "../../api";
import { EndOfDayStep } from "./EndOfDayStep";
import { EndOfDayListStep } from "./EndOfDayListStep";

const OPTIONS: { status: Status; icon: LucideIcon; label: string; placeholder: string }[] = [
  { status: "working", icon: CircleCheck, label: "Working and present", placeholder: "Message, e.g. good morning" },
  { status: "break", icon: Coffee, label: "Taking a break", placeholder: "Message, e.g. out for lunch" },
  { status: "off", icon: Moon, label: "Done for today", placeholder: "Message" },
];

const STEPS: Record<string, number> = { ArrowDown: 1, ArrowUp: -1 };

/** The suggested message follows the pick; anything typed stays. */
function useStatusMessage(suggested: StatusMap<string>, initial: Status) {
  const [picked, setPicked] = useState<Status>(initial);
  const [message, setMessage] = useState(suggested[initial]);
  const isSuggestion = !message.trim() || message === suggested[picked];
  const messageFor = (status: Status) => (isSuggestion ? suggested[status] : message);
  const pick = (status: Status) => {
    if (status === picked) return;
    setMessage(messageFor(status));
    setPicked(status);
  };
  return { picked, pick, message, setMessage, messageFor };
}

/**
 * "What is your status?": the message field keeps the keyboard, ↑↓ or ⌘1–3 pick, ↵ or a click sets.
 * Going Off asks for the end-of-day list, then the check; both go out with the final answer.
 */
export function StatusView({ request }: { request: StatusRequest }) {
  const { picked, pick, message, setMessage, messageFor } = useStatusMessage(request.messages, request.status);
  const [step, setStep] = useState<"pick" | "list" | "check">("pick");
  const [list, setList] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const send = (status: Status, answer?: EndOfDayAnswer, endOfDayList?: string) => {
    api.statusSet(status, messageFor(status), answer, endOfDayList).catch((e) => setError(toAppError(e).message));
  };
  const submit = (status: Status) => {
    if (status !== "off") send(status);
    else if (request.endOfDayList) setStep("list");
    else if (request.endOfDay) setStep("check");
    else send(status);
  };

  if (step === "list") {
    return (
      <EndOfDayListStep
        error={error}
        onContinue={(written) => {
          setList(written);
          if (request.endOfDay) setStep("check");
          else send("off", undefined, written ?? undefined);
        }}
      />
    );
  }
  if (step === "check" && request.endOfDay) {
    return <EndOfDayStep check={request.endOfDay} message={messageFor("off")} error={error} onAnswer={(answer) => send("off", answer, list ?? undefined)} />;
  }

  const current = OPTIONS.findIndex((option) => option.status === picked);
  const option = OPTIONS[current];
  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    const step = STEPS[e.key];
    const numbered = e.metaKey ? OPTIONS.findIndex((_, i) => e.code === `Digit${i + 1}`) : -1;
    if (step) {
      e.preventDefault();
      pick(OPTIONS[(current + step + OPTIONS.length) % OPTIONS.length].status);
    } else if (numbered >= 0) {
      e.preventDefault();
      pick(OPTIONS[numbered].status);
    } else if (e.key === "Enter") {
      e.preventDefault();
      submit(picked);
    }
  };

  return (
    <div className="panel-card status-view">
      <div className="panel-heading o2">What is your status?</div>
      <div className="palette-input">
        <option.icon size="1.077rem" aria-hidden="true" />
        <input
          autoFocus
          aria-label="Message"
          aria-controls="status-options"
          aria-activedescendant={`status-${picked}`}
          placeholder={option.placeholder}
          value={message}
          onChange={(e) => {
            setMessage(e.target.value);
            setError(null);
          }}
          onKeyDown={onKeyDown}
        />
      </div>
      <ul id="status-options" className="palette-list" role="listbox" aria-label="Status">
        {OPTIONS.map((item, i) => (
          <li
            key={item.status}
            id={`status-${item.status}`}
            role="option"
            aria-selected={item.status === picked}
            className={item.status === picked ? "active" : undefined}
            onMouseMove={() => pick(item.status)}
            // Keeps the keyboard in the message field.
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => submit(item.status)}
          >
            <span className="context-number o3">⌘{i + 1}</span>
            <item.icon size="1.077rem" className="o3" aria-hidden="true" />
            <span className="grow-text">{item.label}</span>
            {request.current === item.status && <span className="current-pill">Current</span>}
          </li>
        ))}
      </ul>
      <div className="palette-foot o3">{error ? <span className="palette-error">{error}</span> : <span>↑↓ status · ↵ set · esc close</span>}</div>
    </div>
  );
}
