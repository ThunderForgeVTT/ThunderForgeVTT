import { createElement, useEffect, useId, useState } from "react";
import { getGameSystemManifest } from "@/api/gameSystems";
import { resolvePanel } from "@/panels/systemPanels";
import { ActiveSystemCard } from "@/pages/world/settings/ActiveSystemCard";
import { PlayPauseHistoryCard } from "@/pages/world/settings/PlayPauseHistoryCard";
import { WorldSystemSettingsCard } from "@/pages/world/settings/WorldSystemSettingsCard";
import type { SystemManifest } from "@/types/systemManifest";
import type { WorldRecord } from "@/types/world";

export interface AboutThisTableProps {
  world: WorldRecord;
}

/**
 * What a Player may read about their table: the rules it runs on and their
 * licence (spec 016 FR-005), the settings the system leaves to the table,
 * read-only (spec 067 FR-005), and when play was paused (spec 051 FR-050).
 *
 * # Why it is here
 *
 * All three used to be read on the world's settings page, which a Player now
 * meets as a GIF — the owner's decision, since nothing there is theirs to
 * change. They still have to be readable somewhere a Player can always get
 * to. That is the Overview rather than the dashboard: the dashboard sends a
 * member with no character to Actor Selection first (spec 017), and a
 * licence notice must not wait on a member choosing a character.
 *
 * Every card below is the one the Game Master's settings page draws, with
 * `isGm` false, so a Player reads the same thing their Game Master set and a
 * fix to either is a fix to both. Each renders nothing when it has nothing
 * to say, as on that page.
 */
export function AboutThisTable({ world }: AboutThisTableProps) {
  const headingId = useId();
  const systemId = world.gameSystemId ?? null;
  const [loaded, setLoaded] = useState<{
    systemId: string;
    manifest: SystemManifest | null;
  } | null>(null);

  useEffect(() => {
    if (!systemId) return;
    let active = true;
    getGameSystemManifest(systemId)
      .then((manifest) => {
        if (active) setLoaded({ systemId, manifest });
      })
      .catch(() => {
        if (active) setLoaded({ systemId, manifest: null });
      });
    return () => {
      active = false;
    };
  }, [systemId]);

  const manifest = loaded?.systemId === systemId ? loaded.manifest : null;
  // A system the world runs whose manifest has not arrived (or failed to) is
  // not "no system assigned"; say nothing rather than something untrue.
  const showSystem = !systemId || manifest !== null;
  const panel = resolvePanel(systemId, "world-settings");

  return (
    <section
      className="grid gap-4"
      aria-labelledby={headingId}
      data-testid="about-this-table"
    >
      <div className="grid gap-1">
        <h2 id={headingId} className="text-lg font-semibold">
          About this table
        </h2>
        <p className="max-w-prose text-sm text-muted-foreground">
          The rules this world plays by, the licence they come with, and the
          choices your Game Master has made.
        </p>
      </div>
      <div className="grid items-start gap-4">
        {showSystem ? <ActiveSystemCard manifest={manifest} /> : null}
        {systemId ? (
          <WorldSystemSettingsCard worldId={world.id} isGm={false} />
        ) : null}
        {panel
          ? createElement(panel, {
              worldId: world.id,
              world,
              isGm: false,
              onWorldChanged: () => {},
            })
          : null}
        <PlayPauseHistoryCard worldId={world.id} />
      </div>
    </section>
  );
}
