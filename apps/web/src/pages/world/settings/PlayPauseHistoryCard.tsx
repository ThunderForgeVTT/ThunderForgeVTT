import { Card } from "@/components/ui/card/Card";
import { useWorldPlayState } from "@/hooks/useWorldPlayState";
import {
  formatPauseMoment,
  pauseHistoryRows,
} from "@/pages/world/playPauseStatus";

export interface PlayPauseHistoryCardProps {
  worldId: string;
}

/**
 * Spec 051 US5 (FR-050, SC-004): when an operator paused this world's play,
 * and when each pause was lifted. Times only.
 *
 * Shown to every member, like the appearance card: the fact of a pause is the
 * table's to know, whoever is at it. Absent when the world has never been
 * paused, rather than a card announcing that nothing happened.
 */
export function PlayPauseHistoryCard({ worldId }: PlayPauseHistoryCardProps) {
  const rows = pauseHistoryRows(useWorldPlayState(worldId));
  if (rows.length === 0) return null;

  return (
    <Card className="grid gap-4 p-6" data-testid="play-pause-history-card">
      <div className="grid gap-1">
        <h3 className="text-lg font-semibold">Paused play</h3>
        <p className="text-sm text-muted-foreground">
          When an operator of this instance paused play in this world, and when
          play resumed.
        </p>
      </div>
      <div className="overflow-x-auto">
        <table className="w-full text-left text-sm">
          <thead>
            <tr className="border-b border-border">
              <th scope="col" className="py-2 pr-6 font-semibold">
                Paused
              </th>
              <th scope="col" className="py-2 font-semibold">
                Lifted
              </th>
            </tr>
          </thead>
          <tbody className="tabular-nums">
            {rows.map((span) => (
              <tr
                key={span.pausedAt}
                className="border-b border-border last:border-b-0"
                data-testid="play-pause-history-row"
              >
                <td className="py-2 pr-6">
                  <time dateTime={span.pausedAt}>
                    {formatPauseMoment(span.pausedAt)}
                  </time>
                </td>
                <td className="py-2">
                  {span.liftedAt ? (
                    <time dateTime={span.liftedAt}>
                      {formatPauseMoment(span.liftedAt)}
                    </time>
                  ) : (
                    "Still paused"
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </Card>
  );
}
