import { api, type Rail, type RailCallEdge, type RailCallPosition, type RailEdges, type RailHighlight, type RailPosition, type RailShape, type RailSize, type RailSpan, type RailTheme } from "../../api";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { fail } from "../../state/requests";
import { Group, Row, Segmented, Switch } from "./controls";

const EDGE_LABELS: Record<RailEdges, string> = { right: "Right", left: "Left", both: "Both" };
const SPAN_LABELS: Record<RailSpan, string> = { full: "Whole edge", rail: "Beside the rail" };
const SHAPE_LABELS: Record<RailShape, string> = { pill: "Pill", docked: "Docked", floating: "Floating icons" };
const HIGHLIGHT_LABELS: Record<RailHighlight, string> = { tinted: "Tinted", filled: "Filled", gray: "Gray" };
const POSITION_LABELS: Record<RailPosition, string> = { center: "Centered", top: "Top", bottom: "Bottom", pointer: "At the pointer" };
const CALL_POSITION_LABELS: Record<RailCallPosition, string> = { top: "Top", center: "Centered", bottom: "Bottom" };

/** A segmented control's options, in the order of its labels. */
const optionsOf = <T extends string>(labels: Record<T, string>) => Object.keys(labels) as T[];

const THEMES: RailTheme[] = ["system", "light", "dark"];
const SIZES: RailSize[] = ["compact", "regular", "large"];
const CALL_EDGES: RailCallEdge[] = ["left", "right"];

type Presets = readonly (readonly [number, string])[];

const TRIGGER_WIDTHS: Presets = [
  [1, "Very edge (1 pt)"],
  [2, "Narrow (2 pt)"],
  [6, "Medium (6 pt)"],
  [12, "Wide (12 pt)"],
  [24, "Extra wide (24 pt)"],
];
const SHOW_DELAYS: Presets = [
  [0, "Instantly"],
  [100, "0.1 s"],
  [200, "0.2 s"],
  [400, "0.4 s"],
  [700, "0.7 s"],
];
const HIDE_DELAYS: Presets = [
  [300, "0.3 s"],
  [700, "0.7 s"],
  [1500, "1.5 s"],
  [3000, "3 s"],
];
const TOOLTIP_DELAYS: Presets = [
  [0, "Instantly"],
  [150, "Short"],
  [300, "Normal"],
  [600, "Long"],
];

/** A number from a few presets; a value set elsewhere shows as its own entry. */
function Preset({ label, description, value, presets, unit, disabled, onChange }: {
  label: string;
  description?: string;
  value: number;
  presets: Presets;
  unit: string;
  disabled?: boolean;
  onChange: (value: number) => void;
}) {
  const options = presets.some(([preset]) => preset === value) ? presets : [...presets, [value, `${value} ${unit}`] as const];
  return (
    <Row label={label} description={description} disabled={disabled}>
      <select className="input preset" aria-label={label} value={value} disabled={disabled} onChange={(e) => onChange(Number(e.target.value))}>
        {options.map(([option, text]) => (
          <option key={option} value={option}>
            {text}
          </option>
        ))}
      </select>
    </Row>
  );
}

const toggleRail = () => void api.runCommand("rail:toggle").catch((e) => fail(e, "Couldn't show the rail"));

export function RailPage({ s, set }: SettingsPageProps) {
  const rail = s.rail;
  const change = (patch: Partial<Rail>) => set({ rail: { ...rail, ...patch } });
  const edgeOff = !rail.enabled;
  const noCall = !rail.showDuringMeetings;
  return (
    <>
      <Group
        title="Rail"
        description="A bar of tools at the screen edge: status, tools, journal progress, links and music. It works over full-screen apps and never takes focus."
      >
        <Row label="Try it" description="Shows the rail on this screen so you can see changes as you make them; again hides it.">
          <button type="button" className="btn" onClick={toggleRail}>
            Show or Hide the Rail
          </button>
        </Row>
      </Group>
      <Group
        title="Showing the Rail"
        description="Opened with the rail shortcut it stays until you press the shortcut again, or until you move the pointer to its edge: from then on it hides once the pointer leaves, like a rail shown at the edge."
      >
        <Switch
          label="Show at the screen edge"
          description="Rest the pointer at the edge to slide it in. The rail shortcut works either way."
          checked={rail.enabled}
          onChange={(enabled) => change({ enabled })}
        />
        <Segmented
          label="Edges"
          description="With both, it opens on the edge you came to; the shortcut opens it on the side of the screen the pointer is on."
          options={optionsOf(EDGE_LABELS)}
          labels={EDGE_LABELS}
          value={rail.edges}
          onChange={(edges) => change({ edges })}
        />
        <Preset
          label="Trigger zone"
          description="How close to the edge the pointer has to be. Wider is easier where another screen continues past the edge."
          value={rail.triggerWidth}
          presets={TRIGGER_WIDTHS}
          unit="pt"
          disabled={edgeOff}
          onChange={(triggerWidth) => change({ triggerWidth })}
        />
        <Segmented
          label="Trigger along"
          description="Beside the rail keeps the rest of the edge free, e.g. for scroll bars and hot corners."
          options={optionsOf(SPAN_LABELS)}
          labels={SPAN_LABELS}
          value={rail.triggerSpan}
          disabled={edgeOff || rail.position === "pointer"}
          onChange={(triggerSpan) => change({ triggerSpan })}
        />
        <Preset
          label="Show after"
          description="How long the pointer rests at the edge first."
          value={rail.showDelay}
          presets={SHOW_DELAYS}
          unit="ms"
          disabled={edgeOff}
          onChange={(showDelay) => change({ showDelay })}
        />
        <Preset
          label="Hide after"
          description="How long the pointer stays away before it slides out."
          value={rail.hideDelay}
          presets={HIDE_DELAYS}
          unit="ms"
          onChange={(hideDelay) => change({ hideDelay })}
        />
        <Switch
          label="Keep it visible during meetings"
          description="While a call is detected (Workflows → Calls), the rail stays on screen."
          checked={rail.showDuringMeetings}
          onChange={(showDuringMeetings) => change({ showDuringMeetings })}
        />
        <Segmented
          label="Edge during a call"
          description="A rail kept for a call sits on this edge, whatever Edges says."
          options={CALL_EDGES}
          value={rail.callEdge}
          disabled={noCall}
          onChange={(callEdge) => change({ callEdge })}
        />
        <Segmented
          label="Position during a call"
          description="Where on that edge, instead of Position below."
          options={optionsOf(CALL_POSITION_LABELS)}
          labels={CALL_POSITION_LABELS}
          value={rail.callPosition}
          disabled={noCall}
          onChange={(callPosition) => change({ callPosition })}
        />
      </Group>
      <Group title="Look" description="The rail and its tooltips follow these.">
        <Segmented label="Appearance" options={THEMES} value={rail.theme} onChange={(theme) => change({ theme })} />
        <Segmented
          label="Shape"
          description="Pill floats beside the edge, Docked sits flush against it, Floating icons drops the bar: each tool is its own round button."
          options={optionsOf(SHAPE_LABELS)}
          labels={SHAPE_LABELS}
          value={rail.shape}
          onChange={(shape) => change({ shape })}
        />
        <Segmented label="Size" options={SIZES} value={rail.size} onChange={(size) => change({ size })} />
        <Segmented
          label="Hover highlight"
          description="How the button under the pointer stands out: tinted or filled with your accent colour, or grey."
          options={optionsOf(HIGHLIGHT_LABELS)}
          labels={HIGHLIGHT_LABELS}
          value={rail.highlight}
          onChange={(highlight) => change({ highlight })}
        />
        <Segmented
          label="Position"
          description="Where on the edge it appears. At the pointer centres it where you entered the edge."
          options={optionsOf(POSITION_LABELS)}
          labels={POSITION_LABELS}
          value={rail.position}
          onChange={(position) => change({ position })}
        />
        <Switch
          label="Captions"
          description="Short lines under the icons: break minutes, journal progress, link count."
          checked={rail.captions}
          onChange={(captions) => change({ captions })}
        />
        <Switch
          label="Fade when idle"
          description="A rail that stays (shortcut, meeting) fades back while the pointer is elsewhere and comes back when you point at it."
          checked={rail.fadeWhenIdle}
          onChange={(fadeWhenIdle) => change({ fadeWhenIdle })}
        />
      </Group>
      <Group title="Tooltips">
        <Preset
          label="First tooltip after"
          description="Moving along the rail then shows the next ones at once."
          value={rail.tooltipDelay}
          presets={TOOLTIP_DELAYS}
          unit="ms"
          onChange={(tooltipDelay) => change({ tooltipDelay })}
        />
        <Switch
          label="Show choices in tooltips"
          description="Contexts, playlists and links right in the tooltip, to pick without opening anything."
          checked={rail.hoverMenus}
          onChange={(hoverMenus) => change({ hoverMenus })}
        />
      </Group>
    </>
  );
}
