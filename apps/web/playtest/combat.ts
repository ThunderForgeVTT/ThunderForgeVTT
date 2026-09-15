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
 * table would find. Where the product has no path at all, the helper is
 * absent rather than invented. (The scenario's FINDINGs, which were those
 * missing paths, are all hard checks since spec 046.)
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
  /** Spec 046: `HIT_POINTS` or `GAME_MASTER` while out of the fight. */
  downedBy: "HIT_POINTS" | "GAME_MASTER" | null;
  /** Spec 046 US6: a lair has no token, no actor and no budget. */
  kind: "CREATURE" | "LAIR";
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
  combatants { id label initiative tiebreak active actorId tokenId isNpc downedBy kind }
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

/**
 * Spec 046 US6: the Game Master puts a lair in the order. The server places it
 * at initiative count 20, losing ties.
 */
export async function addLairCombatant(
  table: Table,
  combatId: string,
  label: string,
): Promise<Combat> {
  const { addLairCombatant: combat } = await must<{
    addLairCombatant: Combat;
  }>(
    table.gm,
    `mutation ($combatId: UUID!, $label: String!) {
      addLairCombatant(combatId: $combatId, label: $label) { ${COMBAT_FIELDS} }
    }`,
    { combatId, label },
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

/**
 * Spec 046 FR-014: the Game Master damages or heals a creature, through the
 * same mutation the tracker's Damage and Heal buttons call.
 */
export async function changeHitPoints(
  page: Page,
  tokenId: string,
  kind: "DAMAGE" | "HEALING",
  amount: number,
): Promise<{ current: number; max: number; temporary: number }> {
  const { changeHitPoints: after } = await must<{
    changeHitPoints: { current: number; max: number; temporary: number };
  }>(
    page,
    `mutation ($tokenId: UUID!, $kind: HitPointChange!, $amount: Int!) {
      changeHitPoints(tokenId: $tokenId, kind: $kind, amount: $amount) {
        current max temporary
      }
    }`,
    { tokenId, kind, amount },
  );
  return after;
}

/** What the server says when this client asks to change hit points. */
export async function refusalOfChangeHitPoints(
  page: Page,
  tokenId: string,
): Promise<string[]> {
  const result = await graphql<{ errors?: { message: string }[] }>(
    page,
    `
      mutation ($tokenId: UUID!) {
        changeHitPoints(tokenId: $tokenId, kind: DAMAGE, amount: 1) {
          current
        }
      }
    `,
    { tokenId },
  );
  return (result.errors ?? []).map((error) => error.message);
}

/**
 * The current figure of a token's first `resourceId` bar, as this client's
 * engine holds it, or null when the engine draws no exact figure for it.
 */
export async function barCurrentOn(
  page: Page,
  tokenId: string,
  resourceId = "hitPoints",
): Promise<number | null> {
  const json = await statusOn(page, tokenId);
  if (!json) return null;
  const resources = JSON.parse(json) as {
    definition: { id: string };
    disclosed: { disclosure: string; entries?: { current: number }[] };
  }[];
  const bar = resources.find((r) => r.definition.id === resourceId);
  return bar?.disclosed.entries?.[0]?.current ?? null;
}

/**
 * A token's hit points as the server resolves them for this viewer: its
 * actor's for a linked token, its own for a copy (spec 046 ADR-102). `null`
 * when the viewer is not shown the figure.
 */
export async function tokenHitPointsOf(
  page: Page,
  sceneId: string,
  tokenId: string,
): Promise<number | null> {
  const { tokenStatus } = await must<{
    tokenStatus: {
      tokenId: string;
      resources: {
        definitionId: string;
        entries: { current: number }[] | null;
      }[];
    }[];
  }>(
    page,
    `query ($sceneId: UUID!) {
      tokenStatus(sceneId: $sceneId) {
        tokenId
        resources { definitionId entries { current } }
      }
    }`,
    { sceneId },
  );
  const bar = tokenStatus
    .find((t) => t.tokenId === tokenId)
    ?.resources.find((r) => r.definitionId === "hitPoints");
  return bar?.entries?.[0]?.current ?? null;
}

/** Make a token its actor, or a copy of it (spec 046 FR-016). */
export async function setTokenLink(
  table: Table,
  tokenId: string,
  linked: boolean,
): Promise<void> {
  await must(
    table.gm,
    `mutation ($tokenId: UUID!, $linked: Boolean!) {
      setTokenLink(tokenId: $tokenId, linked: $linked) { tokenId }
    }`,
    { tokenId, linked },
  );
}

/** Mark an NPC a named individual, or not (spec 046 FR-016). */
export async function setActorUnique(
  table: Table,
  actorId: string,
  unique: boolean,
): Promise<void> {
  await must(
    table.gm,
    `mutation ($actorId: UUID!, $unique: Boolean!) {
      setActorUnique(actorId: $actorId, unique: $unique) { id }
    }`,
    { actorId, unique },
  );
}

/** Write a whole `resource_data` blob, as a sheet does. */
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

/** One line of a combatant's budget as a board shows it. */
export interface BudgetLineShown {
  allowed: number;
  spent: number;
  remaining: number;
  overspent: boolean;
  /** The pip's text, "Action −1/1". */
  text: string;
}

export interface BudgetShown {
  action: BudgetLineShown;
  bonusAction: BudgetLineShown;
  reaction: BudgetLineShown;
  movement: BudgetLineShown;
}

/**
 * Spec 046 US5: what a board's tracker shows `label` has left this turn, or
 * null when its row shows no budget. `label` is matched against the row's
 * text, so a hidden combatant is asked for as "Unknown".
 */
export async function budgetOn(
  page: Page,
  label: string,
): Promise<BudgetShown | null> {
  const row = page
    .getByTestId("combatant-row")
    .filter({ hasText: label })
    .first();
  if ((await row.count()) === 0) return null;
  const budget = row.getByTestId("combatant-budget");
  if ((await budget.count()) === 0) return null;
  const read = async (key: string): Promise<BudgetLineShown> => {
    const pip = budget.getByTestId(`budget-${key}`);
    const number = async (name: string) =>
      Number(await pip.getAttribute(`data-${name}`));
    return {
      allowed: await number("allowed"),
      spent: await number("spent"),
      remaining: await number("remaining"),
      overspent: (await pip.getAttribute("data-overspent")) === "true",
      text: ((await pip.textContent()) ?? "").trim(),
    };
  };
  return {
    action: await read("action"),
    bonusAction: await read("bonus-action"),
    reaction: await read("reaction"),
    movement: await read("movement"),
  };
}

/**
 * Spec 046 US6: `label`'s legendary actions as a board's tracker shows them,
 * or null when its row shows none.
 */
export async function legendaryOn(
  page: Page,
  label: string,
): Promise<BudgetLineShown | null> {
  const row = page
    .getByTestId("combatant-row")
    .filter({ hasText: label })
    .first();
  if ((await row.count()) === 0) return null;
  const pip = row.getByTestId("budget-legendary");
  if ((await pip.count()) === 0) return null;
  const number = async (name: string) =>
    Number(await pip.getAttribute(`data-${name}`));
  return {
    allowed: await number("allowed"),
    spent: await number("spent"),
    remaining: await number("remaining"),
    overspent: (await pip.getAttribute("data-overspent")) === "true",
    text: ((await pip.textContent()) ?? "").trim(),
  };
}

/**
 * Spec 046 US6: the Game Master acts from the tracker — "Spend legendary
 * action" on a legendary creature's row, "Lair action" on a lair's — with the
 * ability named, at the target the list names. Returns what the attack flow
 * says came of it.
 */
export async function actFromTracker(
  page: Page,
  rowLabel: string,
  abilityName: string,
  targetLabel: string,
): Promise<string> {
  await openCombatPanel(page);
  const row = page
    .getByTestId("combatant-row")
    .filter({ hasText: rowLabel })
    .first();
  const open = row
    .getByTestId("spend-legendary-action")
    .or(row.getByTestId("lair-action"));
  await expect(open, `${rowLabel} can be acted for`).toBeVisible({
    timeout: 15_000,
  });
  if ((await open.getAttribute("aria-expanded")) !== "true") {
    await open.click();
  }
  const act = row.getByTestId("combatant-act");
  const choice = act.getByTestId("combatant-act-ability");
  const option = choice.locator("option").filter({ hasText: abilityName });
  await expect(option, `${abilityName} is offered`).toHaveCount(1, {
    timeout: 15_000,
  });
  await choice.selectOption((await option.getAttribute("value")) ?? "");
  const flow = act.getByTestId("attack-flow");
  await expect(flow).toBeVisible();
  await flow
    .getByTestId("attack-flow-target")
    .selectOption({ label: targetLabel });
  await page.waitForTimeout(750);
  await flow.getByTestId("attack-flow-confirm").click();
  const outcome = flow
    .getByTestId("attack-flow-result")
    .or(flow.getByTestId("attack-flow-error"));
  await expect(outcome).toBeVisible({ timeout: 15_000 });
  const text = (await outcome.textContent())?.trim() ?? "";
  await flow.getByTestId("attack-flow-close").click();
  return text;
}

/** A budget's four lines as "remaining/allowed", for polling and messages. */
export async function budgetTextOn(
  page: Page,
  label: string,
): Promise<string | null> {
  const budget = await budgetOn(page, label);
  if (!budget) return null;
  return [
    budget.action.text,
    budget.bonusAction.text,
    budget.reaction.text,
    budget.movement.text,
  ].join(" · ");
}

export async function roundOn(page: Page): Promise<string | null> {
  const counter = page.getByTestId("combat-round-counter");
  if ((await counter.count()) === 0) return null;
  return (await counter.textContent())?.trim() ?? null;
}

/**
 * Author an ability the way the compendium does, and give it to an actor.
 *
 * This is how a fighting style exists: a named thing with formulas on it.
 * An `ATTACK_ROLL` and its `DAMAGE` are resolved against a target only when
 * the ability is made as an attack (spec 046 `makeAttack`, from the sheet or
 * the tracker); rolled on its own, a formula is a number and nothing more.
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
  const sheet = page.getByTestId("in-pane-character-sheet");
  // Already open (an attack was just made from it): count what is there.
  if (await sheet.isVisible().catch(() => false)) {
    return page.locator('[data-testid^="in-pane-roll-ability-"]').count();
  }
  const view = page.getByTestId(`actor-view-${actorId}`);
  if ((await view.count()) === 0) return 0;
  await view.click();
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

// ---------------------------------------------------------------------------
// Spec 046 Phase 6: attacks and offers.
// ---------------------------------------------------------------------------

/** One attack, as a viewer's GraphQL answer carries it. */
export interface AttackSeen {
  id: string;
  attacker: { tokenId: string | null; label: string };
  target: { tokenId: string | null; label: string } | null;
  abilityName: string | null;
  toHit: { resultValue: number };
  damage: { resultValue: number } | null;
  defence: number | null;
  outcome: "HIT" | "MISS" | "NO_DEFENCE" | "NO_TARGET";
  flags: string[];
  offer: {
    id: string;
    amount: number;
    status: "PENDING" | "TAKEN" | "DECLINED" | "APPLIED";
    resolvedBy: string | null;
    resolvedOnBehalf: boolean;
  } | null;
}

const ATTACK_SEEN_FIELDS = `
  id
  attacker { tokenId label }
  target { tokenId label }
  abilityName
  toHit { resultValue }
  damage { resultValue }
  defence
  outcome
  flags
  offer { id amount status resolvedBy resolvedOnBehalf }
`;

/**
 * Hands `actorId` to a seat the way a table does it: the Game Master makes the
 * character available and the player claims it. A claimed character is the
 * one whose sheet opens inside the player's dock (spec 031 US2), which is
 * where their attacks are.
 */
export async function claimFor(
  table: Table,
  seat: { page: Page },
  actorId: string,
): Promise<void> {
  await must(
    table.gm,
    `mutation ($actorId: UUID!, $available: Boolean!) {
      setActorAvailability(actorId: $actorId, available: $available) { id }
    }`,
    { actorId, available: true },
  );
  await must(
    seat.page,
    `mutation ($worldId: UUID!, $actorId: UUID!) {
      claimActor(worldId: $worldId, actorId: $actorId) { actorId }
    }`,
    { worldId: table.worldId, actorId },
  );
}

/**
 * Makes an attack through the product's own mutation, as `page`'s user — the
 * Game Master swinging an NPC's greatclub, which has no sheet in a player's
 * dock to press.
 */
export async function makeAttackAs(
  page: Page,
  input: {
    attackerTokenId: string;
    abilityId: string;
    targetTokenId?: string | null;
    actionCost?: string;
  },
): Promise<AttackSeen[]> {
  const { makeAttack } = await must<{ makeAttack: AttackSeen[] }>(
    page,
    `mutation ($input: AttackInput!) {
      makeAttack(input: $input) { ${ATTACK_SEEN_FIELDS} }
    }`,
    { input },
  );
  return makeAttack;
}

/** The refusal `makeAttack` answers with, or `[]` when it was made. */
export async function refusalOfMakeAttack(
  page: Page,
  input: { attackerTokenId: string; abilityId: string; targetTokenId?: string },
): Promise<string[]> {
  const result = await graphql<{ errors?: { message: string }[] }>(
    page,
    `
      mutation ($input: AttackInput!) {
        makeAttack(input: $input) {
          id
        }
      }
    `,
    { input },
  );
  return (result.errors ?? []).map((error) => error.message);
}

/** One attack as `page`'s user is allowed to read it. */
export async function attackSeenBy(
  page: Page,
  attackId: string,
): Promise<AttackSeen | null> {
  const { attack } = await must<{ attack: AttackSeen | null }>(
    page,
    `query ($id: UUID!) { attack(id: $id) { ${ATTACK_SEEN_FIELDS} } }`,
    { id: attackId },
  );
  return attack;
}

/**
 * Attacks with an ability from the player's own sheet, in the dock: open the
 * character, press the ability's attack roll, choose the target by the name
 * the list shows, and roll. Returns what the attack flow says came of it.
 */
export async function attackFromSheet(
  page: Page,
  actorId: string,
  abilityName: string,
  targetLabel: string,
  options: { onRoll?: () => void } = {},
): Promise<string> {
  return (
    await swingFromSheet(page, actorId, abilityName, targetLabel, options)
  ).said;
}

/**
 * `onRoll` is told the moment the roll is confirmed, which is where a timing
 * of "shown to every seat" starts (spec 046 SC-001) — not after the flow's own
 * preview pause, and not after it has been read and closed.
 */
async function swingFromSheet(
  page: Page,
  actorId: string,
  abilityName: string,
  targetLabel: string,
  options: { onRoll?: () => void } = {},
): Promise<{ warnings: string[]; said: string }> {
  await openDockTab(page, "actors");
  if (!(await page.getByTestId("in-pane-character-sheet").isVisible())) {
    await page.getByTestId(`actor-view-${actorId}`).click();
  }
  const sheet = page.getByTestId("in-pane-character-sheet");
  await expect(sheet).toBeVisible({ timeout: 10_000 });
  const attack = sheet
    .locator('[data-testid^="in-pane-roll-ability-"][data-attack="true"]')
    .filter({ hasText: abilityName })
    .first();
  await expect(
    attack,
    `${abilityName} can be swung from the sheet`,
  ).toBeEnabled({
    timeout: 15_000,
  });
  await attack.click();
  const flow = page.getByTestId("attack-flow");
  await expect(flow).toBeVisible();
  await flow
    .getByTestId("attack-flow-target")
    .selectOption({ label: targetLabel });
  // The preview is asked for when the target changes; give it a moment to
  // say what it has to say before the roll is confirmed.
  await page.waitForTimeout(750);
  const warnings = await flow.getByTestId("attack-flow-flag").allTextContents();
  options.onRoll?.();
  await flow.getByTestId("attack-flow-confirm").click();
  const outcome = flow
    .getByTestId("attack-flow-result")
    .or(flow.getByTestId("attack-flow-error"));
  await expect(outcome).toBeVisible({ timeout: 15_000 });
  const text = (await outcome.textContent())?.trim() ?? "";
  await flow.getByTestId("attack-flow-close").click();
  return { warnings, said: text };
}

/**
 * What an ability is as an attack (spec 046 `setAbilityAttack`): its reach or
 * ranges in the system's units, and whether it needs to see its target.
 */
export async function setAbilityReach(
  table: Table,
  abilityId: string,
  reach: {
    reach?: number | null;
    rangeNormal?: number | null;
    rangeLong?: number | null;
    needsLineOfSight?: boolean;
  },
): Promise<void> {
  await must(
    table.gm,
    `mutation ($abilityId: UUID!, $attack: AttackFieldsInput!) {
      setAbilityAttack(abilityId: $abilityId, attack: $attack)
    }`,
    {
      abilityId,
      attack: {
        reach: reach.reach ?? null,
        rangeNormal: reach.rangeNormal ?? null,
        rangeLong: reach.rangeLong ?? null,
        needsLineOfSight: reach.needsLineOfSight ?? true,
        actionCost: "ACTION",
        legendaryCost: 1,
        multiattack: [],
      },
    },
  );
}

/**
 * What an ability costs as an attack (spec 046 `setAbilityAttack`): a
 * `LEGENDARY` ability spends `legendaryCost` from its creature's pool. A reach
 * that covers the room unless one is given, so only the flags a check reads
 * are raised.
 */
export async function setAbilityCost(
  table: Table,
  abilityId: string,
  cost: {
    actionCost: "ACTION" | "BONUS_ACTION" | "REACTION" | "LEGENDARY" | "FREE";
    legendaryCost?: number;
    reach?: number;
  },
): Promise<void> {
  await must(
    table.gm,
    `mutation ($abilityId: UUID!, $attack: AttackFieldsInput!) {
      setAbilityAttack(abilityId: $abilityId, attack: $attack)
    }`,
    {
      abilityId,
      attack: {
        reach: cost.reach ?? 1000,
        rangeNormal: null,
        rangeLong: null,
        needsLineOfSight: false,
        actionCost: cost.actionCost,
        legendaryCost: cost.legendaryCost ?? 1,
        multiattack: [],
      },
    },
  );
}

/** What one token fills as `page`'s engine draws it (spec 046 US4). */
export interface FootprintDrawn {
  tokenId: string;
  footprint: number;
  x: number;
  y: number;
  width: number;
  height: number;
  nameY: number | null;
  barWidth: number | null;
}

/** Asked of the engine, not the store: what this board actually draws. */
export async function footprintOn(
  page: Page,
  tokenId: string,
): Promise<FootprintDrawn | null> {
  return page.evaluate(
    (tokenId) =>
      (
        window as unknown as {
          __engineProbe?: { tokenFootprints?: () => FootprintDrawn[] };
        }
      ).__engineProbe
        ?.tokenFootprints?.()
        .find((drawn) => drawn.tokenId === tokenId) ?? null,
    tokenId,
  );
}

/** The warnings the attack flow shows before rolling, as sentences. */
export async function attackWarningsFromSheet(
  page: Page,
  actorId: string,
  abilityName: string,
  targetLabel: string,
): Promise<{ warnings: string[]; said: string }> {
  return swingFromSheet(page, actorId, abilityName, targetLabel);
}

/** The attack log's entries on a board, newest first, as text. */
export async function attackLogOn(page: Page): Promise<string[]> {
  return page.getByTestId("attack-log-entry").allTextContents();
}

/** The offers a board is asking its viewer about, as their questions. */
export async function offersOn(page: Page): Promise<string[]> {
  return page.getByTestId("offer-question").allTextContents();
}

/** Take or decline the offer whose question contains `about`, from a board. */
export async function answerOffer(
  page: Page,
  about: string,
  take: boolean,
): Promise<void> {
  const row = page.getByTestId("offer-row").filter({ hasText: about }).first();
  await expect(row, `an offer about ${about}`).toBeVisible({ timeout: 15_000 });
  await row.getByTestId(take ? "offer-take" : "offer-decline").click();
  await expect(row).toBeHidden({ timeout: 15_000 });
}
