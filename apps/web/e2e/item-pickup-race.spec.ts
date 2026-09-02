import { expect, test, type Page } from "@playwright/test";
import {
  freshCredentials,
  graphql,
  inviteAndJoinAsPlayer,
  register,
  uniqueSuffix,
} from "./fixtures/helpers";

/**
 * Two players grab for the same coin purse. Spec 031, SC-006 / FR-016.
 *
 * # The claim under test
 *
 * One placed item becomes exactly **one** inventory entry, no matter how many
 * people reach for it at once. The failure this catches is the one that costs
 * a table something real: the item duplicated, two players each holding the
 * only key in the dungeon, and nobody able to say which of them is wrong.
 *
 * # Why this drives GraphQL and not two canvas clicks
 *
 * The claim is that the *server* settles the race — `mutations_pickup.rs`
 * removes the token with a conditional DELETE whose row count is the whole
 * answer. Nothing about that decision lives in the browser.
 *
 * Two canvas clicks would add a hit-test, a camera, a frame boundary and a
 * prompt animation to each side of a comparison whose subject is which of two
 * HTTP requests reached one Postgres row first. All of that is timing noise
 * about something else, and it would eventually make this test fail for a
 * reason that is not the bug. A flaky test of a correctness property is worse
 * than no test at all: it teaches the team to re-run it.
 *
 * So the two requests go out from two separately authenticated browser
 * contexts — real accounts, real session cookies, real membership, the real
 * `/api/graphql` endpoint — fired as close together as `Promise.all` over two
 * independent pages allows. The UI path from a click to this mutation is
 * covered separately by `PlacedItemPrompt` (its `placed-item-pickup` button)
 * and its component tests; this file is about what the world looks like
 * afterwards.
 *
 * # Why the assertions are on the table and not the responses
 *
 * "Exactly one player got it" is a statement about the world, so it is read
 * back out of the world: every inventory that could hold the item is queried,
 * and the entries are counted. Two successful-looking responses is *precisely*
 * the bug, so believing the responses would be believing the thing under
 * suspicion.
 *
 * The loser's error code is asserted too, and separately. A future change that
 * turns a lost race into a generic failure would still leave one entry in the
 * table and still pass the count — while telling a player that something broke
 * when in fact somebody else was simply quicker.
 */

async function gql<T>(
  page: Page,
  query: string,
  variables: Record<string, unknown>,
): Promise<T> {
  const res = await graphql<{ data?: T; errors?: { message: string }[] }>(
    page,
    query,
    variables,
  );
  if (res.errors?.length || !res.data) {
    throw new Error(`GraphQL failed: ${JSON.stringify(res.errors ?? res)}`);
  }
  return res.data;
}

/** The mutation's raw envelope: the loser's half of this test is the errors. */
type PickupResult = {
  data?: {
    pickUpPlacedItem: {
      id: string;
      actorId: string;
      itemId: string;
      quantity: number;
    } | null;
  };
  errors?: { message: string; extensions?: { code?: string } }[];
};

function pickUp(page: Page, tokenId: string, actorId: string) {
  return graphql<PickupResult>(
    page,
    `
      mutation ($input: PickUpPlacedItemInput!) {
        pickUpPlacedItem(input: $input) {
          id
          actorId
          itemId
          quantity
        }
      }
    `,
    { input: { tokenId, actorId } },
  );
}

async function currentUserId(page: Page): Promise<string> {
  const me = await gql<{ me: { id: string } }>(page, `query { me { id } }`, {});
  return me.me.id;
}

test("two players reaching for one placed item produce exactly one inventory entry", async ({
  browser,
}) => {
  test.setTimeout(180_000);

  // The invite flow writes the code through the clipboard; a context without
  // that permission throws before the invite is stored (scene-live-launch).
  const gmContext = await browser.newContext({
    permissions: ["clipboard-read", "clipboard-write"],
  });
  const gmPage = await gmContext.newPage();

  await register(gmPage, freshCredentials("e2egmpickup"));
  await gmPage.goto("/worlds/create");
  await gmPage.locator("#world-name").fill(`E2E Pickup Race ${uniqueSuffix()}`);
  await gmPage.getByRole("button", { name: /create world/i }).click();
  await gmPage.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 20_000 });
  const worldId = /\/world\/([^/]+)\/staging$/.exec(
    new URL(gmPage.url()).pathname,
  )![1];

  const world = await gql<{ world: { activeSceneId: string | null } }>(
    gmPage,
    `query ($id: UUID!) { world(id: $id) { activeSceneId } }`,
    { id: worldId },
  );
  const sceneId = world.world.activeSceneId;
  expect(
    sceneId,
    "a new world is created with a scene, which is where the item is placed",
  ).toBeTruthy();

  // --- the Game Master places one item ----------------------------------

  const item = await gql<{ createItem: { id: string } }>(
    gmPage,
    `mutation ($input: CreateItemInput!) { createItem(input: $input) { id } }`,
    {
      input: {
        worldId,
        name: `Contested Purse ${uniqueSuffix()}`,
        description: "There is exactly one of it.",
      },
    },
  );
  const itemId = item.createItem.id;

  // Authored the way a GM authors one: a prop token, plus an `item.pickup`
  // interactive holding a typed reference to the item. `mutations_pickup.rs`
  // reads the interactive first and the token's metadata only as a fallback,
  // so going through the interactives surface exercises the path a real
  // placement takes rather than the tolerance built in for the other one.
  const prop = await gql<{ createToken: { tokenId: string } }>(
    gmPage,
    `mutation ($input: GraphQLCreateTokenInput!) {
      createToken(input: $input) { tokenId }
    }`,
    { input: { sceneId, x: 220, y: 160, tokenType: "object" } },
  );
  const tokenId = prop.createToken.tokenId;

  await gql(
    gmPage,
    `mutation ($input: GraphQLCreateInteractiveInput!) {
      createInteractive(input: $input) { interactiveId }
    }`,
    {
      input: {
        sceneId,
        subjectKind: "prop",
        subjectRef: tokenId,
        effectId: "item.pickup",
        // `item` is the key the declaration publishes
        // (`crates/thunderforge-canvas-core/src/item.rs`, `ITEM_KEY`), and
        // the only configured field it has.
        effectConfig: { item: itemId },
        trigger: "click",
        activation: "anyone",
      },
    },
  );

  // --- two players, two characters --------------------------------------

  // Two *different* characters on purpose. A double award then shows up as
  // two rows rather than as one row of quantity two; both would be bugs, and
  // this shape makes the likelier one impossible to miss.
  const actorIds: string[] = [];
  for (const label of ["Ferran", "Ilse"]) {
    const actor = await gql<{ createActor: { id: string } }>(
      gmPage,
      `mutation ($input: CreateActorInput!) { createActor(input: $input) { id } }`,
      { input: { worldId, label: `${label} ${uniqueSuffix()}`, isNpc: false } },
    );
    actorIds.push(actor.createActor.id);
  }

  const playerPages: Page[] = [];
  for (const [index, actorId] of actorIds.entries()) {
    const playerPage = await inviteAndJoinAsPlayer(
      browser,
      gmPage,
      worldId,
      `e2eplpickup${index}`,
    );
    // Editor on the *receiving character* is what the mutation requires — a
    // player needs authority over their own sheet, not over the purse. The GM
    // grants it because only the DM may (spec 010 FR-014).
    await gql(
      gmPage,
      `mutation ($input: SetActorPermissionInput!) {
        setActorPermission(input: $input) { actorId level }
      }`,
      {
        input: {
          actorId,
          userId: await currentUserId(playerPage),
          level: "EDITOR",
        },
      },
    );
    playerPages.push(playerPage);
  }

  // --- both grab at once -------------------------------------------------

  const results = await Promise.all(
    playerPages.map((playerPage, index) =>
      pickUp(playerPage, tokenId, actorIds[index]),
    ),
  );

  const winners = results.filter((r) => r.data?.pickUpPlacedItem);
  const losers = results.filter((r) => (r.errors?.length ?? 0) > 0);
  expect(
    winners.length,
    `exactly one caller may win a contested item; got ${JSON.stringify(results)}`,
  ).toBe(1);
  expect(losers.length).toBe(1);
  expect(
    losers[0].errors?.[0]?.extensions?.code,
    "the loser is told the item is gone, not handed a generic failure — the " +
      "client keys on this code to put the token back and say so (FR-017)",
  ).toBe("ALREADY_TAKEN");

  // --- what the world looks like afterwards ------------------------------

  // Read from the Game Master's session, which has implicit Owner on every
  // character in the world — so this sees both inventories, including the one
  // belonging to the player who lost. A player's own view could not.
  const entries: { actorId: string; itemId: string; quantity: number }[] = [];
  for (const actorId of actorIds) {
    const inventory = await gql<{
      actorInventory: { id: string; itemId: string; quantity: number }[];
    }>(
      gmPage,
      `query ($actorId: UUID!) {
        actorInventory(actorId: $actorId) { id itemId quantity }
      }`,
      { actorId },
    );
    for (const entry of inventory.actorInventory.filter(
      (e) => e.itemId === itemId,
    )) {
      entries.push({ actorId, itemId: entry.itemId, quantity: entry.quantity });
    }
  }

  expect(
    entries,
    "one placed item must become exactly one inventory entry across the whole table",
  ).toHaveLength(1);
  expect(
    entries[0].quantity,
    "and one of it — a duplicate that merged into a quantity is still a duplicate",
  ).toBe(1);
  expect(
    entries[0].actorId,
    "the entry belongs to the character whose pickup the server accepted",
  ).toBe(winners[0].data!.pickUpPlacedItem!.actorId);

  const remaining = await gql<{ tokens: { tokenId: string }[] }>(
    gmPage,
    `query ($sceneId: UUID!) { tokens(sceneId: $sceneId) { tokenId } }`,
    { sceneId },
  );
  expect(
    remaining.tokens.map((t) => t.tokenId),
    "the item that was taken is no longer lying on the map",
  ).not.toContain(tokenId);

  for (const playerPage of playerPages) {
    await playerPage.context().close();
  }
  await gmContext.close();
});
