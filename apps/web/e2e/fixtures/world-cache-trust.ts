import { expect, type Browser, type BrowserContext, type Page } from "./test";
import {
  graphql,
  inviteAndJoinAsPlayer,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./helpers";
import { sceneIds } from "./world-cache";
import { currentUserId } from "./offline";

// ---------------------------------------------------------------------------
// The trust model (T108–T108c)
// ---------------------------------------------------------------------------

/**
 * Everything below submits to `reconcileQueuedChanges` directly, over a real
 * signed-in session, rather than driving a browser through an outage.
 *
 * That is not a shortcut around end-to-end: it is the only way to reach what
 * SC-021 and SC-021a–c are about. The rules under test — who may attribute a
 * change to whom, and when a reported outcome counts as a discrepancy — live
 * entirely in the server's answer to this one mutation, and the two inputs
 * that decide them (`attributedToUserId`, `reportedOutcome`) cannot be
 * produced from the UI at all: `api/reconcile.ts` sends neither, and no client
 * code path constructs one. Waiting for a queue to drain would exercise a
 * submission that carries none of the fields being tested.
 *
 * What *is* end to end here is everything that matters to the rules: real
 * accounts, real world membership, real roles, real tokens, real dice records
 * the server rolled itself, and the real authorization the mutation performs.
 */

/** What the server says became of one submitted change. */
export interface ReconcileOutcomeWire {
  localId: string;
  applied: boolean;
  reason: string | null;
  supersededByRole: string | null;
  discrepancy: {
    userId: string;
    recordId: string;
    reportedValue: number;
    determinedValue: number;
  } | null;
}

export interface QueuedChange {
  localId: string;
  command: unknown;
  attributedToUserId?: string;
  reportedOutcome?: {
    kind: string;
    version: number;
    recordId?: string | null;
    value?: number | null;
  };
}

export const RECONCILE = `
  mutation ReconcileQueuedChanges($worldId: UUID!, $changes: [QueuedChangeInput!]!) {
    reconcileQueuedChanges(worldId: $worldId, changes: $changes) {
      localId
      applied
      reason
      supersededByRole
      discrepancy { userId recordId reportedValue determinedValue }
    }
  }
`;

/** Submit queued changes as whoever `page` is signed in as. */
export async function reconcile(
  page: Page,
  worldId: string,
  changes: QueuedChange[],
): Promise<ReconcileOutcomeWire[]> {
  const res = await graphql<{
    data?: { reconcileQueuedChanges?: ReconcileOutcomeWire[] };
    errors?: unknown;
  }>(page, RECONCILE, { worldId, changes });
  expect(
    res.errors,
    `reconcileQueuedChanges failed: ${JSON.stringify(res.errors)}`,
  ).toBe(undefined);
  const outcomes = res.data?.reconcileQueuedChanges;
  expect(
    outcomes,
    "the server must answer with one outcome per change",
  ).toHaveLength(changes.length);
  return outcomes!;
}

/** A move command in exactly the shape the outbox stores and replays. */
export function moveCommand(tokenId: string, x: number, y: number) {
  return { type: "upsert_token", token: { id: tokenId, x, y } };
}

/** Create a token straight through the mutation, with no engine in the way. */
export async function newToken(
  page: Page,
  sceneId: string,
  x = 0,
  y = 0,
): Promise<string> {
  const res = await graphql<{
    data?: { createToken?: { tokenId: string } };
    errors?: unknown;
  }>(
    page,
    `
      mutation ($input: GraphQLCreateTokenInput!) {
        createToken(input: $input) {
          tokenId
        }
      }
    `,
    { input: { sceneId, x, y } },
  );
  expect(res.errors, `createToken failed: ${JSON.stringify(res.errors)}`).toBe(
    undefined,
  );
  const tokenId = res.data?.createToken?.tokenId;
  expect(tokenId, "the fixture must actually create a token").toBeTruthy();
  return tokenId!;
}

/** Hand a token to a user, so a non-GM submitter is entitled to move it. */
export async function ownToken(
  page: Page,
  tokenId: string,
  userId: string,
): Promise<void> {
  const res = await graphql<{
    data?: { updateToken?: { ownerUserId: string | null } };
    errors?: unknown;
  }>(
    page,
    `
      mutation ($tokenId: UUID!, $input: GraphQLUpdateTokenInput!) {
        updateToken(tokenId: $tokenId, input: $input) {
          tokenId
          ownerUserId
        }
      }
    `,
    { tokenId, input: { ownerUserId: userId } },
  );
  expect(res.errors, `updateToken failed: ${JSON.stringify(res.errors)}`).toBe(
    undefined,
  );
  expect(res.data?.updateToken?.ownerUserId).toBe(userId);
}

/**
 * Roll dice for real, and read back what the server recorded.
 *
 * The determined value has to come from the server's own record rather than
 * from anything the test computes: a discrepancy is by definition a
 * disagreement with `world_roll_records`, and a test that supplied both
 * numbers would be checking subtraction.
 */
export async function rollAndRecord(
  roller: Page,
  gm: Page,
  worldId: string,
): Promise<{ recordId: string; determined: number }> {
  const rolled = await graphql<{
    data?: { rollDice?: { resultValue: number } };
    errors?: unknown;
  }>(
    roller,
    `
      mutation ($input: RollDiceInput!) {
        rollDice(input: $input) {
          formula
          resultValue
        }
      }
    `,
    { input: { worldId, formula: "3d20+7" } },
  );
  expect(
    rolled.errors,
    `rollDice failed: ${JSON.stringify(rolled.errors)}`,
  ).toBe(undefined);
  const determined = rolled.data?.rollDice?.resultValue;
  expect(determined, "the server must have resolved the roll").toBeGreaterThan(
    0,
  );

  // The record id is only readable by the Game Master (spec 014 FR-014), which
  // is also the only session that would ever be submitting one of these.
  const records = await graphql<{
    data?: {
      worldRollRecords?: { id: string; resolution: { resultValue: number } }[];
    };
    errors?: unknown;
  }>(
    gm,
    `
      query ($worldId: UUID!) {
        worldRollRecords(worldId: $worldId, limit: 5) {
          id
          resolution {
            resultValue
          }
        }
      }
    `,
    { worldId },
  );
  expect(
    records.errors,
    `worldRollRecords failed: ${JSON.stringify(records.errors)}`,
  ).toBe(undefined);
  const match = (records.data?.worldRollRecords ?? []).find(
    (record) => record.resolution.resultValue === determined,
  );
  expect(
    match,
    "the roll the server resolved must be in its own records",
  ).toBeTruthy();
  return { recordId: match!.id, determined: determined! };
}

/** The token's position as the server holds it. */
export async function serverToken(
  page: Page,
  sceneId: string,
  tokenId: string,
): Promise<{ x: number; y: number } | null> {
  const res = await graphql<{
    data?: { tokens?: { tokenId: string; x: number; y: number }[] };
  }>(
    page,
    `
      query ($sceneId: UUID!) {
        tokens(sceneId: $sceneId) {
          tokenId
          x
          y
        }
      }
    `,
    {
      sceneId,
    },
  );
  return res.data?.tokens?.find((token) => token.tokenId === tokenId) ?? null;
}

export interface TrustTable {
  gmContext: BrowserContext;
  gm: Page;
  gmUserId: string;
  players: Page[];
  playerIds: string[];
  worldId: string;
  sceneId: string;
}

/** A Game Master and `playerCount` players, with no engine anywhere. */
export async function trustTable(
  browser: Browser,
  prefix: string,
  playerCount: number,
): Promise<TrustTable> {
  const gmContext = await browser.newContext({
    permissions: ["clipboard-read", "clipboard-write"],
  });
  const gm = await gmContext.newPage();
  const worldId = await registerAndCreateWorld(
    gm,
    `E2E Trust ${uniqueSuffix()}`,
    prefix,
  );
  const [sceneId] = await sceneIds(gm, worldId);
  const gmUserId = await currentUserId(gm);

  const players: Page[] = [];
  const playerIds: string[] = [];
  for (let i = 0; i < playerCount; i += 1) {
    const page = await inviteAndJoinAsPlayer(
      browser,
      gm,
      worldId,
      `${prefix}p${i}`,
    );
    players.push(page);
    playerIds.push(await currentUserId(page));
  }
  return { gmContext, gm, gmUserId, players, playerIds, worldId, sceneId };
}

export async function closeTrustTable(table: TrustTable) {
  for (const player of table.players)
    await player
      .context()
      .close()
      .catch(() => {});
  await table.gmContext.close().catch(() => {});
}
