import { expect, type Page } from "@playwright/test";
import { graphql, openDockTab } from "../e2e/fixtures/helpers";
import { must, type Table } from "./table";

/**
 * The fight's tracker: a combat started, filled, advanced and ended through
 * the Game Master's mutations, and what each board's Combat panel shows of
 * it (roster, turn, round, budgets, legendary actions).
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
