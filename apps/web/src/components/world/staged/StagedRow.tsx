import type { StagedPiece } from "@/api/stagedContent";
import { Button } from "@/components/ui/button/Button";

export interface StagedRowProps {
  piece: StagedPiece;
  /**
   * Who brought the piece this one differs from, when it differs from
   * another character's of the same name (FR-036).
   */
  differsFromPlayer: string | null;
  busy: boolean;
  onAdopt: (piece: StagedPiece) => void;
  onDecline: (piece: StagedPiece) => void;
  onRevisit: (piece: StagedPiece) => void;
}

const STATE_LABEL: Record<StagedPiece["state"], string> = {
  PENDING: "Waiting",
  ACCEPTED: "Adopted",
  DECLINED: "Declined",
};

/**
 * One piece a player brought in on a sheet: its name, what kind of thing it
 * is, which characters carry it, and what the person may do with it now.
 * A pending piece is adopted or declined; a declined one can be revisited;
 * an adopted one is the world's and needs nothing more here.
 */
export function StagedRow({
  piece,
  differsFromPlayer,
  busy,
  onAdopt,
  onDecline,
  onRevisit,
}: StagedRowProps) {
  const carriers = piece.actors.map((actor) => actor.label).join(", ");
  return (
    <li
      className="flex flex-wrap items-start justify-between gap-3 rounded-md border border-border p-3"
      data-testid="staged-row"
      data-staged-id={piece.id}
      data-state={piece.state}
    >
      <div className="min-w-0 space-y-1">
        <p className="font-medium text-foreground">
          {piece.name}{" "}
          <span className="text-xs font-normal text-muted-foreground">
            {piece.kind} · {STATE_LABEL[piece.state]}
          </span>
        </p>
        {carriers ? (
          <p className="text-xs text-muted-foreground">On {carriers}</p>
        ) : null}
        {differsFromPlayer ? (
          <p className="text-xs text-amber-600" data-testid="staged-differs">
            Differs from the {piece.name} {differsFromPlayer} brought.
          </p>
        ) : null}
      </div>
      <div className="flex shrink-0 gap-2">
        {piece.state === "PENDING" ? (
          <>
            <Button
              size="sm"
              disabled={busy}
              onClick={() => onAdopt(piece)}
              aria-label={`Adopt ${piece.name}`}
            >
              Adopt
            </Button>
            <Button
              size="sm"
              variant="ghost"
              disabled={busy}
              onClick={() => onDecline(piece)}
              aria-label={`Decline ${piece.name}`}
            >
              Decline
            </Button>
          </>
        ) : null}
        {piece.state === "DECLINED" ? (
          <Button
            size="sm"
            variant="secondary"
            disabled={busy}
            onClick={() => onRevisit(piece)}
            aria-label={`Revisit ${piece.name}`}
          >
            Revisit
          </Button>
        ) : null}
      </div>
    </li>
  );
}
