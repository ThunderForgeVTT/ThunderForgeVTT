import { useEffect, useRef, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { rearmPlayPaused } from "@/api/playPauseSignal";
import { getWorldPlayState, type WorldPlayState } from "@/api/playPause";
import { getWorld } from "@/api/world";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import type { SeoConfig } from "@/types/seo";

export const playPausedPageSeo: SeoConfig = {
  title: "Play is paused",
  description: "Play in this world has been paused by an operator.",
  canonicalPath: "/world",
  noindex: true,
};

/**
 * How often to ask again. No stream is open while a world is paused, so a lift
 * cannot be pushed here; a small read on this interval is the honest
 * substitute (research R6).
 */
export const PLAY_STATE_POLL_MS = 30_000;

/** A moment as the reader writes it: the date and the time of day. */
function formatMoment(iso: string): string {
  return new Date(iso).toLocaleString(undefined, {
    dateStyle: "long",
    timeStyle: "short",
  });
}

/**
 * Spec 051 US1: where a table goes when an operator pauses its world's play
 * (FR-010 to FR-013, contracts/live-play-lock.md "The notice").
 *
 * # What it says, and what it never says
 *
 * *That* play was paused, by an operator of this instance, and *when*. Never
 * why, never who, and no word that reads as blame: most people at a paused
 * table have done nothing wrong, and a reason shown to the table could tip off
 * whoever is being looked into. The page could not say more if it tried —
 * `worldPlayState` has no field a reason could be put in.
 *
 * # Read across a room
 *
 * A shared screen at an in-person table is a first-class place for this to
 * appear, so the type is large, the actions are large, and the one sentence
 * that matters is on its own. The heading takes focus on arrival so a screen
 * reader starts where a sighted reader does, and the status is a polite live
 * region so a change (play resuming) is announced without stealing focus.
 */
export default function PlayPausedPage() {
  const { id = "" } = useParams();
  const headingRef = useRef<HTMLHeadingElement>(null);
  const [worldName, setWorldName] = useState<string | null>(null);
  const [playState, setPlayState] = useState<WorldPlayState | null>(null);
  const [unreadable, setUnreadable] = useState(false);

  useEffect(() => {
    headingRef.current?.focus();
  }, []);

  // Leaving the notice re-arms the signal, so meeting this pause again later
  // (Back into the playfield, or another visit) comes here again.
  useEffect(() => {
    if (!id) return;
    return () => rearmPlayPaused(id);
  }, [id]);

  useEffect(() => {
    if (!id) return;
    let active = true;
    // An unpaused read, not a gated one: the world's name is still the
    // world's name while its play is paused.
    void getWorld(id)
      .then((world) => {
        if (active) setWorldName(world?.name ?? null);
      })
      .catch(() => undefined);
    return () => {
      active = false;
    };
  }, [id]);

  useEffect(() => {
    if (!id) return;
    let active = true;
    const ask = () => {
      getWorldPlayState(id)
        .then((next) => {
          if (!active) return;
          setPlayState(next);
          setUnreadable(false);
        })
        .catch(() => {
          // Keep whatever was last known. A failed read is not news about the
          // pause, and the next poll asks again.
          if (active) setUnreadable(true);
        });
    };
    ask();
    const timer = setInterval(ask, PLAY_STATE_POLL_MS);
    window.addEventListener("focus", ask);
    return () => {
      active = false;
      clearInterval(timer);
      window.removeEventListener("focus", ask);
    };
  }, [id]);

  const resumed = playState !== null && !playState.paused;
  const world = worldName ?? "this world";

  return (
    <>
      <SEO {...playPausedPageSeo} canonicalPath={`/world/${id}/paused`} />
      <Container narrow className="py-10 sm:py-16">
        <Card
          className="grid gap-8 p-8 sm:p-12"
          data-testid="play-paused-notice"
        >
          <h1
            ref={headingRef}
            tabIndex={-1}
            className="text-4xl leading-tight font-semibold outline-none sm:text-5xl"
          >
            {resumed ? "Play has resumed" : "Play is paused"}
          </h1>

          <div
            role="status"
            aria-live="polite"
            className="grid gap-4 text-xl leading-relaxed sm:text-2xl"
            data-testid="play-paused-status"
          >
            {resumed ? (
              <p>
                Play in <strong>{world}</strong> is no longer paused. You can
                return to the world.
              </p>
            ) : (
              <p>
                Play in <strong>{world}</strong> has been paused by an operator
                of this instance
                {playState?.pausedAt ? (
                  <>
                    , since{" "}
                    <time
                      dateTime={playState.pausedAt}
                      data-testid="play-paused-since"
                    >
                      {formatMoment(playState.pausedAt)}
                    </time>
                  </>
                ) : null}
                .
              </p>
            )}
          </div>

          <div className="grid gap-4">
            {resumed ? null : (
              <p className="text-lg leading-relaxed text-muted-foreground sm:text-xl">
                You can go to your worlds in the meantime. This page will notice
                when play resumes, so there is no need to reload it.
              </p>
            )}
            {unreadable && !resumed ? (
              <p className="text-base text-muted-foreground">
                This page could not check just now, and will try again shortly.
              </p>
            ) : null}
            <div className="flex flex-wrap gap-4">
              {resumed ? (
                <Button asChild size="lg" icon="worlds">
                  <Link to={`/world/${id}/play`}>Return to the world</Link>
                </Button>
              ) : null}
              <Button
                asChild
                size="lg"
                variant={resumed ? "secondary" : "primary"}
                icon="compass"
              >
                <Link to="/worlds">Go to your worlds</Link>
              </Button>
            </div>
          </div>
        </Card>
      </Container>
    </>
  );
}
