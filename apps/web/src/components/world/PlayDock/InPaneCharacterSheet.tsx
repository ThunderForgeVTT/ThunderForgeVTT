import { createElement, useEffect, useMemo, useState } from "react";
import { getWorldAbilities } from "@/api/abilities";
import { getActorAbilities } from "@/api/actorAbilities";
import { rollDice } from "@/api/roll";
import { triggerDiceRollAnimation } from "@/engine/bevy";
import { RollResult } from "@/components/world/RollResult";
import { useActorSystemData } from "@/hooks/useActorSystemData";
import { resolveActorSheet } from "@/pages/world/actor/systemActorSheets";
import type { WorldAbilityRecord } from "@/types/ability";
import type { ActorAbilityEntryRecord } from "@/types/actorAbility";
import type { WorldActorRecord } from "@/types/actor";
import type { RollResolutionRecord } from "@/types/roll";
import { abilityRolls, statRolls, type CharacterRoll } from "./characterRolls";

/**
 * A player's own character, inside the dock, while the table stays live.
 *
 * # Why this exists when a perfectly good actor page already does
 *
 * Spec 031 US2. The actors pane used to link away for everyone, and for a
 * player that is the difference between playing and administering: making a
 * roll cost them the map, the engine tore down, and coming back meant a
 * reload. A Game Master keeps the new tab (FR-002) because they are inspecting
 * one of many characters beside the map; a player has exactly one character
 * and needs it *on* the map's screen. The two halves of View differ on
 * purpose.
 *
 * # Why it renders the system's own sheet
 *
 * `SYSTEM_ACTOR_SHEETS` is the same registry the full actor page mounts from,
 * so this is the sheet the active game system supplies, compacted — not a
 * second, parallel sheet (spec 031 Assumptions). Writing a dock-sized sheet by
 * hand was the obvious alternative and was rejected: it would drift from the
 * full page the first time a pack changed anything, and a player would be
 * looking at a different character to the one their GM sees.
 *
 * The sheet is mounted read-only. Editing is not forbidden in principle, but
 * a 22rem column during play is the wrong place to restat a character, and
 * `canEdit` there governs the same mutations the full page already offers.
 *
 * # Why the rolls are separate from the sheet
 *
 * Genie's `CharacterSheet` presents scores; it has no roll callback to hand
 * one to. Rather than reach into a pack to add one — which is spec 032's job,
 * not this one's — the rolls are derived alongside it in `characterRolls.ts`
 * from the same data the sheet is drawing, and go out through `rollDice`.
 *
 * Constitution Principle I: nothing here is canvas state. The dice animation
 * is a one-shot presentation trigger the engine already accepts from the dice
 * roller, and the roll itself is decided entirely by the server.
 */

export interface InPaneCharacterSheetProps {
  worldId: string;
  actor: WorldActorRecord;
  /** Returns the pane to whatever it was showing before (FR-002/US2 #3). */
  onDismiss: () => void;
}

export function InPaneCharacterSheet({
  worldId,
  actor,
  onDismiss,
}: InPaneCharacterSheetProps) {
  const sheet = resolveActorSheet(actor.gameSystemId);
  const { data } = useActorSystemData(
    actor.id,
    actor.gameSystemId ?? undefined,
  );
  const [entries, setEntries] = useState<ActorAbilityEntryRecord[] | null>(
    null,
  );
  const [catalog, setCatalog] = useState<WorldAbilityRecord[] | null>(null);
  const [rolling, setRolling] = useState<string | null>(null);
  const [result, setResult] = useState<RollResolutionRecord | null>(null);
  const [rollError, setRollError] = useState<string | null>(null);

  // Two reads because a formula lives on the ability, not on the actor's entry
  // for it. A failure on either leaves the ability rolls absent rather than
  // taking the whole view down: the stats still roll, and the sheet still
  // renders, which is most of why the player opened this.
  useEffect(() => {
    let active = true;
    Promise.all([getActorAbilities(actor.id), getWorldAbilities(worldId)])
      .then(([actorAbilities, worldAbilities]) => {
        if (active) {
          setEntries(actorAbilities);
          setCatalog(worldAbilities);
        }
      })
      .catch(() => {
        if (active) {
          setEntries([]);
          setCatalog([]);
        }
      });
    return () => {
      active = false;
    };
  }, [actor.id, worldId]);

  const rolls = useMemo<CharacterRoll[]>(
    () => [...statRolls(data?.ability_data), ...abilityRolls(entries, catalog)],
    [data?.ability_data, entries, catalog],
  );

  const handleRoll = async (roll: CharacterRoll) => {
    setRolling(roll.key);
    setRollError(null);
    setResult(null);
    try {
      const resolution = await rollDice(worldId, roll.formula);
      // Fire-and-forget, exactly as `DiceRollerPanel` does: the engine is
      // being asked to animate an outcome it played no part in deciding.
      void triggerDiceRollAnimation(
        resolution.dice.map((die) => ({ finalValue: die.finalValue })),
      );
      // Shown as soon as the server answers, rather than behind the dice
      // roller's fixed reveal delay. That delay is a copy of a duration in the
      // engine's Rust, and a second copy of it here would be a number to keep
      // in sync in two files; the dock sits beside the canvas rather than over
      // it, so the total appearing while the dice are still settling costs the
      // reveal nothing.
      setResult(resolution);
    } catch (error) {
      setRollError(
        error instanceof Error ? error.message : "Failed to roll dice",
      );
    } finally {
      setRolling(null);
    }
  };

  return (
    <div className="grid gap-3" data-testid="in-pane-character-sheet">
      <header className="flex items-center gap-2">
        <button
          type="button"
          onClick={onDismiss}
          data-testid="in-pane-sheet-dismiss"
          aria-label="Back to actors"
          className="rounded border border-border px-2 py-1 text-xs transition-colors hover:bg-muted"
        >
          ‹ Back
        </button>
        <span className="min-w-0 flex-1 truncate text-sm font-semibold">
          {actor.label}
        </span>
      </header>

      {/*
        The pack's sheet was drawn for a page, not a 22rem column. Scaling its
        type down and letting it scroll inside its own box keeps it legible
        here without every pack needing a second layout — and without this
        component knowing anything about what that sheet contains.
      */}
      {sheet ? (
        <div
          className="max-h-[45vh] overflow-y-auto rounded-lg border border-border text-xs [&_h1]:text-base [&_h2]:text-sm [&_h3]:text-sm"
          data-testid="in-pane-sheet-body"
        >
          {/*
            `createElement` rather than `<Sheet />` with a capitalised local:
            a component *value* chosen at render time is exactly what
            `react-hooks/static-components` exists to catch, and the rule is
            right in general — it just cannot see that this one comes from a
            module-level registry keyed by a string. Writing the call out
            keeps the rule enforced everywhere else rather than disabled here.
          */}
          {createElement(sheet, { actor, canEdit: false })}
        </div>
      ) : (
        /*
          FR-002 still holds when a system ships no sheet: the player stays in
          the pane and is told plainly why it is bare. Falling back to the Game
          Master's new tab was the tempting alternative and is exactly wrong —
          it would take the player away from the map for a page that has no
          more to show them than this does.
        */
        <p
          className="rounded-lg border border-dashed border-border px-2 py-3 text-xs text-muted-foreground"
          data-testid="in-pane-sheet-unavailable"
        >
          {actor.gameSystemId
            ? `${actor.gameSystemId} supplies no character sheet, so there is nothing to draw here.`
            : "This character belongs to no game system, so there is no sheet to draw."}{" "}
          Anything recorded against them can still be rolled below.
        </p>
      )}

      <section className="grid gap-1.5" data-testid="in-pane-sheet-rolls">
        <h3 className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
          Rolls
        </h3>
        {rolls.length === 0 ? (
          <p
            className="text-xs text-muted-foreground"
            data-testid="in-pane-sheet-no-rolls"
          >
            Nothing on this character carries a formula to roll yet.
          </p>
        ) : (
          <ul className="grid gap-1">
            {rolls.map((roll) => (
              <li key={roll.key}>
                <button
                  type="button"
                  disabled={rolling !== null}
                  onClick={() => void handleRoll(roll)}
                  data-testid={`in-pane-roll-${roll.key}`}
                  className="flex w-full items-center gap-2 rounded border border-border px-2 py-1 text-left text-xs transition-colors hover:bg-muted disabled:opacity-60"
                >
                  <span className="min-w-0 flex-1 truncate">{roll.label}</span>
                  <span className="text-muted-foreground tabular-nums">
                    {rolling === roll.key ? "Rolling…" : roll.formula}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </section>

      {rollError ? (
        <p
          className="text-xs text-destructive"
          data-testid="in-pane-roll-error"
        >
          {rollError}
        </p>
      ) : null}

      {result ? (
        // `RollResult` is the same renderer the dice roller uses, so a roll
        // made from a character reads exactly like one made from the roller —
        // including the Game Master's discrepancy note, which is deliberately
        // never shown to the player rolling (spec 028 FR-067).
        <div data-testid="in-pane-roll-result">
          <RollResult resolution={result} />
        </div>
      ) : null}
    </div>
  );
}
