/**
 * `clocks` — the play dock's section a pack may fill, mid-session.
 *
 * Roll for Shoes has no clocks. What it has is one thing the Game Master
 * says over and over during play — how hard this is — and the dock is where
 * a Game Master running the game without a map is sitting when they say it.
 * So the slot holds that. It shares its component with `world-staging`.
 */
import type { ClocksPanelProps } from "@thunderforge/host";

import { TablePanel } from "../components/TablePanel.tsx";

export default function RollForShoesClocksPanel({ worldId }: ClocksPanelProps) {
  return <TablePanel worldId={worldId} />;
}
