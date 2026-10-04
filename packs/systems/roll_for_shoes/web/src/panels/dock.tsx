/**
 * `dock` — the play dock's section a pack may fill, mid-session.
 *
 * What Roll for Shoes has is one thing the Game Master says over and over
 * during play — how hard this is — and the dock is where a Game Master
 * running the game without a map is sitting when they say it. So the slot
 * holds that. It shares its component with `world-staging`.
 */
import type { DockPanelProps } from "@thunderforge/host";

import { TablePanel } from "../components/TablePanel.tsx";

/** What the dock tab reads. */
export const title = "Table";

export default function RollForShoesDockPanel({ worldId }: DockPanelProps) {
  return <TablePanel worldId={worldId} />;
}
