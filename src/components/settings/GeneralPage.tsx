import type { SettingsPageProps } from "../../registry/settingsPages";
import { Group } from "./controls";
import { LaunchAtLoginSwitch } from "./LaunchAtLoginSwitch";
import { ToolsGroup } from "./ToolsGroup";

export function GeneralPage(props: SettingsPageProps) {
  return (
    <>
      <Group>
        <LaunchAtLoginSwitch />
      </Group>
      <ToolsGroup {...props} />
    </>
  );
}
