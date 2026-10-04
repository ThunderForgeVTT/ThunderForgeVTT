/**
 * Putting a stat block on an actor.
 *
 * A pack says what a block would write (`StatBlockPlan`); this does the
 * writing. It knows nothing about any game: slots are written as given, and
 * an attack is an ability with an attack roll, a damage roll and a reach.
 *
 * # Order, and what a failure leaves
 *
 * Slots first. They are what makes the creature hittable (hit points and a
 * defence), and each is one whole-slot write the system's validator accepts
 * or refuses. A refusal stops the apply there: a creature with half a block
 * and its attacks is worse than one with its old numbers.
 *
 * Attacks second, and each on its own. A world may refuse one (a vocabulary
 * the Game Master trimmed, an ability somebody else owns) and the others are
 * still worth having, so a failed attack is a complaint, not a stop.
 *
 * # Three goblins share one scimitar
 *
 * An ability belongs to the world, not the actor. Applying the same block to
 * a second creature attaches the ability the first one made rather than
 * making another, or a world with thirty goblins would list thirty
 * scimitars. "The same" means the name, the type and both formulas match and
 * the person applying may edit it; an ability a Game Master has since
 * changed no longer matches, and a fresh one is made beside it.
 */

import type {
  StatBlockAttackPlan,
  StatBlockPlan,
  StatBlockSlots,
} from "@thunderforge/host";
import {
  addAbilityEffect,
  createAbility,
  getWorldAbilities,
} from "@/api/abilities";
import { attachAbilityToActor } from "@/api/actorAbilities";
import {
  type ActorSystemDataType,
  updateActorSystemData,
} from "@/api/actorSystemData";
import { setAbilityAttack } from "@/api/attacks";
import type { WorldAbilityRecord } from "@/types/ability";

export interface StatBlockTarget {
  id: string;
  worldId: string;
  gameSystemId: string;
}

export interface AppliedStatBlock {
  /** False when a slot was refused; nothing after it was attempted. */
  applied: boolean;
  complaints: string[];
}

const SLOT_ORDER: (ActorSystemDataType & keyof StatBlockSlots)[] = [
  "ability_data",
  "resource_data",
  "proficiency_data",
  "trait_data",
];

function formulaOf(
  ability: WorldAbilityRecord,
  effectType: "ATTACK_ROLL" | "DAMAGE",
): string | null {
  return (
    [...ability.effects]
      .sort((a, b) => a.sortOrder - b.sortOrder)
      .find((effect) => effect.effectType === effectType)?.formula ?? null
  );
}

/**
 * The world ability a plan entry may reuse, if there is one. See "Three
 * goblins share one scimitar" above for what counts.
 */
export function reusableAbility(
  abilities: WorldAbilityRecord[],
  plan: StatBlockAttackPlan,
): WorldAbilityRecord | null {
  return (
    abilities.find(
      (ability) =>
        ability.name === plan.name &&
        ability.classification === plan.classification &&
        ability.myPermissionLevel !== "VIEWER" &&
        formulaOf(ability, "ATTACK_ROLL") === plan.attackRoll &&
        formulaOf(ability, "DAMAGE") === plan.damage,
    ) ?? null
  );
}

function reason(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

async function makeAbility(
  worldId: string,
  plan: StatBlockAttackPlan,
  parts: string[],
): Promise<string> {
  const ability = await createAbility({
    worldId,
    name: plan.name,
    description: plan.description,
    classification: plan.classification,
    gmOnly: false,
  });
  const effects = [
    plan.attackRoll === null
      ? null
      : { effectType: "ATTACK_ROLL" as const, formula: plan.attackRoll },
    plan.damage === null
      ? null
      : { effectType: "DAMAGE" as const, formula: plan.damage },
  ].filter((effect) => effect !== null);
  for (const [sortOrder, effect] of effects.entries()) {
    await addAbilityEffect(ability.id, {
      ...effect,
      target: "one creature",
      triggerKind: "ON_USE",
      sortOrder,
    });
  }
  await setAbilityAttack(ability.id, {
    reach: plan.reach,
    rangeNormal: plan.rangeNormal,
    rangeLong: plan.rangeLong,
    needsLineOfSight: plan.rangeNormal !== null,
    actionCost: "ACTION",
    legendaryCost: 0,
    multiattack: parts,
  });
  return ability.id;
}

export async function applyStatBlock(
  actor: StatBlockTarget,
  plan: StatBlockPlan,
): Promise<AppliedStatBlock> {
  for (const slot of SLOT_ORDER) {
    const data = plan.slots[slot];
    if (!data) {
      continue;
    }
    try {
      await updateActorSystemData(actor.id, actor.gameSystemId, slot, data);
    } catch (error) {
      return {
        applied: false,
        complaints: [`${plan.name} was refused: ${reason(error)}`],
      };
    }
  }

  const complaints: string[] = [];
  let abilities: WorldAbilityRecord[] = [];
  try {
    abilities = await getWorldAbilities(actor.worldId);
  } catch {
    // Not being able to list them only costs the reuse: each is made fresh.
  }

  // Entries with parts go last, so the abilities they name exist by then.
  const ordered = [
    ...plan.attacks.filter((attack) => attack.parts.length === 0),
    ...plan.attacks.filter((attack) => attack.parts.length > 0),
  ];
  const made = new Map<string, string>();
  for (const attack of ordered) {
    try {
      const parts = attack.parts.map((key) => {
        const id = made.get(key);
        if (!id) {
          throw new Error(`${key} could not be made`);
        }
        return id;
      });
      // An entry that groups others is always made fresh: the ability list
      // does not say which parts an existing one names, and one pointing at
      // somebody else's edited attacks would be the wrong multiattack.
      const existing = reusableAbility(abilities, attack);
      const abilityId =
        existing && attack.parts.length === 0
          ? existing.id
          : await makeAbility(actor.worldId, attack, parts);
      made.set(attack.key, abilityId);
      await attachAbilityToActor(actor.id, abilityId);
    } catch (error) {
      complaints.push(`${attack.name}: ${reason(error)}`);
    }
  }

  return { applied: true, complaints };
}
