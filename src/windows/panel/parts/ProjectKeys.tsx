import { PanelHintItem } from "./PanelFooter";

/** ⌘1…⌘9 pick a project by position. */
export const MAX_PROJECT_KEYS = 9;

/** "⌘1 Internal  ⌘2 Oscorp …": the first nine names, in the order the view shows them. */
export function ProjectKeys({ names }: { names: string[] }) {
  const shown = names.slice(0, MAX_PROJECT_KEYS);
  if (shown.length === 0) return null;
  return (
    <div className="project-keys" aria-label="Project keys">
      {shown.map((name, index) => (
        <PanelHintItem key={`${index}:${name}`} keys={[`⌘${index + 1}`]} label={name} />
      ))}
    </div>
  );
}
