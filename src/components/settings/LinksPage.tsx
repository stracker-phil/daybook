import { useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import type { QuickLink } from "../../api";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { Group, Row, Switch } from "./controls";
import { ChoiceList, CommitInput, DaysField, ItemActions, moveItem, replaceItem, type Choice } from "./fields";
import { clockTime, LinkMeetingFields, LinkTimeFields } from "./LinkRuleFields";

const NEW_LINK: QuickLink = {
  title: "New link",
  target: "https://",
  contexts: null,
  projects: null,
  days: null,
  time: null,
  event: null,
  eventWindow: "until_end",
  leadMinutes: 15,
  pinned: true,
};

const DAY_NAMES = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];

const mondayFirst = (day: number) => (day + 6) % 7;

/** An empty pick means the rule is unset. */
const orNull = <T,>(list: T[]) => (list.length > 0 ? list : null);

/** `Work | Oscorp | Mo, Fr | 09:00–12:00 | 🗓️ Weekly`: when the link shows, for the collapsed card. */
function summary(link: QuickLink, contexts: Choice[]): string {
  const parts = [
    contexts.filter((context) => link.contexts?.includes(context.id)).map((context) => context.title).join(", "),
    (link.projects ?? []).join(", "),
    [...(link.days ?? [])].sort((a, b) => mondayFirst(a) - mondayFirst(b)).map((day) => DAY_NAMES[day]).join(", "),
    link.time ? `${clockTime(link.time.from)}–${clockTime(link.time.to)}` : "",
    link.event ? `🗓️ ${link.event}` : "",
  ].filter(Boolean);
  if (parts.length > 0) return parts.join(" | ");
  return link.pinned ? "Always" : "Only under More";
}

interface LinkCardProps {
  link: QuickLink;
  contexts: Choice[];
  projects: string[];
  index: number;
  count: number;
  open: boolean;
  onToggle: () => void;
  onChange: (patch: Partial<QuickLink>) => void;
  onMove: (from: number, to: number) => void;
  onRemove: () => void;
}

function LinkCard({ link, contexts, projects, index, count, open, onToggle, onChange, onMove, onRemove }: LinkCardProps) {
  const Chevron = open ? ChevronDown : ChevronRight;
  return (
    <Group>
      <div className="settings-row link-head">
        <button type="button" className="icon-btn tiny" aria-label={open ? "Hide Details" : "Show Details"} aria-expanded={open} onClick={onToggle}>
          <Chevron size="0.923rem" aria-hidden="true" />
        </button>
        <CommitInput label="Title" placeholder="Title" className={open ? "link-title grow" : "link-title"} value={link.title} onCommit={(title) => onChange({ title })} />
        {!open && <span className="link-summary o2">{summary(link, contexts)}</span>}
        <ItemActions index={index} count={count} onMove={onMove} onRemove={onRemove} removeLabel="Remove Link" />
      </div>
      {open && (
        <>
          <Row label="URL">
            <CommitInput
              label="URL"
              className="link-input"
              placeholder="https://… or a vault path, e.g. 2-Work/27-Bookmarks/Plan.md"
              required
              value={link.target}
              onCommit={(target) => onChange({ target })}
            />
          </Row>
          {contexts.length > 1 && (
            <Row label="Context" description="Only while one of these is current." stacked>
              <ChoiceList label="Context" choices={contexts} chosen={link.contexts ?? []} onChange={(ids) => onChange({ contexts: orNull(ids) })} placeholder="Add: type part of a context's name, ↵ adds it" />
            </Row>
          )}
          {projects.length > 0 && (
            <Row label="Project" description="Only while you work for one of these." stacked>
              <ChoiceList
                label="Project"
                choices={projects.map((name) => ({ id: name, title: name }))}
                chosen={link.projects ?? []}
                onChange={(names) => onChange({ projects: orNull(projects.filter((name) => names.includes(name))) })}
                placeholder="Add: type part of a project's name, ↵ adds it"
              />
            </Row>
          )}
          <Row label="Days">
            <DaysField label="Days" days={link.days ?? []} onChange={(days) => onChange({ days: orNull(days) })} />
          </Row>
          <LinkTimeFields time={link.time} onChange={(time) => onChange({ time })} />
          <LinkMeetingFields link={link} onChange={onChange} />
          <Switch
            label="Show when no rule is set"
            description="Without a context, project, day, time or meeting, the link shows under Now; off, under More."
            checked={link.pinned}
            onChange={(pinned) => onChange({ pinned })}
          />
        </>
      )}
    </Group>
  );
}

/** One card per link, collapsed to its title and when it shows; the details open below. */
export function LinksPage({ s, set }: SettingsPageProps) {
  // Which cards show their details, by position; they start collapsed.
  const [open, setOpen] = useState<boolean[]>([]);
  const links = s.links;
  const flags = links.map((_, i) => open[i] === true);
  const contexts = s.contexts.map((context) => ({ id: context.id, title: [context.label.trim(), context.name].filter(Boolean).join(" ") }));
  const projects = s.projects.map((project) => project.name);
  const save = (next: QuickLink[]) => set({ links: next.filter((link) => link.target || link.title) });
  // A card's open flag follows its link.
  const moveAt = (from: number, to: number) => {
    setOpen(moveItem(flags, from, to));
    save(moveItem(links, from, to));
  };
  const removeAt = (i: number) => {
    setOpen(flags.filter((_, j) => j !== i));
    save(links.filter((_, j) => j !== i));
  };
  const appendOpen = (link: QuickLink) => {
    setOpen([...flags, true]);
    save([...links, link]);
  };

  return (
    <>
      <p className="settings-group-desc">
        A link shows while everything you set for it applies: the context, the project of your current activity, the days and times, and a meeting in
        today’s calendar.
      </p>
      {links.map((link, i) => (
        <LinkCard
          key={i}
          link={link}
          contexts={contexts}
          projects={projects}
          index={i}
          count={links.length}
          open={flags[i]}
          onToggle={() => setOpen(flags.map((flag, j) => (j === i ? !flag : flag)))}
          onChange={(patch) => save(replaceItem(links, i, patch))}
          onMove={moveAt}
          onRemove={() => removeAt(i)}
        />
      ))}
      <div>
        <button type="button" className="btn" onClick={() => appendOpen(NEW_LINK)}>
          Add Link
        </button>
      </div>
    </>
  );
}
