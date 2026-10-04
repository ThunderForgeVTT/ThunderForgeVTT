import { useEffect, useState } from "react";
import { Card } from "@thunderforge/host";

import { DEFAULT_SETTINGS, type WorldSettings } from "../game.ts";
import { fetchWorldSettings } from "../settings.ts";
import { useTableDifficulty } from "../useTableDifficulty.ts";
import { TableDifficulty } from "./TableDifficulty.tsx";
import { hintClass } from "./styles.ts";

/**
 * The Game Master's seat at a table with no map.
 *
 * Roll for Shoes is mostly theatre of the mind, and a Game Master running it
 * that way has no token to click and no sheet of their own open. This is
 * where they say how hard a thing is. It fills two slots with the one
 * component — the play dock, where they are mid-session, and the staging
 * page, where they are before it — for the reason Genie's session panel
 * does: it is the same thing reached from two places.
 *
 * A player who opens it sees the number and nothing to change. Their own
 * sheet shows it too; this is not the only place they learn it.
 */
export function TablePanel({ worldId }: { worldId: string }) {
  const table = useTableDifficulty(worldId);
  // The mode only decides whether the bands are offered. A failed read
  // leaves the core game's control — a number — which is always allowed.
  const [settings, setSettings] = useState<WorldSettings>(DEFAULT_SETTINGS);

  useEffect(() => {
    let cancelled = false;
    void fetchWorldSettings(worldId)
      .then((stored) => {
        if (!cancelled) {
          setSettings(stored);
        }
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [worldId]);

  return (
    <Card className="grid gap-2 p-4" data-testid="rfs-table-panel">
      {table.state === "failed" ? (
        <p className={hintClass} data-testid="rfs-table-failed">
          What has to be beaten could not be read.{" "}
          <button
            type="button"
            className="underline"
            onClick={() => void table.refresh().catch(() => undefined)}
          >
            Try again
          </button>
        </p>
      ) : (
        <TableDifficulty
          difficulty={table.difficulty}
          mode={settings.difficultyMode}
          busy={table.busy || table.state !== "ready"}
          refusal={table.refusal}
          onSetNumber={(target) => void table.set({ target })}
          onSetBand={(band) => void table.set({ band })}
          onClear={() => void table.clear()}
        />
      )}
    </Card>
  );
}

export default TablePanel;
