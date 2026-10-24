import type { DailyNote, DailySection, HeadingLevel, SectionPlacement } from "../../api";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { Group, Row, TextField } from "./controls";
import { ItemActions, moveItem } from "./fields";

const SECTION_NAMES: Record<DailySection, string> = {
  journal: "Journal",
  focus: "Focus",
  plan: "Routine steps",
  notes: "Notes",
};

const SECTION_HINTS: Record<DailySection, string> = {
  journal: "Journal answers; created with the first answer.",
  focus: "The day's focus list, shared in Slack: [ ] open, [/] in progress, [x] done, [-] canceled (not Markdown tasks, so they aren't counted as todos).",
  plan: "Routine items as tasks: - [x] done, - [ ] open.",
  notes: "Its ### topics are today's notes and the Quick Notes sections.",
};

const LEVELS: HeadingLevel[] = ["##", "###", "####"];
const levelLabel = (level: HeadingLevel) => `Level ${level.length} (${level})`;

const isLevel = (value: string): value is HeadingLevel => (LEVELS as string[]).includes(value);

/** The notes heading is stored as one line ("## Notes"); shown like the others. */
function notesPlacement(line: string): SectionPlacement {
  const match = /^(#{2,4})\s+(.+)$/.exec(line.trim());
  const level = match?.[1] ?? "##";
  return { level: isLevel(level) ? level : "##", text: match?.[2] ?? "Notes" };
}

function placementOf(dailyNote: DailyNote, section: DailySection): SectionPlacement {
  return section === "notes" ? notesPlacement(dailyNote.notes) : dailyNote[section];
}

function withPlacement(dailyNote: DailyNote, section: DailySection, placement: SectionPlacement): DailyNote {
  if (section === "notes") return { ...dailyNote, notes: `${placement.level} ${placement.text}` };
  return { ...dailyNote, [section]: placement };
}

function SectionRow({ section, placement, index, count, onChange, onMove }: {
  section: DailySection;
  placement: SectionPlacement;
  index: number;
  count: number;
  onChange: (placement: SectionPlacement) => void;
  onMove: (from: number, to: number) => void;
}) {
  const name = SECTION_NAMES[section];
  const onLevelChange = (value: string) => {
    if (isLevel(value)) onChange({ ...placement, level: value });
  };
  return (
    <Row label={name} description={SECTION_HINTS[section]}>
      <select className="input" aria-label={`${name} heading level`} value={placement.level} onChange={(e) => onLevelChange(e.target.value)}>
        {LEVELS.map((level) => (
          <option key={level} value={level}>
            {levelLabel(level)}
          </option>
        ))}
      </select>
      <HeadingText label={`${name} heading text`} value={placement.text} onCommit={(text) => onChange({ ...placement, text })} />
      <ItemActions index={index} count={count} onMove={onMove} />
    </Row>
  );
}

/** A bare heading text input inside a section row; blank keeps the old text. */
function HeadingText({ label, value, onCommit }: { label: string; value: string; onCommit: (text: string) => void }) {
  return (
    <input
      key={value}
      className="input"
      aria-label={label}
      defaultValue={value}
      onBlur={(e) => {
        const text = e.target.value.trim();
        if (text && text !== value) onCommit(text);
        else e.target.value = value;
      }}
      onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
    />
  );
}

export function DailyNotePage({ s, set }: SettingsPageProps) {
  const dailyNote = s.dailyNote;
  const save = (next: DailyNote) => set({ dailyNote: next });
  return (
    <>
      <Group title="Fallback" description="Used when Obsidian's daily-notes settings name none.">
        <TextField label="Folder" description="Relative to the vault; empty: the vault's root." value={dailyNote.fallbackFolder} onCommit={(fallbackFolder) => save({ ...dailyNote, fallbackFolder: fallbackFolder.trim() })} />
        <TextField label="Format" description="The note's name, e.g. YYYY-MM-DD." value={dailyNote.fallbackFormat} onCommit={(format) => {
            if (format.trim()) save({ ...dailyNote, fallbackFormat: format.trim() });
          }} />
      </Group>
      <Group title="Sections" description="Each section is found by its heading level and text (ignoring case) and created in this order when Daybook first writes to it.">
        {dailyNote.order.map((section, index) => (
          <SectionRow
            key={section}
            section={section}
            placement={placementOf(dailyNote, section)}
            index={index}
            count={dailyNote.order.length}
            onChange={(placement) => save(withPlacement(dailyNote, section, placement))}
            onMove={(from, to) => save({ ...dailyNote, order: moveItem(dailyNote.order, from, to) })}
          />
        ))}
      </Group>
    </>
  );
}
