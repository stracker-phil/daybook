import type { SettingsPageProps } from "../../registry/settingsPages";
import { CalendarGroup } from "./CalendarGroup";
import { JiraGroup } from "./JiraGroup";
import { SlackGroup } from "./SlackGroup";
import { TimeTrackerGroup } from "./TimeTrackerGroup";
import { VaultGroups } from "./VaultGroups";

export function ConnectionsPage(props: SettingsPageProps) {
  return (
    <>
      <VaultGroups {...props} />
      <CalendarGroup />
      <SlackGroup />
      <JiraGroup {...props} />
      <TimeTrackerGroup {...props} />
    </>
  );
}
