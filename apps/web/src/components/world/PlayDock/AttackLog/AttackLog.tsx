import { useCallback, useEffect, useState } from "react";
import { getAttack, getSceneAttacks } from "@/api/attacks";
import {
  startPlayPanelEventSync,
  subscribeToWorldEvents,
} from "@/engine/world/sync";
import { cn } from "@/lib/utils";
import type { AttackRecord } from "@/types/attack";
import { flagText, offerText, outcomeText, partiesText } from "./attackText";

/** How many of the scene's latest attacks the table sees at once. */
export const ATTACK_LOG_LENGTH = 5;

export interface AttackLogProps {
  worldId: string;
  sceneId: string | null;
}

/**
 * Every seat sees the scene's attacks (spec 046 FR-002).
 *
 * # Why it is not a dock section
 *
 * The dock mounts only its open section, and an attack has to reach a player
 * who is looking at their sheet, or at nothing. So this sits over the board
 * for everyone, always mounted, like the status panel.
 *
 * # What it is told
 *
 * World event 29 carries an attack's id and nothing else; this reads
 * `attack(id)`, which the server builds for this viewer. An attacker this
 * viewer cannot see arrives as "Unknown", with no ability named, and that is
 * what is shown — there is nothing else on this client to show instead. An
 * offer changing (event 30) re-reads the page, so "taken by the Game Master"
 * reaches every seat.
 */
export function AttackLog({ worldId, sceneId }: AttackLogProps) {
  const [attacks, setAttacks] = useState<AttackRecord[]>([]);

  const reload = useCallback(() => {
    if (!sceneId) return;
    getSceneAttacks(sceneId)
      .then((page) => setAttacks(page.slice(0, ATTACK_LOG_LENGTH)))
      // The log is a view of the table, not a control: failing to read it
      // costs this viewer the history, and the next event tries again.
      .catch(() => undefined);
  }, [sceneId]);

  useEffect(() => {
    reload();
  }, [reload]);

  useEffect(() => {
    return startPlayPanelEventSync(
      {
        onAttackMade: (attackId) => {
          if (!attackId) return;
          void getAttack(attackId)
            .then((attack) => {
              if (!attack || attack.sceneId !== sceneId) return;
              setAttacks((current) =>
                [attack, ...current.filter((a) => a.id !== attack.id)].slice(
                  0,
                  ATTACK_LOG_LENGTH,
                ),
              );
            })
            .catch(() => undefined);
        },
        onOfferChanged: () => reload(),
      },
      subscribeToWorldEvents(worldId),
    );
  }, [worldId, sceneId, reload]);

  // Another scene's attacks, still in hand while this scene's are read.
  const shown = attacks.filter((attack) => attack.sceneId === sceneId);
  if (shown.length === 0) return null;

  return (
    <section
      aria-label="Attacks"
      aria-live="polite"
      data-testid="attack-log"
      className="pointer-events-auto grid gap-1 rounded-lg border border-border bg-background/90 p-2 text-xs shadow-lg backdrop-blur"
    >
      <h2 className="text-[0.65rem] font-semibold tracking-widest text-muted-foreground uppercase">
        Attacks
      </h2>
      <ol className="grid gap-1">
        {shown.map((attack) => (
          <li
            key={attack.id}
            data-testid="attack-log-entry"
            data-attack-id={attack.id}
            data-outcome={attack.outcome}
            className="grid gap-0.5 rounded border border-border/60 px-2 py-1"
          >
            <span className="font-medium" data-testid="attack-log-parties">
              {partiesText(attack)}
              {attack.abilityName ? (
                <span className="text-muted-foreground">
                  {" "}
                  · {attack.abilityName}
                </span>
              ) : null}
            </span>
            <span
              data-testid="attack-log-outcome"
              className={cn(
                "tabular-nums",
                attack.outcome === "HIT" && "text-emerald-600",
                attack.outcome === "MISS" && "text-muted-foreground",
              )}
            >
              {outcomeText(attack)}
            </span>
            {attack.offer ? (
              <span data-testid="attack-log-offer">
                {offerText(attack.offer)}
              </span>
            ) : attack.damage ? (
              <span data-testid="attack-log-damage">
                {attack.damage.resultValue} damage
              </span>
            ) : null}
            {attack.flags.length > 0 ? (
              <span data-testid="attack-log-flags" className="text-amber-600">
                {attack.flags.map(flagText).join(", ")}
              </span>
            ) : null}
          </li>
        ))}
      </ol>
    </section>
  );
}
