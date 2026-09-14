import { useWorldPlayState } from "@/hooks/useWorldPlayState";
import { formatPauseMoment, pausedSince } from "@/pages/world/playPauseStatus";

export interface PlayPausedBannerProps {
  worldId: string;
}

/**
 * Spec 051 US5 (FR-050): a quiet line at the top of a world's pages while an
 * operator has paused its play.
 *
 * It says *that* and *when*, and nothing else. A Game Master needs to know
 * their table was not broken by a bug; they must not be told the reason, and
 * the state this reads has no field one could come from.
 *
 * Quiet, not alarming: a muted surface in the incumbent tokens, no warning
 * colour and no icon of danger, because most people at a paused table have
 * done nothing wrong. Large enough to read on a shared screen across a room.
 * Both lines are in the foreground colour: the muted text token does not
 * reach 4.5:1 on the muted surface in either theme, so the second line is set
 * apart by weight and size instead. It holds no control, so focus order is
 * untouched, and it is a polite status so it is announced without taking
 * focus.
 */
export function PlayPausedBanner({ worldId }: PlayPausedBannerProps) {
  const since = pausedSince(useWorldPlayState(worldId));
  if (!since) return null;

  return (
    <div
      role="status"
      data-testid="world-play-paused-banner"
      className="grid gap-1 rounded-xl border border-border bg-muted px-5 py-4 text-foreground sm:px-6"
    >
      <p className="text-lg leading-snug font-semibold text-balance sm:text-xl">
        Play in this world has been paused by an operator since{" "}
        <time dateTime={since}>{formatPauseMoment(since)}</time>.
      </p>
      <p className="text-base leading-relaxed">
        Play can start again once an operator lifts the pause.
      </p>
    </div>
  );
}
