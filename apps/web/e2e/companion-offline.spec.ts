import { expect, test, type Page } from "@playwright/test";
import { type GqlResult } from "./fixtures/admin";
import {
  freshCredentials,
  graphql,
  register,
  uniqueSuffix,
} from "./fixtures/helpers";

/**
 * Spec 036 US3c (FR-039, FR-040, T053): a companion surface that has lost the
 * server refuses, says so, sends you to the play field — and records nothing.
 *
 * # Why the severing is narrow
 *
 * `fixtures/offline.ts`'s `severableLink` blocks heartbeats, because the
 * suites it was written for are about the *client's verdict* that it has gone
 * offline. This is about one action's transport failure, so only the
 * `RollCheck` mutation is cut. Everything else keeps working, which is what
 * makes the last assertion possible at all: FR-040 says the refusal leaves no
 * record, and the only way to check that is to ask the server what it has —
 * over a link the test did not break.
 *
 * # What could pass this by accident, and does not
 *
 * A panel that queued the roll for replay would show the same refusal and then
 * submit it on reconnect. So the link is restored and the records are read
 * again: a queued roll arrives late, and "nothing was recorded" has to still
 * be true afterwards. That is the assertion FR-040 actually needs, and a
 * before/after count at the moment of refusal would not have caught it.
 */

const SYSTEM = "dnd5e";

interface RollRecord {
  id: string;
}

const ROLL_RECORDS = `
  query RollRecords($worldId: UUID!) {
    worldRollRecords(worldId: $worldId) { id }
  }
`;

async function aCharacterWithScores(
  page: Page,
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
    { input: { name: `Offline Sheet ${suffix}`, gameSystemId: SYSTEM } },
  );
  const worldId = world.data?.createWorld?.id;
  if (!worldId) {
    throw new Error(
      `could not create a world: ${JSON.stringify(world.errors ?? world)}`,
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
        label: `Offline Character ${suffix}`,
        isNpc: false,
        gameSystemId: SYSTEM,
      },
    },
  );
  const actorId = actor.data?.createActor?.id;
  if (!actorId) {
    throw new Error(
      `could not create an actor: ${JSON.stringify(actor.errors ?? actor)}`,
    );
  }

  await graphql(
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
        gameSystemId: SYSTEM,
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

  return { worldId, actorId };
}

test.describe("Spec 036 US3c: a sheet that has lost the server", () => {
  test("refuses the check, names the play field, and records nothing — then or later", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2eoffchk"));
    const { worldId, actorId } = await aCharacterWithScores(page);

    // Only `rollCheck`. The page's other calls — including the panel's own
    // `systemChecks` query — go through untouched, so the buttons are drawn
    // and only the adjudicated action is cut off.
    let severed = true;
    let refusedAttempts = 0;
    await page.route("**/api/graphql", async (route) => {
      const body = route.request().postData() ?? "";
      if (severed && body.includes("RollCheck")) {
        refusedAttempts += 1;
        await route.abort("internetdisconnected");
        return;
      }
      await route.fallback();
    });

    await page.goto(`/world/${worldId}/actor/${actorId}/view`);
    await expect(page.getByTestId("system-checks")).toBeVisible({
      timeout: 30_000,
    });

    await page.getByTestId("system-check-dexterity").click();

    // FR-039: it says it has lost the server and names the play field. Both
    // halves, because "something went wrong" would satisfy neither.
    const refusal = page.getByTestId("system-check-error");
    await expect(refusal).toBeVisible({ timeout: 30_000 });
    await expect(refusal).toContainText(/cannot be reached/i);
    await expect(refusal).toContainText(/play field/i);

    // And no result is shown. A sheet that displayed a number here would be
    // showing one the table never saw.
    await expect(page.getByTestId("system-check-result")).toHaveCount(0);
    expect(refusedAttempts, "the roll was attempted and cut off").toBe(1);

    // FR-040, first half: nothing at the table.
    const during = await graphql<GqlResult<{ worldRollRecords: RollRecord[] }>>(
      page,
      ROLL_RECORDS,
      { worldId },
    );
    expect(during.data?.worldRollRecords ?? []).toHaveLength(0);

    // FR-040, second half — the one that catches a replay queue. Let the link
    // back and give anything that was queued every chance to arrive.
    severed = false;
    await page.waitForTimeout(5_000);

    const after = await graphql<GqlResult<{ worldRollRecords: RollRecord[] }>>(
      page,
      ROLL_RECORDS,
      { worldId },
    );
    expect(
      after.data?.worldRollRecords ?? [],
      "a refused roll must not be queued and replayed when the server returns",
    ).toHaveLength(0);

    // The action is still available: refusing is not disabling. Pressing it
    // now, with the link back, works — which is what makes the refusal a
    // refusal of *this attempt* rather than of the capability.
    await page.getByTestId("system-check-dexterity").click();
    await expect(page.getByTestId("system-check-result")).toBeVisible({
      timeout: 30_000,
    });
    const recovered = await graphql<
      GqlResult<{ worldRollRecords: RollRecord[] }>
    >(page, ROLL_RECORDS, { worldId });
    expect(recovered.data?.worldRollRecords ?? []).toHaveLength(1);
  });
});
