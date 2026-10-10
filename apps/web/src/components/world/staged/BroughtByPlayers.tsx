import { useCallback, useEffect, useState } from "react";
import type { UserSummary } from "@/api/sheetImport";
import {
  adoptAllStagedContent,
  adoptStagedContent,
  declineStagedContent,
  listStagedContent,
  revisitStagedContent,
  type StagedPiece,
} from "@/api/stagedContent";
import { Button } from "@/components/ui/button/Button";
import { Loader } from "@/components/ui/loader/Loader";
import { differsFromPlayer, groupByPlayer } from "./grouping";
import { StagedRow } from "./StagedRow";

export interface BroughtByPlayersViewProps {
  pieces: StagedPiece[];
  busy: boolean;
  error: string | null;
  onAdopt: (piece: StagedPiece) => void;
  onAdoptAll: (player: UserSummary) => void;
  onDecline: (piece: StagedPiece) => void;
  onRevisit: (piece: StagedPiece) => void;
}

/** The queue as drawn, with no fetching, so it renders in a test. */
export function BroughtByPlayersView({
  pieces,
  busy,
  error,
  onAdopt,
  onAdoptAll,
  onDecline,
  onRevisit,
}: BroughtByPlayersViewProps) {
  const groups = groupByPlayer(pieces);
  return (
    <section className="space-y-6" aria-label="Brought by players">
      <p className="text-sm text-muted-foreground">
        Pieces players brought in on their sheets stay off the table until you
        adopt them into this world.
      </p>
      {error ? (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      ) : null}
      {groups.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          Nobody has brought anything in yet.
        </p>
      ) : null}
      {groups.map((group) => (
        <div
          key={group.player.id}
          className="space-y-2"
          data-testid="staged-player"
          data-player-id={group.player.id}
        >
          <div className="flex items-center justify-between gap-3">
            <h3 className="font-semibold text-foreground">
              {group.player.displayName}
            </h3>
            {group.pending > 1 ? (
              <Button
                size="sm"
                variant="secondary"
                disabled={busy}
                onClick={() => onAdoptAll(group.player)}
                aria-label={`Adopt all from ${group.player.displayName}`}
              >
                Adopt all ({group.pending})
              </Button>
            ) : null}
          </div>
          <ul className="space-y-2">
            {group.pieces.map((piece) => (
              <StagedRow
                key={piece.id}
                piece={piece}
                differsFromPlayer={differsFromPlayer(piece, pieces)}
                busy={busy}
                onAdopt={onAdopt}
                onDecline={onDecline}
                onRevisit={onRevisit}
              />
            ))}
          </ul>
        </div>
      ))}
    </section>
  );
}

export interface BroughtByPlayersProps {
  worldId: string;
}

/**
 * Spec 048 US3: the Game Master's and Trusted Players' queue of what
 * players brought in. Every decision is a mutation; the list is fetched
 * again after each one, so it always shows what the server holds.
 */
export function BroughtByPlayers({ worldId }: BroughtByPlayersProps) {
  const [pieces, setPieces] = useState<StagedPiece[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refetch = useCallback(
    () =>
      listStagedContent(worldId).then(setPieces, (cause: unknown) => {
        setPieces((current) => current ?? []);
        setError(cause instanceof Error ? cause.message : String(cause));
      }),
    [worldId],
  );

  useEffect(() => {
    void refetch();
  }, [refetch]);

  const decide = (action: () => Promise<unknown>) => {
    setBusy(true);
    setError(null);
    action()
      .catch((cause: unknown) =>
        setError(cause instanceof Error ? cause.message : String(cause)),
      )
      .then(refetch)
      .finally(() => setBusy(false));
  };

  if (pieces === null) return <Loader />;
  return (
    <BroughtByPlayersView
      pieces={pieces}
      busy={busy}
      error={error}
      onAdopt={(piece) => decide(() => adoptStagedContent(piece.id))}
      onAdoptAll={(player) =>
        decide(() => adoptAllStagedContent(worldId, player.id))
      }
      onDecline={(piece) => decide(() => declineStagedContent(piece.id))}
      onRevisit={(piece) =>
        decide(() => revisitStagedContent(piece.id, "PENDING"))
      }
    />
  );
}
