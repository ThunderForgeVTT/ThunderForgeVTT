import { Link } from "react-router-dom";
import { titleFor, type GameSystemSummary } from "@/api/gameSystems";
import { DataTable } from "@/components/ui/data-table/DataTable";
import { useWorldMemberCounts } from "@/hooks/useWorldMemberCounts";
import { useWorldPlayState } from "@/hooks/useWorldPlayState";
import { formatPauseMoment, pausedSince } from "@/pages/world/playPauseStatus";
import { isWorldMemberRole, roleLabel } from "@/types/world";
import type { WorldRecord } from "@/types/world";

/** One world as the archive lists it: the world, and the caller's standing
 * in it. `role` is `null` for an operator browsing worlds they are not in. */
export interface WorldListEntry {
  world: WorldRecord;
  role: string | null;
}

interface WorldsTableProps {
  entries: readonly WorldListEntry[];
  systems: readonly GameSystemSummary[];
  /** Admin scope: the archive is showing worlds other people own. */
  showOwner?: boolean;
}

/**
 * The world archive as rows.
 *
 * # Which columns
 *
 * The owner's list, and the reason it is that list: these are the things a
 * person reads to decide *which world to open*. The name, the system it
 * runs, what they are in it, how many people are at the table, and whether
 * play is paused — a paused world is one you cannot enter, which is the one
 * fact that changes what the row is for.
 *
 * Everything the tile offers is still reachable from the row: the dashboard
 * and the live workspace, the two links a `WorldCard` ends with. A table
 * that drops an action is a table you have to leave to act.
 *
 * # Last played is not here, and that is deliberate
 *
 * It is the column a Game Master would reach for first, and the product does
 * not have it on this screen. The underlying fact *is* recorded —
 * `world_live_play.last_beat_at`, written by the heartbeat (spec 051) — but
 * nothing exposes it to a list: `worldPlayState(worldId)` is per world and
 * its type carries `paused`/`pausedAt` and no beat. Spec 054 (FR-012,
 * FR-012a) is where that gets read properly, per world, for the dashboard.
 * Inventing it here from `updatedAt` would be a lie a person would plan a
 * session around — `updatedAt` moves when a GM renames a scene at breakfast.
 *
 * # Not a compendium of every fact
 *
 * Created-at, updated-at, interface pack and owner id are on the tile and
 * are not columns. They are what you read *about* a world you have already
 * chosen, and the dashboard shows them. Six columns fit; eleven would make
 * this the thing the tiles were.
 */
export function WorldsTable({
  entries,
  systems,
  showOwner = false,
}: WorldsTableProps) {
  const worldIds = entries.map((entry) => entry.world.id);
  const memberCounts = useWorldMemberCounts(worldIds);

  const columns = showOwner
    ? ([
        "World",
        "Game system",
        "Owner",
        "Your role",
        "Players",
        "Play",
      ] as const)
    : (["World", "Game system", "Your role", "Players", "Play"] as const);

  return (
    <DataTable label="Your worlds" columns={columns} data-testid="worlds-table">
      <tbody>
        {entries.map((entry) => (
          <WorldRow
            key={entry.world.id}
            entry={entry}
            systems={systems}
            memberCount={memberCounts[entry.world.id] ?? null}
            showOwner={showOwner}
          />
        ))}
      </tbody>
    </DataTable>
  );
}

function WorldRow({
  entry,
  systems,
  memberCount,
  showOwner,
}: {
  entry: WorldListEntry;
  systems: readonly GameSystemSummary[];
  memberCount: number | null;
  showOwner: boolean;
}) {
  const { world, role } = entry;
  // Spec 051 US5, exactly as the tile reads it: that and when, never why.
  const pausedAt = pausedSince(useWorldPlayState(world.id));

  return (
    <tr
      className="border-b border-border last:border-b-0"
      data-testid="worlds-table-row"
    >
      {/* `th scope="row"` rather than a sixth `td`: the world's name is what
          names the row, and a screen reader moving across the row should
          hear it before each value. */}
      <th scope="row" className="px-3 py-3 text-left align-top font-medium">
        <Link
          to={`/world/${world.id}`}
          className="underline-offset-4 hover:underline"
          data-testid="worlds-table-name"
        >
          {world.name}
        </Link>
        <span className="mt-1 block text-xs font-normal">
          <Link
            to={`/world/${world.id}/staging`}
            className="text-primary underline-offset-4 hover:underline"
          >
            Enter world
          </Link>
        </span>
      </th>
      <td className="px-3 py-3 align-top">
        {world.gameSystemId ? (
          titleFor([...systems], world.gameSystemId)
        ) : (
          <span className="text-muted-foreground">Not assigned</span>
        )}
      </td>
      {showOwner ? (
        <td className="px-3 py-3 align-top font-mono text-xs">
          {world.createdBy.slice(0, 8)}…
        </td>
      ) : null}
      <td className="px-3 py-3 align-top">
        {role && isWorldMemberRole(role) ? (
          roleLabel(role)
        ) : (
          // An operator browsing the whole archive is in none of these
          // worlds. Saying "Player" would be worse than saying nothing.
          <span className="text-muted-foreground">Not a member</span>
        )}
      </td>
      <td className="px-3 py-3 align-top tabular-nums">
        {memberCount === null ? (
          <span className="text-muted-foreground" aria-label="Players unknown">
            —
          </span>
        ) : (
          memberCount
        )}
      </td>
      <td className="px-3 py-3 align-top">
        {pausedAt ? (
          <span data-testid="worlds-table-paused">
            Paused since{" "}
            <time dateTime={pausedAt} className="tabular-nums">
              {formatPauseMoment(pausedAt, "short")}
            </time>
          </span>
        ) : (
          <span className="text-muted-foreground">Not paused</span>
        )}
      </td>
    </tr>
  );
}
