/**
 * `world-settings` — Session Resource carryover, on the world's System
 * settings page.
 *
 * This was an inline card in `WorldSystemSettingsPage.tsx` behind
 * `isGm && world.gameSystemId === "genie"`, along with its own saving state
 * and error handling and a `updateWorldGenieResourceCarryover` in the shared
 * world API module. All of it is Genie's, and all of it is here now.
 *
 * # What is deliberately left behind
 *
 * The column. `worlds.genie_resource_carryover_enabled` is still a column
 * named for one ruleset on a table every system shares, and moving the panel
 * does not fix that — it is a migration and a rename on the server, tracked
 * as its own loose end. `check-system-registry.mjs` does not catch it either
 * way: it is a column name, not a quoted id and not a filename.
 *
 * The card reads the flag off the `WorldRecord` the host hands it, which is
 * the same shape every other consumer sees. That is the honest cost of the
 * column being where it is, stated rather than hidden.
 */
import { useState } from "react";
import {
  Card,
  postGraphQL,
  type WorldSettingsPanelProps,
} from "@thunderforge/host";

type UpdateCarryoverMutation = {
  updateWorldGenieResourceCarryover: { id: string };
};

/**
 * Spec 020 (FR-003, research.md R1): GM-only, and enforced on the server —
 * the `isGm` prop below only decides whether to draw the control.
 *
 * Selects `id` alone. The mutation returns the whole world, but this panel
 * does not need it and taking it would tie the pack to this app's
 * `WorldRecord` shape; the host is told to re-read instead
 * (`onWorldChanged`).
 */
function setCarryover(worldId: string, enabled: boolean): Promise<void> {
  return postGraphQL<UpdateCarryoverMutation>(
    `
      mutation UpdateWorldGenieResourceCarryover($input: UpdateWorldGenieResourceCarryoverInput!) {
        updateWorldGenieResourceCarryover(input: $input) {
          id
        }
      }
    `,
    { input: { worldId, enabled } },
  ).then(() => undefined);
}

export default function GenieWorldSettingsPanel({
  worldId,
  world,
  isGm,
  onWorldChanged,
}: WorldSettingsPanelProps) {
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!isGm) {
    return null;
  }

  const handleToggle = async (enabled: boolean) => {
    setIsSaving(true);
    setError(null);
    try {
      await setCarryover(worldId, enabled);
      onWorldChanged();
    } catch (err) {
      setError(
        err instanceof Error
          ? err.message
          : "Failed to update resource carryover setting",
      );
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <Card className="grid gap-3 p-6" data-testid="genie-resource-carryover-card">
      <h2 className="text-lg font-semibold">Session Resource carryover</h2>
      <p className="text-sm text-muted-foreground">
        When enabled, players&apos; Insight/Favor/Essence holdings carry over
        into the next Genie session instead of resetting to 0 — &quot;the rope
        doesn&apos;t disappear.&quot;
      </p>
      <label className="flex items-center gap-2 text-sm">
        <input
          type="checkbox"
          checked={world.genieResourceCarryoverEnabled}
          disabled={isSaving}
          onChange={(event) => void handleToggle(event.target.checked)}
          data-testid="genie-resource-carryover-toggle"
        />
        Carry over Session Resource holdings between sessions
      </label>
      {error ? (
        <p className="text-sm text-destructive" data-testid="genie-carryover-error">
          {error}
        </p>
      ) : null}
    </Card>
  );
}
