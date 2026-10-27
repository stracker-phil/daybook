/**
 * The page side of the Stage bridge: the generated Stage types (Rust `stage::page`) plus the roles as a
 * list for the page's checks.
 */
import type { StageRole } from "../api/types";

export type {
  StageAct,
  StageActReply,
  StageActRequest,
  StageAnchorsReply,
  StageCollectRequest,
  StageKeys,
  StagePage,
  StagePageAnchor,
  StagePageMeta,
  StagePanelState,
  StageRect,
  StageRole,
  StageScrollTo,
  StageTypeMode,
} from "../api/types";

/** Rust's `Role`; the `Record` makes the compiler check the list is complete. */
const ROLES: Record<StageRole, true> = { region: true, button: true, field: true, item: true, tab: true, toggle: true, link: true, menu: true, text: true };

export const isStageRole = (value: string | null): value is StageRole => !!value && Object.hasOwn(ROLES, value);
