/**
 * Moves this client has sent, so a read of the scene older than one of them
 * does not put the token back where it was (spec 045 T065).
 *
 * # The race
 *
 * Every token change the server records comes back as event 14, and the
 * client answers each one by re-reading the scene's tokens. The read is a
 * round trip of its own, and nothing orders it against the next move this
 * player makes. So a player who steps, sees the step land, and steps again
 * can have the *first* step's re-read arrive after the *second* step left:
 * the read carries the first position, the store takes it, the engine draws
 * the token a cell back — and the next key press is measured from there. The
 * server then accepts a move to a square the token already stood on, and
 * the walk has lost a step the player saw it take.
 *
 * # The rule
 *
 * A read's position for a token is stale when a move of that token was still
 * unanswered when the read was sent, or is unanswered now. Only this client's
 * own moves are counted, because only for those does this client already hold
 * a newer position than the server's reply can: everybody else's moves are
 * known here only through the very reads being judged.
 *
 * A stale read still says everything else — a name, an owner, a sheet — and
 * only its position is passed over. The server stores exactly the position a
 * move sends, so the position kept is the one the server will hold once the
 * move lands; a refused move is settled before it is read back, so its
 * read-back is never stale and does put the token back.
 */

let settledClock = 0;
const unanswered = new Map<string, number>();
const lastSettled = new Map<string, number>();

/** A mark to take before sending a read, for `isPositionStale`. */
export function markRead(): number {
  return settledClock;
}

/**
 * Note that a move of `tokenId` has been sent. Call the returned function
 * once it has been answered, accepted or refused, and before anything reads
 * the token back because of it.
 */
export function beginLocalMove(tokenId: string): () => void {
  unanswered.set(tokenId, (unanswered.get(tokenId) ?? 0) + 1);
  let settled = false;
  return () => {
    if (settled) return;
    settled = true;
    const left = (unanswered.get(tokenId) ?? 1) - 1;
    if (left > 0) {
      unanswered.set(tokenId, left);
    } else {
      unanswered.delete(tokenId);
    }
    settledClock += 1;
    lastSettled.set(tokenId, settledClock);
  };
}

/** Whether a read marked `mark` is older than a move of `tokenId` from here. */
export function isPositionStale(tokenId: string, mark: number): boolean {
  return (
    (unanswered.get(tokenId) ?? 0) > 0 || (lastSettled.get(tokenId) ?? 0) > mark
  );
}

/** For tests: forget every move. */
export function resetLocalMovesForTests(): void {
  settledClock = 0;
  unanswered.clear();
  lastSettled.clear();
}
