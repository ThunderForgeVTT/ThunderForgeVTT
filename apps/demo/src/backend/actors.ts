/**
 * The cast, as the demo backend answers for it: actor rows, their 5e sheet
 * data, and a roll.
 *
 * ADR-044 makes the server the only party that produces a roll. In the demo
 * the in-page backend is that party, so the dice are thrown here and never in
 * a component; the sheet asks and is told, as it would be of a server.
 */
import { GraphQLError } from "graphql";
import system from "../../../../packs/systems/dnd5e/system.json";
import {
  calculateAbilityModifier,
  calculateProficiencyBonus,
  calculateProficiencyBonusForChallenge,
} from "../../../../packs/systems/dnd5e/web/src/derived-data";
import { DND5E_SKILLS } from "../../../../packs/systems/dnd5e/web/src/sheet-regions";
import { DEMO_PLAYER, DEMO_USER } from "../seed/world";
import { now } from "./events";
import { demoState, markChanged, type DemoState, type Row } from "./state";

type Args = Record<string, any>; // eslint-disable-line @typescript-eslint/no-explicit-any

/** The member the page is rendering for. */
export function viewerUser(state: DemoState) {
  return state.viewer === "player" ? DEMO_PLAYER : DEMO_USER;
}

export function viewerIsGm(state: DemoState): boolean {
  return state.viewer !== "player";
}

/**
 * What the viewer may know exists. A player never receives a hidden NPC at
 * all (owner decision 2026-09-15), nor its tokens.
 */
export function actorVisible(state: DemoState, actor: Row): boolean {
  return viewerIsGm(state) || !actor.isNpc || actor.visibleToPlayers === true;
}

export function visibleActors(state: DemoState): Row[] {
  return state.actors.filter((actor) => actorVisible(state, actor));
}

export function tokenVisible(state: DemoState, token: Row): boolean {
  if (!token.actorId) return true;
  const actor = state.actors.find((a) => a.id === token.actorId);
  return !actor || actorVisible(state, actor);
}

function permissionOf(state: DemoState, actor: Row): string {
  if (viewerIsGm(state)) return "OWNER";
  return actor.ownedBy === DEMO_PLAYER.id ? "OWNER" : "VIEWER";
}

/** An actor row with the viewer-dependent fields filled in. */
export function actorRow(state: DemoState, actor: Row): Row {
  const mine = permissionOf(state, actor);
  return {
    ...actor,
    myPermissionLevel: mine,
    myMayChangeImagery: mine !== "VIEWER",
    claimedBy:
      actor.id === state.claimedActorId
        ? {
            id: playerMemberId(state),
            worldId: state.world.id,
            userId: DEMO_PLAYER.id,
            username: DEMO_PLAYER.username,
          }
        : null,
  };
}

/** The seeded player's membership id, as `members` in handlers numbers it. */
export function playerMemberId(state: DemoState): string {
  return `${state.world.id.slice(0, -1)}2`;
}

/** `GraphQLActorClaim` for the hero the player is playing, if any. */
export function claimRow(state: DemoState): Row | null {
  const actor = state.actors.find((a) => a.id === state.claimedActorId);
  if (!actor) return null;
  return {
    actorId: actor.id,
    worldMemberId: playerMemberId(state),
    claimedByUserId: DEMO_PLAYER.id,
    claimedAt: state.world.createdAt,
    actor: actorRow(state, actor),
  };
}

/** The player takes up a hero; a claim only ever moves between the two. */
export function claim(state: DemoState, actorId: string): Row {
  const actor = findActor(state, actorId);
  if (actor.isNpc || actor.availableForClaim !== true) {
    throw new GraphQLError("That character cannot be claimed");
  }
  state.claimedActorId = actorId;
  markChanged();
  return claimRow(state) as Row;
}

export function findActor(state: DemoState, actorId: string): Row {
  const actor = state.actors.find((a) => a.id === actorId);
  if (!actor || !actorVisible(state, actor)) {
    throw new GraphQLError("Actor not found");
  }
  return actor;
}

export function systemDataOf(state: DemoState, actorId: string): Row | null {
  return state.systemData.find((row) => row.actorId === actorId) ?? null;
}

const SLOT_KEYS: Record<string, string> = {
  ability_data: "abilityData",
  resource_data: "resourceData",
  proficiency_data: "proficiencyData",
  trait_data: "traitData",
  spell_data: "spellData",
};

/** `updateActorSystemData`: the whole slot, replaced, as the server does. */
export function updateSystemData(input: Args): Row {
  const state = demoState();
  findActor(state, input.actorId);
  const key = SLOT_KEYS[input.dataType];
  if (!key) throw new GraphQLError(`Unknown data type ${input.dataType}`);
  let row = systemDataOf(state, input.actorId);
  if (!row) {
    row = {
      id: crypto.randomUUID(),
      actorId: input.actorId,
      gameSystemId: input.gameSystemId,
      abilityData: null,
      resourceData: null,
      proficiencyData: null,
      traitData: null,
      spellData: null,
      createdAt: now(),
      updatedAt: now(),
    };
    state.systemData.push(row);
  }
  row[key] = input.data;
  row.updatedAt = now();
  markChanged();
  return row;
}

interface Check {
  id: string;
  label: string;
  group: string;
}

export const CHECKS: Check[] = (system.checks as Check[]).map(
  ({ id, label, group }) => ({ id, label, group }),
);

export const CONDITIONS: Row[] = (
  system.conditions as Array<{
    id: string;
    label: string;
    description: string;
    marker: { glyph: string; color: string };
  }>
).map((c) => ({
  id: c.id,
  label: c.label,
  description: c.description,
  glyph: c.marker.glyph,
  color: c.marker.color,
}));

function number(value: unknown, fallback = 0): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function list(value: unknown): string[] {
  return Array.isArray(value) ? value.map(String) : [];
}

/**
 * The modifier a check adds, from the sheet's stored data by the pack's own
 * arithmetic: ability modifier, plus proficiency where the sheet says so.
 */
export function modifierFor(data: Row | null, checkId: string): number {
  const abilities = (data?.abilityData ?? {}) as Row;
  const proficiency = (data?.proficiencyData ?? {}) as Row;
  const traits = (data?.traitData ?? {}) as Row;
  const level = traits.level;
  const bonus =
    typeof level === "number"
      ? calculateProficiencyBonus(level)
      : (calculateProficiencyBonusForChallenge(
          String(traits.challenge ?? "0"),
        ) ?? 2);
  const mod = (ability: string) =>
    calculateAbilityModifier(number(abilities[ability], 10));

  const skill = DND5E_SKILLS.find((s) => s.id === checkId);
  if (skill) {
    const trained = list(proficiency.skill_proficiencies).includes(skill.id);
    const expert = list(proficiency.skill_expertise).includes(skill.id);
    return mod(skill.ability) + (expert ? 2 * bonus : trained ? bonus : 0);
  }
  if (checkId in abilities) return mod(checkId);
  const save = /^save([A-Z][a-z]+)$/.exec(checkId);
  if (save) {
    const ability = save[1].toLowerCase();
    const trained = list(proficiency.saving_throw_proficiencies).includes(
      ability,
    );
    return mod(ability) + (trained ? bonus : 0);
  }
  return 0;
}

/** One d20, from the browser's own randomness. */
export function rollCheck(args: Args): Row {
  const state = demoState();
  findActor(state, args.actorId);
  if (!CHECKS.some((c) => c.id === args.checkId)) {
    throw new GraphQLError(`Unknown check ${args.checkId}`);
  }
  const modifier = modifierFor(systemDataOf(state, args.actorId), args.checkId);
  const die = 1 + Math.floor(Math.random() * 20);
  const sign = modifier < 0 ? "-" : "+";
  return {
    formula: `1d20 ${sign} ${Math.abs(modifier)}`,
    dice: [
      { sidesKind: "NUMERIC", numericSides: 20, finalValue: die, kept: true },
    ],
    resultKind: "TOTAL",
    resultValue: die + modifier,
    outcome: null,
  };
}
