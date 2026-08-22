import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { getWorldActors } from "@/api/actors";
import type { WorldActorRecord } from "@/types/actor";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";

export interface NpcRosterProps {
  worldId: string;
  /** Bump this to force a re-fetch (e.g. after creating a new NPC, spec 010). */
  refreshKey?: number;
}

/**
 * Spec 009 (T008): the world's NPC roster — real `world_actors` rows
 * (`isNpc === true`), or a genuine empty state (FR-004: never
 * placeholder/lorem-ipsum text). Shared by the staging page and the
 * full-screen sidebar's NPC/combat section.
 */
export function NpcRoster({ worldId, refreshKey }: NpcRosterProps) {
  const [actors, setActors] = useState<WorldActorRecord[] | null>(null);
  const [error, setError] = useState<Error | null>(null);

  useEffect(() => {
    let active = true;
    setActors(null);
    setError(null);

    getWorldActors(worldId)
      .then((result) => {
        if (active) {
          setActors(result);
        }
      })
      .catch((err) => {
        if (active) {
          setError(err instanceof Error ? err : new Error(String(err)));
        }
      });

    return () => {
      active = false;
    };
  }, [worldId, refreshKey]);

  if (error) {
    return (
      <p className="text-sm text-destructive">
        Failed to load NPCs: {error.message}
      </p>
    );
  }

  if (actors === null) {
    return <p className="text-sm text-muted-foreground">Loading NPCs…</p>;
  }

  const npcs = actors.filter((actor) => actor.isNpc);

  if (npcs.length === 0) {
    return <p className="text-sm text-muted-foreground">No NPCs yet.</p>;
  }

  return (
    <ul className="grid gap-2" data-testid="npc-roster-list">
      {npcs.map((npc) => (
        <li key={npc.id}>
          <Link
            to={`/world/${worldId}/actor/${npc.id}/view`}
            className="flex items-center gap-2 rounded-lg border border-border p-2 hover:bg-muted"
          >
          <FantasyIcon name="skull" size={16} />
          <div>
            <strong className="block text-sm">{npc.label}</strong>
            <small className="text-xs text-muted-foreground uppercase tracking-wide">
              {npc.actorType}
            </small>
          </div>
          </Link>
        </li>
      ))}
    </ul>
  );
}
