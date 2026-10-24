import type { ToolPlacement } from "../../api";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { Group, Row } from "./controls";
import { ItemActions, moveItem, replaceItem } from "./fields";
import { TOOL_INFO } from "./toolInfo";

function PlaceSwitch({ label, checked, disabled, onChange }: { label: string; checked: boolean; disabled?: boolean; onChange: (on: boolean) => void }) {
  return (
    <label className="check">
      <input type="checkbox" role="switch" className="switch" checked={checked} disabled={disabled} onChange={(e) => onChange(e.target.checked)} />
      {label}
    </label>
  );
}

export function ToolsPage({ s, set }: SettingsPageProps) {
  const tools = s.tools;
  const save = (next: ToolPlacement[]) => set({ tools: next });
  return (
    <Group
      title="Tools"
      description="Order and place of the tools in the rail and the Daybook window; the menu-bar menu uses the same order. A tool that has nothing to offer right now stays hidden."
    >
      {tools.map((tool, i) => {
        const info = TOOL_INFO[tool.id];
        const Icon = info.icon;
        return (
          <Row key={tool.id} label={info.label} icon={<Icon size="1.077rem" aria-hidden="true" />} description={info.description}>
            <PlaceSwitch label="Rail" checked={tool.rail} onChange={(rail) => save(replaceItem(tools, i, { rail }))} />
            <PlaceSwitch label="Window" checked={tool.window} disabled={tool.id === "hub"} onChange={(window) => save(replaceItem(tools, i, { window }))} />
            <ItemActions index={i} count={tools.length} onMove={(from, to) => save(moveItem(tools, from, to))} />
          </Row>
        );
      })}
    </Group>
  );
}
