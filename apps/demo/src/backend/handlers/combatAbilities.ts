/**
 * Spec 079: what each creature in the demo can attack with.
 *
 * On the server these are world abilities a Game Master makes, or that
 * applying a stat block makes for them (`applyStatBlock.ts`). The demo has no
 * ability editor, so it answers with the abilities that applying each
 * ambusher's stat block would have made — from the same `attacksFor` plans —
 * and one weapon each for the two heroes. They are worked out from the cast,
 * not stored: they never change, and a saved world needs no room for them.
 */
import { GraphQLError } from "graphql";
import {
  STAT_BLOCKS,
  attacksFor,
} from "../../../../../packs/systems/dnd5e/web/src/StatBlocks";
import { viewerIsGm } from "../actors";
import { seedId } from "../../seed/world";
import type { DemoState, Row } from "../state";

/** `seedId` kinds the fight's abilities are numbered under (spec 079). */
const ABILITY = 0x79;
const EFFECT = 0x7a;
const ENTRY = 0x7b;

interface Weapon {
  name: string;
  description: string;
  attackRoll: string;
  damage: string;
  reach: number | null;
  rangeNormal: number | null;
  rangeLong: number | null;
}

/** The heroes' sheets carry no ability rows; these are what they fight with. */
const HERO_WEAPONS: Record<string, Weapon[]> = {
  fighter: [
    {
      name: "Longsword",
      description:
        "Melee weapon attack: +5 to hit, reach 5 ft. Hit: 1d8 + 3 slashing.",
      attackRoll: "1d20 + 5",
      damage: "1d8 + 3",
      reach: 5,
      rangeNormal: null,
      rangeLong: null,
    },
  ],
  wizard: [
    {
      name: "Fire Bolt",
      description:
        "Ranged spell attack: +5 to hit, range 120 ft. Hit: 1d10 fire.",
      attackRoll: "1d20 + 5",
      damage: "1d10",
      reach: null,
      rangeNormal: 120,
      rangeLong: 120,
    },
  ],
};

function weaponsOf(actor: Row): Weapon[] {
  const hero = HERO_WEAPONS[actor.castKey as string];
  if (hero) return hero;
  const block = STAT_BLOCKS.find((b) => b.name === actor.label);
  if (!block) return [];
  // A multiattack plan has no roll of its own; the demo leaves it out.
  return attacksFor(block)
    .filter((plan) => plan.attackRoll !== null && plan.damage !== null)
    .map((plan) => ({
      name: plan.name,
      description: plan.description,
      attackRoll: plan.attackRoll as string,
      damage: plan.damage as string,
      reach: plan.reach,
      rangeNormal: plan.rangeNormal,
      rangeLong: plan.rangeLong,
    }));
}

/** Every ability in the world, with the actor it belongs to. */
function catalogue(state: DemoState): Array<{ ability: Row; actorId: string }> {
  const out: Array<{ ability: Row; actorId: string }> = [];
  let n = 0;
  let e = 0;
  for (const actor of state.actors) {
    for (const weapon of weaponsOf(actor)) {
      n += 1;
      const id = seedId(ABILITY, n);
      const effects = [
        ["ATTACK_ROLL", weapon.attackRoll],
        ["DAMAGE", weapon.damage],
      ].map(([effectType, formula], sortOrder) => ({
        id: seedId(EFFECT, (e += 1)),
        abilityId: id,
        effectType,
        formula,
        target: "one creature",
        triggerKind: "ON_USE",
        sortOrder,
      }));
      out.push({
        actorId: actor.id as string,
        ability: {
          id,
          worldId: state.world.id,
          name: weapon.name,
          description: weapon.description,
          classification: "feat",
          grade: null,
          gmOnly: false,
          effects,
          myPermissionLevel: viewerIsGm(state) ? "OWNER" : "VIEWER",
          createdAt: actor.createdAt,
          updatedAt: actor.createdAt,
          moderated: false,
          moderationCaseId: null,
          reach: weapon.reach,
          rangeNormal: weapon.rangeNormal,
          rangeLong: weapon.rangeLong,
          // Melee goes through a gap a bolt cannot: only a ranged attack
          // asks for a clear line (research R2 of spec 046).
          needsLineOfSight: weapon.rangeNormal !== null,
          actionCost: "ACTION",
          legendaryCost: 0,
          multiattack: [],
          linkedFromLore: [],
        },
      });
    }
  }
  return out;
}

export function worldAbilities(
  state: DemoState,
  search?: string | null,
): Row[] {
  const needle = search?.trim().toLowerCase();
  return catalogue(state)
    .map((c) => c.ability)
    .filter((a) => !needle || String(a.name).toLowerCase().includes(needle));
}

export function actorAbilities(state: DemoState, actorId: string): Row[] {
  return catalogue(state)
    .filter((c) => c.actorId === actorId)
    .map(({ ability }) => ({
      id: seedId(ENTRY, Number.parseInt(String(ability.id).slice(-12), 16)),
      actorId,
      abilityId: ability.id,
      abilityName: ability.name,
      classification: ability.classification,
      gmOnly: ability.gmOnly,
    }));
}

/**
 * `find_weapon` (`combat/weapon.rs`): the ability an attack names. The Game
 * Master may use any of the world's; anyone else only one on the attacker's
 * sheet. The demo has no items, so an item is never a weapon here.
 */
export function abilityFor(
  state: DemoState,
  actorId: unknown,
  abilityId: unknown,
  itemId: unknown,
): Row {
  if ((abilityId == null) === (itemId == null)) {
    throw new GraphQLError("Choose one ability or one item to attack with");
  }
  const found = catalogue(state).find(
    (c) =>
      c.ability.id === abilityId &&
      (viewerIsGm(state) || c.actorId === actorId),
  );
  if (!found) throw new GraphQLError("That creature has no such attack");
  return found.ability;
}
