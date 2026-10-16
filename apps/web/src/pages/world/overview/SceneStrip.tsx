import { useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { launchScene } from "@/api/scenes";
import type { SceneHeadline } from "@/api/worldStatistics";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { FullMapCredit } from "@/components/world/MapCredit";
import { pictureOf, scenesForStrip } from "@/pages/world/overview/sceneStrip";

/**
 * Spec 100 US3.1, "where we start": the world's scenes as cards with their
 * maps, the table's scene first.
 *
 * A Game Master's "Play here" is the Scenes section's Launch (spec 022
 * FR-002a, ADR-046): it sets the world's active scene for everyone, then
 * opens the table — the same two steps, in the same order, so arriving at a
 * table whose scene did not change cannot happen. Players get the cards
 * without the action; the scenes query never sends them a hidden scene.
 *
 * The testids are the dashboard list's this replaced, so its coverage
 * (five cards, not forty, and a link to the rest) still holds.
 */
export function SceneStrip({
  worldId,
  isGm,
  activeSceneId,
  scenes,
}: {
  worldId: string;
  isGm: boolean;
  activeSceneId: string | null;
  /** `null` while loading; the overview reads them to name Play's scene. */
  scenes: SceneHeadline[] | "failed" | null;
}) {
  const navigate = useNavigate();
  const [launching, setLaunching] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const all = scenes === "failed" ? null : scenes;
  const cards = all ? scenesForStrip(all, activeSceneId) : null;

  const playHere = async (sceneId: string) => {
    setLaunching(sceneId);
    setError(null);
    try {
      await launchScene(worldId, sceneId);
      navigate(`/world/${worldId}/play`);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to launch scene");
      setLaunching(null);
    }
  };

  return (
    <Card
      surface="parchment"
      className="grid gap-4 p-5"
      role="region"
      aria-labelledby="scene-strip-heading"
      data-testid="recent-scenes"
    >
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h2 id="scene-strip-heading" className="text-xl font-semibold">
          Scenes
        </h2>
        <Link
          to={`/world/${worldId}/scenes`}
          className="text-sm text-primary underline-offset-4 hover:underline"
          data-testid="recent-scenes-all-link"
        >
          {all
            ? `All ${all.length} scene${all.length === 1 ? "" : "s"}`
            : "All scenes"}
        </Link>
      </div>

      {error ? (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      ) : null}

      {scenes === null ? (
        <p className="text-sm text-muted-foreground">Loading…</p>
      ) : cards === null ? (
        <p className="text-sm text-muted-foreground">
          The scenes could not be read.
        </p>
      ) : cards.length === 0 ? (
        <div
          className="grid justify-items-start gap-2"
          data-testid="scene-strip-empty"
        >
          <p className="text-sm text-muted-foreground">
            {isGm
              ? "No scenes yet. A scene is a map the table plays on."
              : "No scene has been shown to the players yet."}
          </p>
          {isGm ? (
            <Button asChild variant="secondary" size="sm" icon="map">
              <Link to={`/world/${worldId}/scenes`}>Create a scene</Link>
            </Button>
          ) : null}
        </div>
      ) : (
        <ol className="grid grid-cols-[repeat(auto-fill,minmax(min(16rem,100%),1fr))] gap-4">
          {cards.map((scene) => {
            const isActive = scene.sceneId === activeSceneId;
            const picture = pictureOf(scene);
            return (
              <li
                key={scene.sceneId}
                className="grid min-w-0 content-start gap-2 overflow-hidden rounded-lg border border-border bg-background/60"
                data-testid="recent-scene"
                data-active={isActive ? "true" : undefined}
              >
                <Link
                  to={`/world/${worldId}/scenes/${scene.sceneId}`}
                  className="block aspect-video bg-muted"
                  // The name below says which scene; the picture is a
                  // second way to the same page, not a second label.
                  tabIndex={-1}
                  aria-hidden="true"
                >
                  {picture ? (
                    <img
                      src={picture}
                      alt=""
                      loading="lazy"
                      className="size-full object-cover"
                    />
                  ) : (
                    <span className="grid size-full place-items-center text-xs text-foreground">
                      No map
                    </span>
                  )}
                </Link>
                <div className="grid gap-1 px-3 pb-3">
                  <div className="flex flex-wrap items-baseline gap-x-2 gap-y-1">
                    <Link
                      to={`/world/${worldId}/scenes/${scene.sceneId}`}
                      className="min-w-0 font-medium break-words hover:underline"
                    >
                      {scene.name}
                    </Link>
                    {isActive ? <Badge>At the table</Badge> : null}
                    {scene.hidden ? (
                      // Only a Game Master is sent hidden scenes.
                      <Badge variant="secondary">Hidden from players</Badge>
                    ) : null}
                  </div>
                  <span className="text-xs text-muted-foreground">
                    Updated{" "}
                    <time dateTime={scene.updatedAt}>
                      {new Date(scene.updatedAt).toLocaleDateString()}
                    </time>
                  </span>
                  {scene.backgroundCredit ? (
                    <FullMapCredit
                      credit={scene.backgroundCredit}
                      className="text-xs text-muted-foreground"
                    />
                  ) : null}
                  {isGm ? (
                    isActive ? (
                      <Button
                        asChild
                        variant="secondary"
                        size="sm"
                        icon="spark"
                        className="justify-self-start"
                      >
                        <Link
                          to={`/world/${worldId}/play`}
                          data-testid="scene-play-here"
                        >
                          Play
                        </Link>
                      </Button>
                    ) : (
                      <Button
                        type="button"
                        variant="secondary"
                        size="sm"
                        icon="spark"
                        className="justify-self-start"
                        disabled={launching !== null}
                        onClick={() => void playHere(scene.sceneId)}
                        data-testid="scene-play-here"
                      >
                        {launching === scene.sceneId
                          ? "Launching..."
                          : "Play here"}
                      </Button>
                    )
                  ) : null}
                </div>
              </li>
            );
          })}
        </ol>
      )}
    </Card>
  );
}
