import type { UserSummary } from "@/api/sheetImport";
import type { StagedPiece } from "@/api/stagedContent";

export interface PlayerGroup {
  player: UserSummary;
  pieces: StagedPiece[];
  pending: number;
}

/**
 * One group per player, in the order they first appear; within a group,
 * pending pieces first, then by name.
 */
export function groupByPlayer(pieces: StagedPiece[]): PlayerGroup[] {
  const groups = new Map<string, PlayerGroup>();
  for (const piece of pieces) {
    const id = piece.broughtBy.id;
    const group = groups.get(id) ?? {
      player: piece.broughtBy,
      pieces: [],
      pending: 0,
    };
    group.pieces.push(piece);
    if (piece.state === "PENDING") group.pending += 1;
    groups.set(id, group);
  }
  const rank = (piece: StagedPiece) => (piece.state === "PENDING" ? 0 : 1);
  for (const group of groups.values()) {
    group.pieces.sort(
      (a, b) => rank(a) - rank(b) || a.name.localeCompare(b.name),
    );
  }
  return [...groups.values()];
}

/** Who brought the piece `piece` differs from, if it is in the list. */
export function differsFromPlayer(
  piece: StagedPiece,
  pieces: StagedPiece[],
): string | null {
  if (!piece.differsFrom) return null;
  const other = pieces.find((candidate) => candidate.id === piece.differsFrom);
  return other ? other.broughtBy.displayName : "another player";
}
