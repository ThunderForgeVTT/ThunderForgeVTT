import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import {
  getSceneHeadlines,
  getWorldStatistics,
  type SceneHeadline,
  type WorldStatistics,
} from "@/api/worldStatistics";
import { Card } from "@/components/ui/card/Card";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import type { FantasyIconName } from "@/components/ui/fantasy-icon/FantasyIcon";

/** How many recently touched scenes the dashboard names. */
export const RECENT_SCENE_COUNT = 5;

interface Figure {
  key: string;
  label: string;
  value: number;
  detail?: string;
  icon: FantasyIconName;
}

/**
 * A world by its figures, and the few scenes most recently worked on.
 *
 * # Why figures and not the scene list
 *
 * The owner: "imagine 40 scenes we've been running for five years. Showing
 * all the scenes would be chaotic. Actual statistics for the GM would be
 * really good." The dashboard used to render every scene as a bullet. Now it
 * leads with numbers — how big the world is, who is at the table — and names
 * only the handful of scenes someone touched last, with a link to the rest.
 *
 * # Figures are the viewer's, and a player gets fewer
 *
 * `worldStatistics` counts for the person asking. NPCs come back `null` for a
 * player and the figure is simply not drawn: no "—", no "hidden", nothing a
 * player could read as a clue that there is something to hide. The same
 * goes for the encounter, which is only drawn while one is running.
 *
 * # Semantics
 *
 * A `<dl>`: each figure is a term and its value, which is what a screen
 * reader should announce ("Scenes, 40"), rather than a heading followed by an
 * unlabelled number.
 */
export function WorldAtAGlance({ worldId }: { worldId: string }) {
  const [stats, setStats] = useState<{
    worldId: string;
    value: WorldStatistics | null;
    failed: boolean;
  } | null>(null);
  const [scenes, setScenes] = useState<{
    worldId: string;
    value: SceneHeadline[];
  } | null>(null);

  useEffect(() => {
    let active = true;
    getWorldStatistics(worldId)
      .then((value) => {
        if (active) setStats({ worldId, value, failed: false });
      })
      .catch(() => {
        if (active) setStats({ worldId, value: null, failed: true });
      });
    getSceneHeadlines(worldId)
      .then((value) => {
        if (active) setScenes({ worldId, value });
      })
      .catch(() => {
        if (active) setScenes({ worldId, value: [] });
      });
    return () => {
      active = false;
    };
  }, [worldId]);

  const current = stats && stats.worldId === worldId ? stats : null;
  const figures = current?.value ? figuresFor(current.value) : null;
  const headlines = scenes && scenes.worldId === worldId ? scenes.value : null;
  const recent = headlines ? mostRecentlyUpdated(headlines) : null;
  const encounter = current?.value?.activeEncounter ?? null;

  return (
    <section
      className="grid gap-4"
      aria-labelledby="world-at-a-glance-heading"
      data-testid="world-at-a-glance"
    >
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <h2 id="world-at-a-glance-heading" className="text-xl font-semibold">
          At a glance
        </h2>
        {encounter ? (
          <p
            className="inline-flex items-center gap-2 rounded-full border border-primary/40 bg-primary/10 px-3 py-1 text-sm font-medium text-primary"
            data-testid="world-active-encounter"
          >
            <FantasyIcon name="skull" size={14} />
            Encounter running · round {encounter.round} · {encounter.combatants}{" "}
            combatant
            {encounter.combatants === 1 ? "" : "s"}
          </p>
        ) : null}
      </div>

      {current?.failed ? (
        <p className="text-sm text-muted-foreground">
          This world&apos;s figures could not be counted just now.
        </p>
      ) : (
        <dl
          className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-5"
          aria-busy={figures === null}
          data-testid="world-statistics"
        >
          {(figures ?? placeholderFigures()).map((figure) => (
            <div
              key={figure.key}
              className="grid gap-1 rounded-lg border border-border bg-card p-4"
              data-testid={`world-stat-${figure.key}`}
            >
              <dt className="inline-flex items-center gap-2 text-xs font-bold tracking-widest text-muted-foreground uppercase">
                <FantasyIcon name={figure.icon} size={14} />
                {figure.label}
              </dt>
              <dd className="text-3xl leading-none font-semibold tabular-nums">
                {figures ? figure.value : "…"}
              </dd>
              {figure.detail ? (
                <dd className="text-sm text-muted-foreground">
                  {figure.detail}
                </dd>
              ) : null}
            </div>
          ))}
        </dl>
      )}

      <Card
        surface="parchment"
        className="grid gap-3 p-5"
        data-testid="recent-scenes"
      >
        <div className="flex flex-wrap items-center justify-between gap-2">
          <h3 className="text-lg font-semibold">Recently updated scenes</h3>
          <Link
            to={`/world/${worldId}/scenes`}
            className="text-sm text-primary underline-offset-4 hover:underline"
            data-testid="recent-scenes-all-link"
          >
            {headlines
              ? `All ${headlines.length} scene${headlines.length === 1 ? "" : "s"}`
              : "All scenes"}
          </Link>
        </div>
        {recent === null ? (
          <p className="text-sm text-muted-foreground">Loading…</p>
        ) : recent.length === 0 ? (
          <p className="text-sm text-muted-foreground">No scenes yet.</p>
        ) : (
          <ol className="grid gap-1 text-sm">
            {recent.map((scene) => (
              <li
                key={scene.sceneId}
                className="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-0.5 border-b border-border py-1.5 last:border-b-0"
                data-testid="recent-scene"
              >
                <span className="font-medium">
                  {scene.name}
                  {scene.hidden ? (
                    // Only a Game Master is sent hidden scenes, so only a
                    // Game Master ever sees this.
                    <span className="ml-2 text-xs text-muted-foreground">
                      (hidden from players)
                    </span>
                  ) : null}
                </span>
                <span className="text-xs text-muted-foreground">
                  Updated{" "}
                  <time dateTime={scene.updatedAt}>
                    {new Date(scene.updatedAt).toLocaleDateString()}
                  </time>
                </span>
              </li>
            ))}
          </ol>
        )}
      </Card>
    </section>
  );
}

/**
 * The figures this viewer is told, in reading order.
 *
 * A figure the server withheld is left out rather than drawn empty.
 */
export function figuresFor(stats: WorldStatistics): Figure[] {
  const figures: Figure[] = [
    { key: "scenes", label: "Scenes", value: stats.scenes, icon: "map" },
    {
      key: "players",
      label: "Players",
      value: stats.members,
      detail: `${stats.membersWithCharacter} with a character`,
      icon: "actors",
    },
    {
      key: "characters",
      label: "Characters",
      value: stats.characters,
      icon: "quill",
    },
  ];
  if (stats.npcs !== null) {
    figures.push({
      key: "npcs",
      label: "NPCs",
      value: stats.npcs,
      icon: "skull",
    });
  }
  figures.push({
    key: "tokens",
    label: "Tokens",
    value: stats.tokens,
    icon: "worlds",
  });
  return figures;
}

/** Shapes to hold the layout while the counts arrive — never their values. */
function placeholderFigures(): Figure[] {
  return [
    { key: "scenes", label: "Scenes", value: 0, icon: "map" },
    { key: "players", label: "Players", value: 0, icon: "actors" },
    { key: "characters", label: "Characters", value: 0, icon: "quill" },
    { key: "tokens", label: "Tokens", value: 0, icon: "worlds" },
  ];
}

/** Newest first, the first few. Sorted here rather than trusting the order
 * the scenes query happens to return, which is not a contract. */
export function mostRecentlyUpdated(
  scenes: readonly SceneHeadline[],
): SceneHeadline[] {
  return [...scenes]
    .sort((a, b) => Date.parse(b.updatedAt) - Date.parse(a.updatedAt))
    .slice(0, RECENT_SCENE_COUNT);
}
