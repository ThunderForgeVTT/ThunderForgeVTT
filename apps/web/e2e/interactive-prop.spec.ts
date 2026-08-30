import { expect, test, type Page } from "@playwright/test";
import {
  graphql,
  inviteAndJoinAsPlayer,
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";
import { sceneIds } from "./fixtures/world-cache";

/**
 * Spec 030, User Story 1 — a Game Master places a book, and a player who
 * clicks it gets the page.
 *
 * This is the MVP, and it is the whole architecture end to end with exactly
 * one contributor: a prop is a token with no actor, an interactive points at a
 * lore entry, activation is decided at the server, and the engine dispatches
 * the effect it was handed without knowing what "lore" means.
 *
 * # What is asserted, and why not the screen
 *
 * The claims are about *what reaches whom*. A screen assertion would pass
 * against a client that received an effect's configuration and chose not to
 * render it — and the interface boundary here is precisely that a player is
 * not sent the configuration at all. So the payloads are checked directly.
 *
 * The four claims:
 *
 * 1. A prop is a token with no actor, and placing one disturbs nothing.
 * 2. A player clicking it is told the effect ran, and is told which entry —
 *    because the application, not the engine, opens the tab.
 * 3. A prop with no effect does nothing, and says so rather than failing.
 *    Scenery is a legitimate thing to place.
 * 4. Someone who is not in the world is offered no interactive at all.
 */

const LORE_TITLE_PREFIX = "The Ledger of";

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

test("a Game Master places a book and a player who clicks it gets the page", async ({
  page,
  browser,
}) => {
  test.setTimeout(5 * 60_000);

  const suffix = uniqueSuffix();
  const worldId = await registerAndCreateWorld(page, `Props ${suffix}`);

  const active = await gql<{ world?: { activeSceneId: string | null } }>(
    page,
    `query ($id: UUID!) { world(id: $id) { activeSceneId } }`,
    { id: worldId },
  );
  const [firstScene] = await sceneIds(page, worldId);
  const sceneId = active.world?.activeSceneId ?? firstScene;

  // --- the registry drives what may be authored -------------------------

  const registry = await gql<{
    effectRegistry: {
      id: string;
      subjectKinds: string[];
      config: { key: string; kind: string; required: boolean }[];
    }[];
  }>(
    page,
    `query { effectRegistry { id subjectKinds config { key kind required } } }`,
    {},
  );

  const lore = registry.effectRegistry.find((d) => d.id === "lore.open");
  expect(lore, "the lore contributor is compiled into this build").toBeTruthy();
  expect(lore!.subjectKinds).toContain("prop");

  // The structural claim behind the hostile-destination edge case: the only
  // way to configure this is a typed reference. There is no field that would
  // accept an address, so a Game Master cannot enter one.
  expect(
    lore!.config.every(
      (f) => f.kind === "reference" || f.kind === "referenceList",
    ),
    "a link effect has no free-text field to type an address into",
  ).toBe(true);

  // Nothing declares a sound effect, because there is no audio subsystem. An
  // absent contributor contributes nothing rather than a dead option.
  expect(
    registry.effectRegistry.some((d) => d.id.startsWith("audio.")),
    "an unbuilt subsystem offers nothing at all",
  ).toBe(false);

  // --- the Game Master places a book and links a page -------------------

  const entry = await gql<{ createLoreEntry: { id: string } }>(
    page,
    `mutation ($input: CreateLoreEntryInput!) {
      createLoreEntry(input: $input) { id }
    }`,
    {
      input: {
        worldId,
        title: `${LORE_TITLE_PREFIX} ${suffix}`,
        content: "A ledger of debts nobody wishes to settle.",
      },
    },
  );
  const entryId = entry.createLoreEntry.id;

  // A prop is a token of the existing `object` kind with no actor. No new
  // placement pipeline, and nothing here passes an actor.
  const prop = await gql<{ createToken: { tokenId: string } }>(
    page,
    `mutation ($input: GraphQLCreateTokenInput!) {
      createToken(input: $input) { tokenId }
    }`,
    { input: { sceneId, x: 100, y: 100, tokenType: "object" } },
  );
  const propTokenId = prop.createToken.tokenId;

  // Claim 1: the actorless token disturbs nothing. `tokenStatus` treats it as
  // a marker rather than a creature, which is the precedent spec 029 set and
  // the one thing a prop most easily breaks.
  const status = await gql<{ tokenStatus: { tokenId: string }[] }>(
    page,
    `query ($sceneId: UUID!) { tokenStatus(sceneId: $sceneId) { tokenId } }`,
    { sceneId },
  );
  expect(
    status.tokenStatus.some((t) => t.tokenId === propTokenId),
    "a prop has no resources, so it carries no status",
  ).toBe(false);

  const created = await gql<{
    createInteractive: { interactiveId: string; available: boolean };
  }>(
    page,
    `mutation ($input: GraphQLCreateInteractiveInput!) {
      createInteractive(input: $input) { interactiveId available }
    }`,
    {
      input: {
        sceneId,
        subjectKind: "prop",
        subjectRef: propTokenId,
        effectId: "lore.open",
        effectConfig: { entry: entryId },
        trigger: "click",
        activation: "anyone",
      },
    },
  );
  const interactiveId = created.createInteractive.interactiveId;
  expect(created.createInteractive.available).toBe(true);

  // Configuration that does not match the declaration is refused at authoring
  // time, not stored and silently ignored later.
  const badConfig = await graphql<{ errors?: { message: string }[] }>(
    page,
    `
      mutation ($input: GraphQLCreateInteractiveInput!) {
        createInteractive(input: $input) {
          interactiveId
        }
      }
    `,
    {
      input: {
        sceneId,
        subjectKind: "prop",
        subjectRef: propTokenId,
        effectId: "lore.open",
        effectConfig: { url: "https://example.invalid" },
        trigger: "click",
        activation: "anyone",
      },
    },
  );
  expect(
    badConfig.errors?.length,
    "a field nothing declared must be refused rather than stored",
  ).toBeGreaterThan(0);

  // --- scenery: a prop with no effect -----------------------------------

  const scenery = await gql<{ createToken: { tokenId: string } }>(
    page,
    `mutation ($input: GraphQLCreateTokenInput!) {
      createToken(input: $input) { tokenId }
    }`,
    { input: { sceneId, x: 300, y: 100, tokenType: "object" } },
  );
  const sceneryInteractive = await gql<{
    createInteractive: { interactiveId: string };
  }>(
    page,
    `mutation ($input: GraphQLCreateInteractiveInput!) {
      createInteractive(input: $input) { interactiveId }
    }`,
    {
      input: {
        sceneId,
        subjectKind: "prop",
        subjectRef: scenery.createToken.tokenId,
        trigger: "click",
        activation: "anyone",
      },
    },
  );

  // --- the player's view -------------------------------------------------

  const playerPage = await inviteAndJoinAsPlayer(browser, page, worldId);

  const playerSees = await gql<{
    interactives: {
      interactiveId: string;
      effectId: string | null;
      effectConfig: unknown | null;
      canActivate: boolean;
    }[];
  }>(
    playerPage,
    `query ($sceneId: UUID!) {
      interactives(sceneId: $sceneId) {
        interactiveId
        effectId
        effectConfig
        canActivate
      }
    }`,
    { sceneId },
  );

  const playerBook = playerSees.interactives.find(
    (i) => i.interactiveId === interactiveId,
  );
  expect(playerBook, "a player is told the book responds").toBeTruthy();
  expect(playerBook!.canActivate).toBe(true);
  // The interface boundary: a player has no use for the configuration, and
  // sending it would invite a client to render it.
  expect(playerBook!.effectId).toBeNull();
  expect(playerBook!.effectConfig).toBeNull();

  // Claim 2: clicking it runs the effect, and says which entry.
  const activated = await gql<{
    activateInteractive: {
      outcome: string;
      effectId: string | null;
      effectConfig: { entry?: string } | null;
    };
  }>(
    playerPage,
    `mutation ($id: UUID!) {
      activateInteractive(interactiveId: $id) {
        outcome
        effectId
        effectConfig
      }
    }`,
    { id: interactiveId },
  );
  expect(activated.activateInteractive.outcome).toBe("performed");
  expect(activated.activateInteractive.effectId).toBe("lore.open");
  expect(activated.activateInteractive.effectConfig?.entry).toBe(entryId);

  // Claim 3: scenery does nothing, and is not an error.
  const inert = await gql<{ activateInteractive: { outcome: string } }>(
    playerPage,
    `mutation ($id: UUID!) {
      activateInteractive(interactiveId: $id) { outcome }
    }`,
    { id: sceneryInteractive.createInteractive.interactiveId },
  );
  expect(inert.activateInteractive.outcome).toBe("noEffect");

  // --- the engine dispatches it -----------------------------------------

  await playerPage.goto(`/world/${worldId}/play`);
  await playerPage.evaluate((id) => {
    (window as unknown as { __interactive: string }).__interactive = id;
  }, interactiveId);
  await waitForEngineReady(playerPage);

  // Drive the same path the application does: activate, then hand the
  // permitted effect to the engine. What is asserted is that the engine
  // *dispatched* it — the seam working — rather than what the effect drew.
  const dispatched = await playerPage.evaluate(async (scene: string) => {
    const sync = (await import(
      /* @vite-ignore */ "/src/engine/world/sync/interactives.ts"
    )) as typeof import("../src/engine/world/sync/interactives");
    const bevy = (await import(
      /* @vite-ignore */ "/src/engine/bevy/index.ts"
    )) as typeof import("../src/engine/bevy/index");

    const opened: string[] = [];
    const stop = bevy.onOpenLore((event) => opened.push(event.entryId));

    const store = bevy.getBoundWorldStore();
    if (!store) {
      stop();
      return { opened, outcome: "no-store" };
    }

    await sync.refreshInteractives(store, scene);
    const result = await sync.activateAndApply(
      store,
      (window as unknown as { __interactive: string }).__interactive,
    );

    // One frame for the engine's Update schedule to read the message.
    await new Promise((resolve) => setTimeout(resolve, 400));
    stop();
    return { opened, outcome: result.outcome };
  }, sceneId);

  expect(dispatched.outcome).toBe("performed");
  expect(
    dispatched.opened,
    "the engine dispatched lore.open, and asked the application to open it",
  ).toContain(entryId);

  // --- claim 4: someone outside the world is offered nothing -------------

  const strangerContext = await browser.newContext();
  const strangerPage = await strangerContext.newPage();
  const { register, freshCredentials } = await import("./fixtures/helpers");
  await register(strangerPage, freshCredentials(`stranger${suffix}`));

  const refused = await graphql<{ errors?: { message: string }[] }>(
    strangerPage,
    `
      query ($sceneId: UUID!) {
        interactives(sceneId: $sceneId) {
          interactiveId
        }
      }
    `,
    { sceneId },
  );
  expect(
    refused.errors?.length,
    "someone who is not at this table is offered no interactive at all",
  ).toBeGreaterThan(0);

  console.log(
    `[prop] registry=${registry.effectRegistry.length} performed=true scenery=noEffect playerConfig=withheld`,
  );

  await strangerContext.close();
  await playerPage.context().close();
});
