import { useCallback, useEffect, useState } from "react";
import {
  addCombatant,
  advanceTurn,
  endCombat,
  getActiveCombat,
  removeCombatant,
  startCombat,
  updateCombatant,
} from "@/api/combat";
import { getWorldActors } from "@/api/actors";
import { Button } from "@/components/ui/button/Button";
import {
  subscribeToWorldEvents,
  startPlayPanelEventSync,
} from "@/engine/world/sync";
import { cn } from "@/lib/utils";
import type { CombatRecord } from "@/types/combat";
import type { WorldActorRecord } from "@/types/actor";

export interface CombatPanelProps {
  worldId: string;
  sceneId: string | null;
  isGm: boolean;
}

/**
 * The shared initiative tracker.
 *
 * Shared is the whole point, so this component holds no turn-order state of
 * its own: every mutation returns the authoritative combat and the
 * `world_events` subscription (code 18) refetches it whenever anyone else
 * changes it. The list is rendered in exactly the order the server sends —
 * sorting here would be a second ordering rule that could disagree with the
 * server's, which is the bug a shared tracker exists to prevent.
 *
 * Players get a read-only view; every control is GM-gated both here and,
 * authoritatively, in `mutations_combat.rs`.
 */
export function CombatPanel({ worldId, sceneId, isGm }: CombatPanelProps) {
  const [combat, setCombat] = useState<CombatRecord | null>(null);
  const [loading, setLoading] = useState(true);
  const [actors, setActors] = useState<WorldActorRecord[]>([]);
  const [addActorId, setAddActorId] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(() => {
    return getActiveCombat(worldId)
      .then(setCombat)
      .catch((err) =>
        setError(err instanceof Error ? err.message : "Failed to load combat"),
      )
      .finally(() => setLoading(false));
  }, [worldId]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    const stop = startPlayPanelEventSync(
      { onCombatChanged: () => void refresh() },
      subscribeToWorldEvents(worldId),
    );
    return stop;
  }, [worldId, refresh]);

  useEffect(() => {
    if (!isGm) return;
    getWorldActors(worldId)
      .then(setActors)
      .catch(() => setActors([]));
  }, [worldId, isGm]);

  /** Runs a mutation, adopts its authoritative result, and surfaces failures. */
  const run = async (action: () => Promise<CombatRecord>) => {
    setBusy(true);
    setError(null);
    try {
      setCombat(await action());
    } catch (err) {
      setError(err instanceof Error ? err.message : "Combat action failed");
    } finally {
      setBusy(false);
    }
  };

  if (loading) {
    return <p className="text-sm text-muted-foreground">Loading combat…</p>;
  }

  if (!combat || combat.endedAt) {
    return (
      <div className="grid gap-3" data-testid="combat-panel">
        <p className="text-sm text-muted-foreground">No combat in progress.</p>
        {error ? <p className="text-sm text-destructive">{error}</p> : null}
        {isGm ? (
          <Button
            type="button"
            size="sm"
            disabled={busy}
            data-testid="start-combat-button"
            onClick={() => void run(() => startCombat(worldId, sceneId))}
          >
            Start combat
          </Button>
        ) : null}
      </div>
    );
  }

  const addableActors = actors.filter(
    (actor) =>
      !combat.combatants.some((combatant) => combatant.actorId === actor.id),
  );

  return (
    <div className="grid gap-3" data-testid="combat-panel">
      <div className="flex items-center justify-between gap-2">
        <span className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
          Round {combat.round}
        </span>
        {isGm ? (
          <div className="flex gap-2">
            <Button
              type="button"
              size="sm"
              disabled={busy || combat.combatants.length === 0}
              data-testid="advance-turn-button"
              onClick={() => void run(() => advanceTurn(combat.id))}
            >
              Next turn
            </Button>
            <Button
              type="button"
              size="sm"
              variant="secondary"
              disabled={busy}
              data-testid="end-combat-button"
              onClick={() => void run(() => endCombat(combat.id))}
            >
              End
            </Button>
          </div>
        ) : null}
      </div>

      {error ? <p className="text-sm text-destructive">{error}</p> : null}

      {combat.combatants.length === 0 ? (
        <p className="text-sm text-muted-foreground">No combatants yet.</p>
      ) : (
        <ul className="grid gap-1" data-testid="combatant-list">
          {combat.combatants.map((combatant) => {
            const isTurn = combatant.id === combat.activeCombatantId;
            return (
              <li
                key={combatant.id}
                data-testid="combatant-row"
                data-active-turn={isTurn ? "true" : "false"}
                className={cn(
                  "flex items-center gap-2 rounded-lg border px-2 py-1.5",
                  isTurn ? "border-primary bg-primary/10" : "border-border",
                  !combatant.active && "opacity-50",
                )}
              >
                {isGm ? (
                  <input
                    type="number"
                    value={combatant.initiative}
                    aria-label={`Initiative for ${combatant.label}`}
                    className="h-7 w-12 rounded border border-input bg-transparent px-1 text-sm tabular-nums outline-none"
                    onChange={(event) => {
                      const initiative = Number.parseInt(
                        event.target.value,
                        10,
                      );
                      if (!Number.isFinite(initiative)) return;
                      void run(() =>
                        updateCombatant({
                          combatantId: combatant.id,
                          initiative,
                        }),
                      );
                    }}
                  />
                ) : (
                  <span className="w-8 text-sm tabular-nums">
                    {combatant.initiative}
                  </span>
                )}

                <span className="min-w-0 flex-1 truncate text-sm">
                  {combatant.label}
                  {combatant.isNpc ? (
                    <span className="ml-1 text-xs text-muted-foreground">
                      NPC
                    </span>
                  ) : null}
                </span>

                {isGm ? (
                  <>
                    <button
                      type="button"
                      title={combatant.active ? "Mark down" : "Revive"}
                      aria-label={
                        combatant.active
                          ? `Mark ${combatant.label} down`
                          : `Revive ${combatant.label}`
                      }
                      disabled={busy}
                      className="rounded px-1.5 py-0.5 text-xs text-muted-foreground hover:bg-muted hover:text-foreground"
                      onClick={() =>
                        void run(() =>
                          updateCombatant({
                            combatantId: combatant.id,
                            active: !combatant.active,
                          }),
                        )
                      }
                    >
                      {combatant.active ? "Down" : "Up"}
                    </button>
                    <button
                      type="button"
                      aria-label={`Remove ${combatant.label}`}
                      disabled={busy}
                      className="rounded px-1.5 py-0.5 text-xs text-muted-foreground hover:bg-muted hover:text-destructive"
                      onClick={() =>
                        void run(() => removeCombatant(combatant.id))
                      }
                    >
                      ✕
                    </button>
                  </>
                ) : null}
              </li>
            );
          })}
        </ul>
      )}

      {isGm ? (
        <div className="grid gap-2 border-t border-border pt-3">
          <label
            htmlFor="combat-add-actor"
            className="text-xs font-semibold tracking-widest text-muted-foreground uppercase"
          >
            Add combatant
          </label>
          <div className="flex gap-2">
            <select
              id="combat-add-actor"
              value={addActorId}
              onChange={(event) => setAddActorId(event.target.value)}
              data-testid="combat-add-actor-select"
              className="h-9 min-w-0 flex-1 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none"
            >
              <option value="">Select an actor…</option>
              {addableActors.map((actor) => (
                <option key={actor.id} value={actor.id}>
                  {actor.label}
                </option>
              ))}
            </select>
            <Button
              type="button"
              size="sm"
              disabled={busy || !addActorId}
              data-testid="combat-add-button"
              onClick={() => {
                const actor = actors.find(
                  (candidate) => candidate.id === addActorId,
                );
                if (!actor) return;
                void run(() =>
                  addCombatant({
                    combatId: combat.id,
                    label: actor.label,
                    actorId: actor.id,
                    isNpc: actor.isNpc,
                  }),
                ).then(() => setAddActorId(""));
              }}
            >
              Add
            </Button>
          </div>
        </div>
      ) : null}
    </div>
  );
}
