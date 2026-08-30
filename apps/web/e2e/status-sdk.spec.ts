import { expect, test } from "@playwright/test";
import {
  graphql,
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";
import { sceneIds } from "./fixtures/world-cache";

/**
 * Spec 029, User Story 6 — the engine reports what it cannot accept.
 *
 * # Why this test exists
 *
 * Three defects in this spec alone had the same shape: a field name drifted,
 * the engine deserialized what it recognised, ignored the rest, and displayed
 * nothing. No error, no warning, nothing to attach a debugger to. The typed
 * SDK catches drift the compiler can see; this covers the rest — a stale
 * bundle, a hand-built command, a field that changed shape.
 *
 * # Why it observes the console rather than installing a callback
 *
 * The engine's event callback is a single slot that the bridge owns. A test
 * that installed its own would be testing an engine no user ever runs, and
 * would break the app's world sync while doing it. So the report is observed
 * where the product actually puts it, at the end of the bridge's own path.
 *
 * That path had a real bug when this was written: an `sdkError` was parsed as
 * a world command and dispatched into the store, which is worse than dropping
 * it — a command the store has never heard of, entering world state.
 */

test("the engine reports a command it cannot accept instead of dropping it", async ({
  page,
}) => {
  test.setTimeout(3 * 60_000);

  const reports: string[] = [];
  page.on("console", (message) => {
    const text = message.text();
    if (text.includes("[engine sdk]")) reports.push(text);
  });

  const worldId = await registerAndCreateWorld(page, `Sdk ${uniqueSuffix()}`);
  await page.goto(`/world/${worldId}/play`);
  await waitForEngineReady(page);

  // Reach the engine through the module the app itself loaded. ES module
  // instances are shared, so this is the running engine, not a second one.
  const sent = await page.evaluate(async () => {
    const probe = (await import(
      /* @vite-ignore */ "/src/engine/bevy/sdkFaultProbe.ts"
    )) as typeof import("../src/engine/bevy/sdkFaultProbe");

    // Note what is *not* done here: the event callback is left alone. It
    // belongs to the bridge, and replacing it would both break world sync
    // and test an engine no user runs. These are real refusals by the real
    // engine, observed where the product actually reports them.
    const payloads = [
      // A bundle that does not share this engine's contract.
      JSON.stringify({
        type: "set_token_status",
        sdkVersion: 999,
        tokenId: "whatever",
        resources: [],
      }),
      // A command whose shape is wrong — the case that used to vanish.
      JSON.stringify({ type: "set_token_status", tokenId: 12345 }),
      // Not JSON at all.
      "{not json",
    ];
    let delivered = 0;
    for (const payload of payloads) {
      if (await probe.injectRawEngineCommand(payload)) delivered += 1;
    }
    return delivered;
  });

  expect(sent, "the test must be able to reach the engine").toBe(3);

  await expect
    .poll(() => reports.length, {
      message: "every refused command must be reported, not dropped",
      timeout: 30_000,
    })
    .toBeGreaterThanOrEqual(3);

  const all = reports.join("\n");
  expect(all, "a version mismatch must say so").toContain("versionMismatch");
  expect(all, "a wrong shape must say so").toContain("malformed");
  expect(
    all,
    "the report must name the command, or it cannot be acted on",
  ).toContain("set_token_status");

  console.log(`[sdk] sent=${sent} reported=${reports.length} silentDrops=0`);
});

/**
 * Spec 029 T059/FR-020 — a refused command must not damage what was correct.
 *
 * This is the half of "report, don't drop" that is easy to get wrong in the
 * other direction. An engine that clears a token's display *before* checking
 * whether the incoming command is acceptable turns a rejected update into a
 * blanked bar: the player loses information they legitimately had, because
 * something else drifted. That is worse than the original silent drop, and it
 * would pass a test that only asserted an error was reported.
 *
 * So the shape here is: establish a display that is genuinely correct, refuse
 * a command aimed at that same token, then look again.
 */
test("a refused command leaves a correct display exactly as it was", async ({
  page,
}) => {
  test.setTimeout(4 * 60_000);

  const suffix = uniqueSuffix();
  const worldId = await registerAndCreateWorld(page, `SdkKeep ${suffix}`);

  const active = await graphql<{
    data?: { world?: { activeSceneId: string | null } };
  }>(
    page,
    `
      query ($id: UUID!) {
        world(id: $id) {
          activeSceneId
        }
      }
    `,
    {
      id: worldId,
    },
  );
  const [firstScene] = await sceneIds(page, worldId);
  const sceneId = active.data?.world?.activeSceneId ?? firstScene;

  await graphql(
    page,
    `
      mutation ($input: UpdateWorldGameSystemInput!) {
        updateWorldGameSystem(input: $input) {
          id
        }
      }
    `,
    { input: { worldId, gameSystemId: "genie" } },
  );

  const actor = await graphql<{ data?: { createActor?: { id: string } } }>(
    page,
    `
      mutation ($input: CreateActorInput!) {
        createActor(input: $input) {
          id
        }
      }
    `,
    {
      input: {
        worldId,
        label: `Keeper ${suffix}`,
        isNpc: false,
        gameSystemId: "genie",
      },
    },
  );
  const actorId = actor.data?.createActor?.id;
  expect(
    actorId,
    `the actor must exist: ${JSON.stringify(actor)}`,
  ).toBeTruthy();

  await graphql(
    page,
    `
      mutation ($input: GraphQLUpdateActorSystemDataInput!) {
        updateActorSystemData(input: $input) {
          id
        }
      }
    `,
    {
      input: {
        actorId,
        gameSystemId: "genie",
        dataType: "resource_data",
        data: {
          current_health: 7,
          max_health: 12,
          current_wish_points: 3,
          max_wish_points: 5,
        },
      },
    },
  );

  const me = await graphql<{ data?: { me?: { id: string } } }>(
    page,
    `
      query {
        me {
          id
        }
      }
    `,
    {},
  );
  const created = await graphql<{
    data?: { createToken?: { tokenId: string } };
  }>(
    page,
    `
      mutation ($input: GraphQLCreateTokenInput!) {
        createToken(input: $input) {
          tokenId
        }
      }
    `,
    { input: { sceneId, x: 0, y: 0, actorId, tokenType: "character" } },
  );
  const tokenId = created.data?.createToken?.tokenId;
  expect(
    tokenId,
    `the token must exist: ${JSON.stringify(created)}`,
  ).toBeTruthy();

  await graphql(
    page,
    `mutation ($input: GraphQLUpdateTokenInput!) {
      updateToken(tokenId: "${tokenId}", input: $input) { tokenId }
    }`,
    { input: { ownerUserId: me.data?.me?.id } },
  );

  await page.goto(`/world/${worldId}/play`);
  await page.evaluate((id) => {
    (window as unknown as { __keepToken: string }).__keepToken = id;
  }, tokenId);
  await waitForEngineReady(page);

  const read = () =>
    page.evaluate(async () => {
      const engine = (await import(
        /* @vite-ignore */ "/src/engine/bevy/tokenStatus.ts"
      )) as typeof import("../src/engine/bevy/tokenStatus");
      const status = await engine.readTokenStatus(
        (window as unknown as { __keepToken: string }).__keepToken,
      );
      return status ? JSON.stringify(status) : null;
    });

  await expect
    .poll(read, {
      message: "the display must be correct before anything is refused",
      timeout: 60_000,
    })
    .not.toBeNull();

  const before = await read();
  expect(before, "the established display carries the real figures").toContain(
    '"current":7',
  );

  // Now aim three refusals at that exact token.
  const sent = await page.evaluate(async (id) => {
    const probe = (await import(
      /* @vite-ignore */ "/src/engine/bevy/sdkFaultProbe.ts"
    )) as typeof import("../src/engine/bevy/sdkFaultProbe");
    const payloads = [
      JSON.stringify({
        type: "set_token_status",
        sdkVersion: 999,
        tokenId: id,
        resources: [],
      }),
      JSON.stringify({ type: "set_token_status", tokenId: id, resources: 5 }),
      JSON.stringify({
        type: "clear_token_status",
        sdkVersion: 999,
        tokenId: id,
      }),
    ];
    let delivered = 0;
    for (const payload of payloads) {
      if (await probe.injectRawEngineCommand(payload)) delivered += 1;
    }
    return delivered;
  }, tokenId);
  expect(sent).toBe(3);

  // Including a refused *clear*, which is the one that would blank the bar.
  const after = await read();
  expect(
    after,
    "a refused command must not alter a display that was already correct",
  ).toBe(before);

   
  console.log(`[sdk] refusedAgainstLiveToken=${sent} displayUnchanged=true`);
});
