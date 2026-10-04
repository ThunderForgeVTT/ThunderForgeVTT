/**
 * `world-staging` — the page a Game Master is on before the session starts.
 *
 * The same control the play dock holds (`clocks.tsx`), reached before play
 * as well as during it: a Game Master who opens the world to set a scene in
 * words rather than on a map can set what has to be beaten from here.
 */
import type { WorldStagingPanelProps } from "@thunderforge/host";

import { TablePanel } from "../components/TablePanel.tsx";

export default function RollForShoesStagingPanel({
  worldId,
}: WorldStagingPanelProps) {
  return <TablePanel worldId={worldId} />;
}
