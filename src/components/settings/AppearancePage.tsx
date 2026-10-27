import { TEXT_SIZES, type Scheme } from "../../api";
import { schemeLook } from "../../lib/theme";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { AccentSwatches } from "../Accent";
import { Group, Row, Segmented } from "./controls";
import { RailGroups } from "./RailGroups";

const SCHEME_LABELS: Record<Scheme, string> = { default: "Default", light: "Light", dark: "Dark" };

/** One tile per scheme: a miniature window in the scheme's look. */
function SchemePicker({ label, value, onChange }: { label: string; value: Scheme; onChange: (scheme: Scheme) => void }) {
  return (
    <div className="scheme-picker" role="group" aria-label={label}>
      {(Object.keys(SCHEME_LABELS) as Scheme[]).map((id) => {
        const look = schemeLook(id);
        return (
          <button key={id} type="button" className="scheme-option" aria-pressed={value === id} onClick={() => onChange(id)}>
            <span className="scheme-tile" data-content={look.content} data-sidebar={look.sidebar} aria-hidden="true">
              <span className="scheme-tile-sidebar">
                <span className="scheme-tile-entry" />
              </span>
              <span className="scheme-tile-content">
                <span className="scheme-tile-block" />
              </span>
            </span>
            {SCHEME_LABELS[id]}
          </button>
        );
      })}
    </div>
  );
}

const HOURS = Array.from({ length: 25 }, (_, hour) => hour);
const hourLabel = (hour: number) => `${String(hour).padStart(2, "0")}:00`;

function HourSelect({ label, value, hours, onChange }: { label: string; value: number; hours: number[]; onChange: (hour: number) => void }) {
  return (
    <Row label={label}>
      <select className="input" aria-label={label} value={value} onChange={(e) => onChange(Number(e.target.value))}>
        {hours.map((hour) => (
          <option key={hour} value={hour}>
            {hourLabel(hour)}
          </option>
        ))}
      </select>
    </Row>
  );
}

export function AppearancePage(props: SettingsPageProps) {
  const { s, set } = props;
  const timeline = s.timeline;
  return (
    <>
      <Group title="Look">
        <Row label="Light mode scheme" stacked>
          <SchemePicker label="Light mode scheme" value={s.lightScheme} onChange={(lightScheme) => set({ lightScheme })} />
        </Row>
        <Row label="Dark mode scheme" stacked>
          <SchemePicker label="Dark mode scheme" value={s.darkScheme} onChange={(darkScheme) => set({ darkScheme })} />
        </Row>
        <Row label="Accent" description="Buttons, links, selection and focus. System follows macOS.">
          <AccentSwatches withSystem value={s.accent} onChange={(accent) => set({ accent })} />
        </Row>
      </Group>
      <Group title="Text">
        <Segmented
          label="Text size"
          description="All text in the window. ⌘= and ⌘- step through the sizes."
          options={TEXT_SIZES.map((t) => t.size)}
          value={s.textSize}
          onChange={(textSize) => set({ textSize })}
        />
      </Group>
      <Group title="Day timeline" description="The ruler above the day's events in the Daybook window. Each context sets its own work hours in Settings → Contexts.">
        <HourSelect label="Starts at" value={timeline.firstHour} hours={HOURS.filter((hour) => hour < timeline.lastHour)} onChange={(firstHour) => set({ timeline: { ...timeline, firstHour } })} />
        <HourSelect label="Ends at" value={timeline.lastHour} hours={HOURS.filter((hour) => hour > timeline.firstHour)} onChange={(lastHour) => set({ timeline: { ...timeline, lastHour } })} />
      </Group>
      <RailGroups {...props} />
    </>
  );
}
