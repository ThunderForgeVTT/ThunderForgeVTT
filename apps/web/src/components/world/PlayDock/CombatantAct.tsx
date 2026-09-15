import { useEffect, useMemo, useState } from "react";
import { getTrackerAbilities, type TrackerAbilityRecord } from "@/api/attacks";
import type { CombatantRecord } from "@/types/combat";
import type { TokenRecord } from "@/types/token";
import { AttackFlow } from "./AttackFlow/AttackFlow";
import { abilitiesFor, type CombatantActKind } from "./combatantActions";

/**
 * Spec 046 US6: "Spend legendary action" on a legendary creature's row, and
 * "Lair action" on a lair's. Game Master only; the caller renders it only for
 * them, and the server refuses anyone else.
 *
 * Either opens the attack flow — a legendary action resolves like any other
 * attack (FR-051) — with its cost fixed: `LEGENDARY` for the creature, whose
 * pool the server spends and whose pip the tracker re-reads on event 18; the
 * ability's own for a lair, which has no budget to spend from.
 */
export function CombatantAct({
  worldId,
  combatant,
  tokens,
  kind,
}: {
  worldId: string;
  combatant: CombatantRecord;
  tokens: TokenRecord[];
  kind: CombatantActKind;
}) {
  const [open, setOpen] = useState(false);
  const [abilities, setAbilities] = useState<TrackerAbilityRecord[] | null>(
    null,
  );
  const [abilityId, setAbilityId] = useState("");

  useEffect(() => {
    if (!open || abilities !== null) return;
    let active = true;
    getTrackerAbilities(worldId)
      .then((found) => {
        if (active) setAbilities(found);
      })
      .catch(() => {
        if (active) setAbilities([]);
      });
    return () => {
      active = false;
    };
  }, [open, abilities, worldId]);

  const offered = useMemo(
    () => abilitiesFor(kind, abilities ?? []),
    [kind, abilities],
  );
  const chosen = offered.find((ability) => ability.id === abilityId);
  const name =
    kind === "legendary"
      ? `Spend a legendary action for ${combatant.label}`
      : `Act for ${combatant.label}`;

  return (
    <div className="grid basis-full gap-1 pl-10">
      <button
        type="button"
        aria-expanded={open}
        aria-label={name}
        data-testid={
          kind === "legendary" ? "spend-legendary-action" : "lair-action"
        }
        className="justify-self-start rounded border border-border px-1.5 py-0.5 text-xs text-muted-foreground hover:bg-muted hover:text-foreground"
        onClick={() => {
          setOpen((was) => !was);
          setAbilityId("");
        }}
      >
        {kind === "legendary" ? "Spend legendary action" : "Lair action"}
      </button>
      {open ? (
        <div className="grid gap-1" data-testid="combatant-act">
          <label className="grid gap-1 text-xs">
            <span className="text-muted-foreground">With</span>
            <select
              value={abilityId}
              onChange={(event) => setAbilityId(event.target.value)}
              data-testid="combatant-act-ability"
              className="rounded border border-border bg-background px-2 py-1"
            >
              <option value="">
                {abilities === null ? "Loading…" : "Choose an ability…"}
              </option>
              {offered.map((ability) => (
                <option key={ability.id} value={ability.id}>
                  {ability.name}
                  {kind === "legendary" && ability.actionCost === "LEGENDARY"
                    ? ` (costs ${ability.legendaryCost})`
                    : ""}
                </option>
              ))}
            </select>
          </label>
          {chosen ? (
            <AttackFlow
              key={chosen.id}
              worldId={worldId}
              attackerTokenId={kind === "legendary" ? combatant.tokenId : null}
              lairCombatantId={kind === "lair" ? combatant.id : null}
              actionCost={kind === "legendary" ? "LEGENDARY" : null}
              abilityId={chosen.id}
              abilityName={chosen.name}
              tokens={tokens}
              onClose={() => {
                setOpen(false);
                setAbilityId("");
              }}
            />
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
