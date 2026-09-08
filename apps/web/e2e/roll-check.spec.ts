import { expect, test, type Page } from "@playwright/test";
import { type GqlResult } from "./fixtures/admin";
import {
  freshCredentials,
  graphql,
  inviteAndJoinAsPlayer,
  register,
  uniqueSuffix,
} from "./fixtures/helpers";

/**
 * Spec 036 US3b (FR-035 to FR-037): a check declared by the world's system,
 * rolled against one actor's numbers, recorded exactly as any other roll.
 *
 * # What this adds to the unit tests
 *
 * `mutations_roll_check_tests.rs` proves the substitution and the three
 * refusals against `roll_check_impl` directly. What it cannot show is that the
 * mutation is reachable through the schema under the argument names a client
 * uses, that the pack the *running server* loads declares the check, and that
 * the number in the result came from the actor rather than from the caller —
 * because over the wire the caller genuinely has no way to send one.
 *
 * # Why the modifier is asserted by arithmetic
 *
 * `worldRollRecords` does not publish the bindings, and `rollCheck` returns
 * the formula with its placeholder still in it (`1d20 + MODIFIER`) — which is
 * the point: the client is never told the number. So the assertion is
 * `resultValue - the die = 3`, which is dexterity 16's modifier under 5e's own
 * rules. Nothing in this file ever says "3" to the server.
 */

const SYSTEM = "dnd5e";

/** Dexterity 16 is +3 under the 5e rules the pack declares. */
const DEXTERITY_MODIFIER = 3;

const SYSTEM_CHECKS = `
  query SystemChecks($worldId: UUID!) {
    systemChecks(worldId: $worldId) { id label group }
  }
`;

const ROLL_CHECK = `
  mutation RollCheck($worldId: UUID!, $actorId: UUID!, $checkId: String!) {
    rollCheck(worldId: $worldId, actorId: $actorId, checkId: $checkId) {
      formula
      resultKind
      resultValue
      dice { sidesKind numericSides rolls kept finalValue }
    }
  }
`;

const ROLL_RECORDS = `
  query RollRecords($worldId: UUID!) {
    worldRollRecords(worldId: $worldId) {
      id
      triggeredBy
      resolution { formula resultValue dice { numericSides finalValue } }
    }
  }
`;

interface Resolution {
  formula: string;
  resultKind: string;
  resultValue: number;
  dice: {
    sidesKind: string;
    numericSides: number | null;
    rolls: number[];
    kept: boolean;
    finalValue: number;
  }[];
}

interface RollRecord {
  id: string;
  triggeredBy: string;
  resolution: { formula: string; resultValue: number };
}

/**
 * A world on `dnd5e`, with one character whose sheet is filled in.
 *
 * The system is chosen at creation rather than switched afterwards: a world
 * with content refuses an unacknowledged change (spec 031's guard), and
 * nothing here is about that.
 */
async function aWorldWithACharacter(
  page: Page,
): Promise<{ worldId: string; actorId: string }> {
  await register(page, freshCredentials("e2echeck"));

  const suffix = uniqueSuffix();
  const world = await graphql<GqlResult<{ createWorld: { id: string } }>>(
    page,
    `
      mutation CW($input: GraphQLCreateWorldInput!) {
        createWorld(input: $input) {
          id
          gameSystemId
        }
      }
    `,
    { input: { name: `Check World ${suffix}`, gameSystemId: SYSTEM } },
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
        label: `Stormcaller ${suffix}`,
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

  const sheet = await graphql<
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
        gameSystemId: SYSTEM,
        dataType: "ability_data",
        // The scores, which is all a sheet records. The *modifiers* are the
        // ruleset's business, and the whole point of the mutation under test
        // is that the server derives them.
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
  if (!sheet.data?.updateActorSystemData?.id) {
    throw new Error(
      `could not fill the sheet in: ${JSON.stringify(sheet.errors ?? sheet)}`,
    );
  }

  return { worldId, actorId };
}

async function rollRecords(page: Page, worldId: string): Promise<RollRecord[]> {
  const result = await graphql<GqlResult<{ worldRollRecords: RollRecord[] }>>(
    page,
    ROLL_RECORDS,
    { worldId },
  );
  return result.data?.worldRollRecords ?? [];
}

test.describe("Spec 036 US3b: rolling a check the system declares", () => {
  test("a declared check is offered, rolled and recorded, with the number the server derived", async ({
    page,
  }) => {
    const { worldId, actorId } = await aWorldWithACharacter(page);

    // FR-037: the sheet draws its buttons from what the pack declares, and
    // this is that list — from the packs the running server actually loaded.
    const offered = await graphql<
      GqlResult<{
        systemChecks: { id: string; label: string; group: string | null }[];
      }>
    >(page, SYSTEM_CHECKS, { worldId });
    const dexterity = offered.data?.systemChecks?.find(
      (check) => check.id === "dexterity",
    );
    expect(
      dexterity,
      "dnd5e declares a Dexterity check, and a sheet cannot offer what it is not told about",
    ).toBeTruthy();
    expect(dexterity!.label).toBe("Dexterity");
    expect(dexterity!.group).toBe("abilities");

    const before = await rollRecords(page, worldId);

    const rolled = await graphql<GqlResult<{ rollCheck: Resolution }>>(
      page,
      ROLL_CHECK,
      { worldId, actorId, checkId: "dexterity" },
    );
    expect(rolled.errors, "the actor's owner may roll their check").toBeFalsy();
    const resolution = rolled.data!.rollCheck;

    // The formula is the system's, placeholder and all — the client is told
    // what was rolled and never what it was rolled with.
    expect(resolution.formula).toContain("d20");
    expect(resolution.formula).toContain("MODIFIER");
    expect(resolution.dice).toHaveLength(1);
    expect(resolution.dice[0].numericSides).toBe(20);
    expect(resolution.resultKind).toBe("TOTAL");

    // FR-035, stated as arithmetic: the total is the die plus a modifier this
    // test never sent. Dexterity 16 is +3 under 5e's rules, and the only
    // place that number exists is the ruleset.
    expect(
      resolution.resultValue - resolution.dice[0].finalValue,
      "the modifier must be the one derived from the sheet, not one the caller supplied",
    ).toBe(DEXTERITY_MODIFIER);

    // FR-036: it is an ordinary roll record — same table, same history, and
    // attributed to the person who asked.
    const after = await rollRecords(page, worldId);
    expect(after.length).toBe(before.length + 1);
    const record = after.find(
      (entry) => !before.some((earlier) => earlier.id === entry.id),
    );
    expect(record).toBeTruthy();
    expect(record!.resolution.formula).toBe(resolution.formula);
    expect(record!.resolution.resultValue).toBe(resolution.resultValue);
  });

  test("an unknown check id is refused and never treated as a formula", async ({
    page,
  }) => {
    const { worldId, actorId } = await aWorldWithACharacter(page);
    const before = await rollRecords(page, worldId);

    for (const candidate of ["1d20+100", "not_a_check", "dexterity "]) {
      const refused = await graphql<GqlResult<{ rollCheck: Resolution }>>(
        page,
        ROLL_CHECK,
        { worldId, actorId, checkId: candidate },
      );
      expect(
        refused.data?.rollCheck,
        `'${candidate}' must not resolve to a roll`,
      ).toBeFalsy();
      // Refused as an *id*. `1d20+100` is a perfectly valid formula, so a
      // server that parsed the argument would answer it happily — which is
      // exactly the hole this refusal exists to close.
      expect(refused.errors?.[0]?.message ?? "").toContain(
        "declares no such check",
      );
    }

    // And nothing was rolled on the way to any of those refusals: a refusal
    // that still left a record would mean a die had already been thrown.
    expect((await rollRecords(page, worldId)).length).toBe(before.length);
  });

  test("a member without permission on the actor may not roll its checks", async ({
    page,
    browser,
  }) => {
    const { worldId, actorId } = await aWorldWithACharacter(page);
    const before = await rollRecords(page, worldId);

    // A real member of the world, with no explicit grant on this actor — so
    // they resolve to Viewer, and a check needs Editor.
    const player = await inviteAndJoinAsPlayer(
      browser,
      page,
      worldId,
      "e2echeckplayer",
    );

    try {
      // First: they are genuinely in the world, and the checks surface answers
      // them. Without this, the refusal below would be equally satisfied by a
      // player who never joined, or by a world that does not exist.
      const offered = await graphql<
        GqlResult<{ systemChecks: { id: string }[] }>
      >(player, SYSTEM_CHECKS, { worldId });
      expect(offered.data?.systemChecks?.map((check) => check.id)).toContain(
        "dexterity",
      );

      const refused = await graphql<GqlResult<{ rollCheck: Resolution }>>(
        player,
        ROLL_CHECK,
        { worldId, actorId, checkId: "dexterity" },
      );
      expect(refused.data?.rollCheck).toBeFalsy();
      // A sheet window is a surface, not a capability: the same requirement
      // as any other action on this actor, in the same words.
      expect(refused.errors?.[0]?.message ?? "").toContain(
        "You do not have sufficient permission on this actor",
      );

      expect(
        (await rollRecords(page, worldId)).length,
        "a refused caller must leave no roll behind",
      ).toBe(before.length);
    } finally {
      await player.context().close();
    }
  });
});
