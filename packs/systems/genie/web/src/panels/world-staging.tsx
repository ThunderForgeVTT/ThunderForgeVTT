/**
 * `world-staging` — Genie's session loop on the pre-session staging page.
 *
 * The staging page used to hold `world?.gameSystemId === "genie"` and mount
 * this itself, wrapper, heading and all. It now mounts whatever pack fills
 * this slot and knows nothing about which one did; the chrome came with the
 * panel, because a "Genie session" heading is Genie's to write.
 *
 * `clocks.tsx` beside this file points at the same component on purpose —
 * see its header.
 */
import { Panel, type WorldStagingPanelProps } from "@thunderforge/host";
import { GenieSessionPanel } from "../components/SessionPanel";

export default function GenieStagingPanel({
  worldId,
  isGm,
  currentUserId,
}: WorldStagingPanelProps) {
  return (
    <Panel
      variant="stone"
      className="grid gap-3 rounded-xl border border-border"
      data-testid="genie-session-panel-wrapper"
    >
      <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
        Genie session
      </p>
      <GenieSessionPanel
        worldId={worldId}
        isGm={isGm}
        currentUserId={currentUserId}
      />
    </Panel>
  );
}
