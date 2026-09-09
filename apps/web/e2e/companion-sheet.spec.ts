import { expect, test, type Page } from "@playwright/test";
import { type GqlResult } from "./fixtures/admin";
import {
  graphql,
  register,
  freshCredentials,
  uniqueSuffix,
} from "./fixtures/helpers";

/**
 * Spec 036 US3b (T048): a check rolled from the **sheet**, by clicking it.
 *
 * # What this adds to `roll-check.spec.ts`
 *
 * That file drives `systemChecks` and `rollCheck` through GraphQL, and proves
 * the server's half completely: the substitution, the refusals, the record.
 * What it cannot show is that a person can reach any of it. Until 2026-09-09
 * they could not — nothing in `apps/web/src` referenced `rollCheck` at all, so
 * a capability the ledger recorded as done was reachable only by hand-rolled
 * requests. This is the test that would have said so.
 *
 * # Why two systems
 *
 * FR-037 is an absence: a system that declares no checks offers no button, and
 * an absence is the easiest thing in the world to get right by accident. A
 * panel that never rendered would pass a one-system test that only asserted
 * the Blades case, and a panel hard-coded to 5e would pass one that only
 * asserted the dnd5e case. Asserting both, in one run, is what makes either
 * assertion mean anything.
 */

/** Declares `checks`. Dexterity 16 is +3 under the rules the pack declares. */
const WITH_CHECKS = "dnd5e";
/** Declares none — its `actionRoll` is a different question (ADR-074). */
const WITHOUT_CHECKS = "blades_in_the_dark";

interface RollRecord {
  id: string;
  resolution: { resultValue: number };
}

const ROLL_RECORDS = `
  query RollRecords($worldId: UUID!) {
    worldRollRecords(worldId: $worldId) {
      id
      resolution { resultValue }
    }
  }
`;

async function aCharacterIn(
  page: Page,
  gameSystemId: string,
): Promise<{ worldId: string; actorId: string }> {
  const suffix = uniqueSuffix();

  const world = await graphql<GqlResult<{ createWorld: { id: string } }>>(
    page,
    `
      mutation CW($input: GraphQLCreateWorldInput!) {
        createWorld(input: $input) {
          id
        }
      }
    `,
    { input: { name: `Sheet World ${suffix}`, gameSystemId } },
  );
  const worldId = world.data?.createWorld?.id;
  if (!worldId) {
    throw new Error(
      `could not create a ${gameSystemId} world: ${JSON.stringify(world.errors ?? world)}`,
    );
  }

  const actor = await graphql<GqlResult<{ createActor: { id: string } }>>(
    page,
    `
      mutation CA($input: CreateActorInput!) {
        createActor(input: $input) {
          id
        }
      }
    `,
    {
      input: {
        worldId,
        label: `Sheet Character ${suffix}`,
        isNpc: false,
        gameSystemId,
      },
    },
  );
  const actorId = actor.data?.createActor?.id;
  if (!actorId) {
    throw new Error(
      `could not create an actor: ${JSON.stringify(actor.errors ?? actor)}`,
    );
  }

  return { worldId, actorId };
}

/** The scores only. The modifiers are the ruleset's business, not this test's. */
async function fillIn5eAbilities(page: Page, actorId: string): Promise<void> {
  const result = await graphql<
    GqlResult<{ updateActorSystemData: { id: string } }>
  >(
    page,
    `
      mutation US($input: GraphQLUpdateActorSystemDataInput!) {
        updateActorSystemData(input: $input) {
          id
        }
      }
    `,
    {
      input: {
        actorId,
        gameSystemId: WITH_CHECKS,
        dataType: "ability_data",
        data: {
          strength: 10,
          dexterity: 16,
          constitution: 14,
          intelligence: 8,
          wisdom: 12,
          charisma: 7,
        },
      },
    },
  );
  if (!result.data?.updateActorSystemData?.id) {
    throw new Error(
      `could not fill the sheet in: ${JSON.stringify(result.errors ?? result)}`,
    );
  }
}

test.describe("Spec 036 US3b: rolling a check from the sheet", () => {
  test("a 5e player presses Dexterity and the table gets the roll", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2esheetchk"));
    const { worldId, actorId } = await aCharacterIn(page, WITH_CHECKS);
    await fillIn5eAbilities(page, actorId);

    const before = await graphql<GqlResult<{ worldRollRecords: RollRecord[] }>>(
      page,
      ROLL_RECORDS,
      { worldId },
    );
    const countBefore = before.data?.worldRollRecords?.length ?? 0;

    await page.goto(`/world/${worldId}/actor/${actorId}/view`);

    const panel = page.getByTestId("system-checks");
    await expect(
      panel,
      "dnd5e declares checks, so its sheet offers them",
    ).toBeVisible({ timeout: 30_000 });

    // The button carries the check's id, and that id is all the click sends:
    // no formula, no modifier, no total. ADR-044's boundary is what makes
    // that not merely true but the only thing it *could* send.
    const dexterity = page.getByTestId("system-check-dexterity");
    await expect(dexterity).toBeVisible();
    await expect(dexterity).toHaveText(/dexterity/i);
    await dexterity.click();

    const result = page.getByTestId("system-check-result");
    await expect(result).toBeVisible({ timeout: 30_000 });
    await expect(
      page.getByTestId("system-check-error"),
      "a filled-in sheet has the value the check binds to",
    ).toHaveCount(0);

    // FR-036: the roll is the table's, on the same record as any other. This
    // is the assertion that a panel computing its own number would fail.
    const after = await graphql<GqlResult<{ worldRollRecords: RollRecord[] }>>(
      page,
      ROLL_RECORDS,
      { worldId },
    );
    const records = after.data?.worldRollRecords ?? [];
    expect(
      records.length,
      "pressing the button must record a roll at the table",
    ).toBe(countBefore + 1);

    // Dexterity 16 is +3 under 5e's own rules, and 1d20 + 3 lands in 4..23.
    // Nothing in this file told the server the number 3.
    const total = records[0].resolution.resultValue;
    expect(total).toBeGreaterThanOrEqual(4);
    expect(total).toBeLessThanOrEqual(23);
  });

  test("a system that declares no checks offers no button at all", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2esheetnochk"));
    const { worldId, actorId } = await aCharacterIn(page, WITHOUT_CHECKS);

    await page.goto(`/world/${worldId}/actor/${actorId}/view`);

    // Waited for rather than asserted immediately: the panel fetches, and
    // "not there yet" and "never going to be there" look identical for the
    // first few hundred milliseconds. Something else on the page being
    // visible is what makes the absence meaningful.
    await expect(page.getByText(/Sheet Character/i).first()).toBeVisible({
      timeout: 30_000,
    });

    // FR-037. Not a disabled button and not an explanation — nothing. Blades
    // has an `actionRoll` in its manifest, and this is the assertion that
    // stops anybody deciding that is close enough to a check to render.
    await expect(page.getByTestId("system-checks")).toHaveCount(0);
  });
});
