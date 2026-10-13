import { expect, type Page } from "@playwright/test";
import { graphql, openDockTab } from "../e2e/fixtures/helpers";
import { must, type Table } from "./table";

/**
 * Attacks: abilities granted and costed, made from a sheet or through the
 * mutation, refused, and what each board's log and offers show of them.
 */

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
