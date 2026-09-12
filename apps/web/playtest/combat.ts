import { expect, type Page } from "@playwright/test";
import { graphql, openDockTab } from "../e2e/fixtures/helpers";
import { must, type Table } from "./table";

/**
 * The fight: initiative, turns, dice, hit points and what each board shows of
 * them.
 *
 * Everything here drives what the product already offers — the combat
 * mutations a Game Master's panel calls, the dice roller every seat has, the
 * system data an actor's sheet is built from — so a playtest finds what a
 * table would find. Where the product has no path at all (see the scenario's
 * FINDINGs), the helper is absent rather than invented.
 */

export interface Combatant {
  id: string;
  label: string;
  initiative: number;
  /** Breaks an initiative tie, highest first, before the id decides. */
  tiebreak: number;
  active: boolean;
  actorId: string | null;
  tokenId: string | null;
  isNpc: boolean;
}

export interface Combat {
  id: string;
  round: number;
  activeCombatantId: string | null;
  endedAt: string | null;
  combatants: Combatant[];
}

const COMBAT_FIELDS = `
  id round activeCombatantId endedAt
  combatants { id label initiative tiebreak active actorId tokenId isNpc }
`;

export async function startCombat(table: Table): Promise<Combat> {
  const { startCombat: combat } = await must<{ startCombat: Combat }>(
    table.gm,
    `mutation ($input: StartCombatInput!) {
      startCombat(input: $input) { ${COMBAT_FIELDS} }
    }`,
    { input: { worldId: table.worldId, sceneId: table.sceneId } },
  );
  return combat;
}

export async function addCombatant(
  table: Table,
  combatId: string,
  entry: {
    label: string;
    actorId?: string;
    tokenId?: string;
    initiative: number;
    tiebreak?: number;
    isNpc?: boolean;
  },
): Promise<Combat> {
  const { addCombatant: combat } = await must<{ addCombatant: Combat }>(
    table.gm,
    `mutation ($input: AddCombatantInput!) {
      addCombatant(input: $input) { ${COMBAT_FIELDS} }
    }`,
    { input: { combatId, ...entry } },
  );
  return combat;
}

export async function updateCombatant(
  table: Table,
  input: {
    combatantId: string;
    initiative?: number;
    tiebreak?: number;
    active?: boolean;
    label?: string;
  },
): Promise<Combat> {
  const { updateCombatant: combat } = await must<{ updateCombatant: Combat }>(
    table.gm,
    `mutation ($input: UpdateCombatantInput!) {
      updateCombatant(input: $input) { ${COMBAT_FIELDS} }
    }`,
    { input },
  );
  return combat;
}

export async function advanceTurn(
  table: Table,
  combatId: string,
): Promise<Combat> {
  const { advanceTurn: combat } = await must<{ advanceTurn: Combat }>(
    table.gm,
    `mutation ($combatId: UUID!) {
      advanceTurn(combatId: $combatId) { ${COMBAT_FIELDS} }
    }`,
    { combatId },
  );
  return combat;
}

export async function endCombat(
  table: Table,
  combatId: string,
): Promise<Combat> {
  const { endCombat: combat } = await must<{ endCombat: Combat }>(
    table.gm,
    `mutation ($combatId: UUID!) {
      endCombat(combatId: $combatId) { ${COMBAT_FIELDS} }
    }`,
    { combatId },
  );
  return combat;
}

/** The running fight as this client's own session is allowed to see it. */
export async function combatSeenBy(
  page: Page,
  worldId: string,
): Promise<Combat | null> {
  const { activeCombat } = await must<{ activeCombat: Combat | null }>(
    page,
    `query ($worldId: UUID!) {
      activeCombat(worldId: $worldId) { ${COMBAT_FIELDS} }
    }`,
    { worldId },
  );
  return activeCombat;
}

/** What the server says when this client asks to advance the turn. */
export async function refusalOfAdvanceTurn(
  page: Page,
  combatId: string,
): Promise<string[]> {
  const result = await graphql<{ errors?: { message: string }[] }>(
    page,
    `
      mutation ($combatId: UUID!) {
        advanceTurn(combatId: $combatId) {
          id
        }
      }
    `,
    { combatId },
  );
  return (result.errors ?? []).map((error) => error.message);
}

/** Roll in this client's own dice roller, and read the number it shows. */
export async function rollInPanel(
  page: Page,
  formula: string,
): Promise<number> {
  await page.getByTestId("dice-formula-input").fill(formula);
  await page.getByTestId("dice-roll-button").click();
  const result = page.getByTestId("dice-roll-result");
  // The canvas plays a throw before the number appears.
  await expect(result).toBeVisible({ timeout: 30_000 });
  const text = (await result.textContent())?.trim() ?? "";
  const match = /(-?\d+)\s*$/.exec(text);
  if (!match) throw new Error(`the roller showed no number: "${text}"`);
  return Number(match[1]);
}

/** Whatever this client's roller is showing, or null if it shows nothing. */
export async function rollShown(page: Page): Promise<string | null> {
  const result = page.getByTestId("dice-roll-result");
  if ((await result.count()) === 0) return null;
  return (await result.textContent())?.trim() ?? null;
}

/** Write a whole `resource_data` blob — the only way hit points change. */
export async function setHitPoints(
  table: Table,
  actorId: string,
  hp: { current: number; max: number; temporary?: number },
): Promise<void> {
  await must(
    table.gm,
    `mutation ($input: GraphQLUpdateActorSystemDataInput!) {
      updateActorSystemData(input: $input) { id }
    }`,
    {
      input: {
        actorId,
        gameSystemId: table.system,
        dataType: "resource_data",
        // Whole, because the column is replaced and `max_hp` is required:
        // a write of `current_hp` alone is refused by the pack's validator.
        data: {
          current_hp: hp.current,
          max_hp: hp.max,
          temporary_hp: hp.temporary ?? 0,
        },
      },
    },
  );
}

/** The six scores a 5e actor must have before anything else will validate. */
export async function setAbilityScores(
  table: Table,
  actorId: string,
  scores: Record<string, number>,
): Promise<void> {
  await must(
    table.gm,
    `mutation ($input: GraphQLUpdateActorSystemDataInput!) {
      updateActorSystemData(input: $input) { id }
    }`,
    {
      input: {
        actorId,
        gameSystemId: table.system,
        dataType: "ability_data",
        data: scores,
      },
    },
  );
}

/**
 * Set a character's traits — where D&D 5e keeps darkvision (spec 045 US6).
 *
 * The same mutation `setAbilityScores` uses, on the slot the system's own
 * `vision` block names. Written through the product's API rather than into
 * the database, so what the playtest proves is what a Game Master editing a
 * sheet would get.
 */
export async function setTraits(
  table: Table,
  actorId: string,
  traits: Record<string, unknown>,
): Promise<void> {
  await must(
    table.gm,
    `mutation ($input: GraphQLUpdateActorSystemDataInput!) {
      updateActorSystemData(input: $input) { id }
    }`,
    {
      input: {
        actorId,
        gameSystemId: table.system,
        dataType: "trait_data",
        data: traits,
      },
    },
  );
}

/**
 * The scene's grid size, in world units per cell.
 *
 * Read from the scene rather than assumed, because a distance a game system
 * quotes in feet only becomes a distance on a board through this number —
 * which is what spec 045 US6 converts through.
 */
export async function gridSizeOf(table: Table): Promise<number> {
  const { scene } = await must<{ scene: { gridSize: number } }>(
    table.gm,
    `query ($sceneId: UUID!) { scene(sceneId: $sceneId) { gridSize } }`,
    { sceneId: table.sceneId },
  );
  return scene.gridSize;
}

/** What this client's engine believes one token can see, in world units. */
export async function darkvisionOn(
  page: Page,
  tokenId: string,
): Promise<number | null> {
  return page.evaluate(
    (tokenId) =>
      (
        window as unknown as {
          __engineProbe?: { tokenVision?: (id: string) => number | null };
        }
      ).__engineProbe?.tokenVision?.(tokenId) ?? null,
    tokenId,
  );
}

export interface ActorSystemData {
  abilityData: Record<string, unknown> | null;
  resourceData: Record<string, number> | null;
  traitData: Record<string, unknown> | null;
  spellData: Record<string, unknown> | null;
}

export async function systemDataOf(
  page: Page,
  actorId: string,
): Promise<ActorSystemData> {
  const { actorSystemData } = await must<{ actorSystemData: ActorSystemData }>(
    page,
    `query ($actorId: UUID!) {
      actorSystemData(actorId: $actorId) {
        abilityData resourceData traitData spellData
      }
    }`,
    { actorId },
  );
  return actorSystemData;
}

/** How much a token tells the table about one of its resources. */
export async function setDisclosure(
  table: Table,
  tokenId: string,
  resourceId: string,
  state: "VISIBLE" | "GREYED" | "PERCENTAGE" | "CHUNKED",
): Promise<void> {
  await must(
    table.gm,
    `mutation ($input: SetTokenDisclosureInput!) {
      setTokenDisclosure(input: $input) { tokenId }
    }`,
    { input: { tokenId, resourceId, state } },
  );
}

/**
 * What this client's *engine* holds for a token's status bars, as JSON.
 *
 * Read from the engine rather than the server on purpose: the question a
 * playtest asks is what each board shows, and the gap between the two is
 * exactly what the scenario is looking for.
 */
export async function statusOn(
  page: Page,
  tokenId: string,
): Promise<string | null> {
  const status = await page.evaluate(async (id) => {
    const mod = (await import(
      /* @vite-ignore */ "/src/engine/bevy/tokenStatus.ts"
    )) as typeof import("../src/engine/bevy/tokenStatus");
    return (await mod.readTokenStatus(id)) as unknown;
  }, tokenId);
  return status === null || status === undefined
    ? null
    : JSON.stringify(status);
}

export async function openCombatPanel(page: Page): Promise<void> {
  await openDockTab(page, "combat");
  await expect(page.getByTestId("combat-panel")).toBeVisible({
    timeout: 30_000,
  });
}

/** The roster this board draws, in the order it draws it. */
export async function rosterOn(page: Page): Promise<string[]> {
  return page
    .getByTestId("combatant-row")
    .allTextContents()
    .then((rows) => rows.map((row) => row.replace(/\s+/g, " ").trim()));
}

/** The label of the row this board marks as the active turn. */
export async function activeRowOn(page: Page): Promise<string | null> {
  const row = page.locator(
    '[data-testid="combatant-row"][data-active-turn="true"]',
  );
  if ((await row.count()) === 0) return null;
  return ((await row.first().textContent()) ?? "").replace(/\s+/g, " ").trim();
}

export async function roundOn(page: Page): Promise<string | null> {
  const counter = page.getByTestId("combat-round-counter");
  if ((await counter.count()) === 0) return null;
  return (await counter.textContent())?.trim() ?? null;
}

/**
 * Author an ability the way the compendium does, and give it to an actor.
 *
 * This is how a fighting style exists at all today: a named thing with
 * formulas on it. An effect's `formula` is checked for shape and never
 * resolved, and nothing is ever applied to a target — which is what the
 * scenario's FINDINGs are about.
 */
export async function grantAbility(
  table: Table,
  actorId: string,
  spec: {
    name: string;
    /** Must be one the world's vocabulary knows: `feat`, `spell`, … */
    classification: string;
    /** A spell's level. A feat has none. */
    grade?: number;
    description?: string;
    effects: {
      effectType: "ATTACK_ROLL" | "DAMAGE" | "HEAL" | "MODIFIER";
      formula: string;
      target?: string;
    }[];
  },
): Promise<string> {
  const { createAbility } = await must<{ createAbility: { id: string } }>(
    table.gm,
    `mutation ($input: CreateAbilityInput!) {
      createAbility(input: $input) { id }
    }`,
    {
      input: {
        worldId: table.worldId,
        name: spec.name,
        description: spec.description ?? "",
        classification: spec.classification,
        ...(spec.grade === undefined ? {} : { grade: spec.grade }),
        gmOnly: false,
      },
    },
  );
  for (const [index, effect] of spec.effects.entries()) {
    await must(
      table.gm,
      `mutation ($abilityId: UUID!, $effect: AbilityEffectInput!) {
        addAbilityEffect(abilityId: $abilityId, effect: $effect) { id }
      }`,
      {
        abilityId: createAbility.id,
        effect: {
          effectType: effect.effectType,
          formula: effect.formula,
          target: effect.target ?? "one creature",
          triggerKind: "ON_USE",
          sortOrder: index,
        },
      },
    );
  }
  await must(
    table.gm,
    `mutation ($input: AttachAbilityToActorInput!) {
      attachAbilityToActor(input: $input) { id }
    }`,
    { input: { actorId, abilityId: createAbility.id } },
  );
  return createAbility.id;
}

/**
 * How many of an actor's abilities this client's own character sheet offers
 * to roll. Zero means the player cannot reach their own style from Play.
 */
export async function abilityRollsOn(
  page: Page,
  actorId: string,
): Promise<number> {
  await openDockTab(page, "actors");
  const view = page.getByTestId(`actor-view-${actorId}`);
  if ((await view.count()) === 0) return 0;
  await view.click();
  const sheet = page.getByTestId("in-pane-character-sheet");
  if (!(await sheet.isVisible().catch(() => false))) return 0;
  return page.locator('[data-testid^="in-pane-roll-ability-"]').count();
}

/** Reads until `done` holds or `ms` pass, returning the last reading. */
export async function settle<T>(
  read: () => Promise<T>,
  done: (value: T) => boolean,
  ms: number,
): Promise<T> {
  const deadline = Date.now() + ms;
  let value = await read();
  while (!done(value) && Date.now() < deadline) {
    await new Promise((resolve) => setTimeout(resolve, 250));
    value = await read();
  }
  return value;
}
