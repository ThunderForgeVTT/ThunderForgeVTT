import { test, expect } from "@playwright/test";
import { freshCredentials, graphql, register } from "./fixtures/helpers";
import { openAnotherClient } from "./fixtures/clients";

/**
 * Spec 036 US3: what two clients of one account share, and what they do not.
 *
 * "The same experience across clients" is untestable until it is split in
 * two. Membership, permissions and world state must agree everywhere. Which
 * token is selected and where the camera points must not — mirroring those
 * into every window would make two windows useless for the reason people open
 * two windows.
 *
 * The strongest half of that is now structural rather than asserted: only one
 * client of an account holds the play field, so only one runs an engine, so a
 * companion has no selection or camera *to* cross. These tests cover the
 * shared half, and the one privacy property that does not follow from the
 * claim — presence, which is about the person rather than the window.
 */

const HEARTBEAT = `
  mutation Heartbeat($worldId: UUID!) {
    heartbeat(worldId: $worldId)
  }
`;

const PRESENCE = `
  query Presence($worldId: UUID!) {
    worldPresence(worldId: $worldId) { userId secondsSinceSeen }
  }
`;

const MY_WORLDS = `query MyWorlds { myWorlds { id name } }`;

const CREATE_WORLD = `
  mutation CreateWorld($name: String!) {
    createWorld(input: { name: $name }) { id name }
  }
`;

test.describe("Spec 036 US3: shared truth, private view", () => {
  test("a person with two clients open is one person at the table", async ({
    page,
    browser,
  }) => {
    const creds = freshCredentials("e2eshared");
    await register(page, creds);
    const created = await graphql<{
      data: { createWorld: { id: string } };
    }>(page, CREATE_WORLD, { name: `Presence ${Date.now().toString(36)}` });
    const worldId = created.data.createWorld.id;

    const second = await openAnotherClient(browser, creds, "context");

    // Both windows say they are here, which is what two open windows do.
    await graphql(page, HEARTBEAT, { worldId });
    await graphql(second, HEARTBEAT, { worldId });

    const presence = await graphql<{
      data: { worldPresence: { userId: string }[] };
    }>(page, PRESENCE, { worldId });

    // FR-014. Presence is about the person, not the connection: two windows
    // are one player at the table, and a Game Master reading the list must
    // not see somebody twice for having a sheet open on another screen.
    const ids = presence.data.worldPresence.map((p) => p.userId);
    expect(new Set(ids).size).toBe(ids.length);
    expect(ids).toHaveLength(1);

    await second.context().close();
  });

  test("world state agrees across both clients without either reloading", async ({
    page,
    browser,
  }) => {
    const creds = freshCredentials("e2esharedw");
    await register(page, creds);
    const second = await openAnotherClient(browser, creds, "context");

    // Made in one window...
    const name = `Agreed ${Date.now().toString(36)}`;
    const created = await graphql<{
      data: { createWorld: { id: string } };
    }>(second, CREATE_WORLD, { name });
    const worldId = created.data.createWorld.id;

    // ...and true in the other. FR-011: what an account owns is one answer,
    // not one answer per window.
    const seen = await graphql<{ data: { myWorlds: { id: string }[] } }>(
      page,
      MY_WORLDS,
      {},
    );
    expect(seen.data.myWorlds.map((w) => w.id)).toContain(worldId);

    await second.context().close();
  });

  test("losing access takes it from every client, not the one that happened to ask", async ({
    page,
    browser,
  }) => {
    const ownerCreds = freshCredentials("e2esharedown");
    await register(page, ownerCreds);
    const created = await graphql<{ data: { createWorld: { id: string } } }>(
      page,
      CREATE_WORLD,
      { name: `Revoked ${Date.now().toString(36)}` },
    );
    const worldId = created.data.createWorld.id;

    // A member who joins properly, then opens a second window of their own.
    const invite = await graphql<{
      data: { generateInviteCode: { inviteCode: string } };
    }>(
      page,
      `
        mutation ($input: GenerateInviteCodeInput!) {
          generateInviteCode(input: $input) {
            inviteCode
          }
        }
      `,
      { input: { worldId, maxUses: 10 } },
    );

    const memberCreds = freshCredentials("e2esharedmem");
    const memberContext = await browser.newContext();
    const memberFirst = await memberContext.newPage();
    await register(memberFirst, memberCreds);
    // `joinWorld` answers with the *membership*, not the world: `id` is the
    // membership row and `worldId` is the world it is in. Asserting on `id`
    // compares a membership to a world and fails for a reason that has
    // nothing to do with what is under test — which is how this read the
    // first time.
    const joined = await graphql<{
      data: { joinWorld: { worldId: string; userId: string } };
    }>(
      memberFirst,
      `
        mutation ($input: JoinWorldInput!) {
          joinWorld(input: $input) {
            worldId
            userId
          }
        }
      `,
      { input: { inviteCode: invite.data.generateInviteCode.inviteCode } },
    );
    expect(joined.data.joinWorld.worldId).toBe(worldId);
    const memberId = joined.data.joinWorld.userId;

    const memberSecond = await openAnotherClient(
      browser,
      memberCreds,
      "context",
    );

    // Both windows can read the world. This is the half that makes the
    // removal below mean something — without it the test would pass against
    // a member who never had access at all.
    for (const [label, client] of [
      ["the first window", memberFirst],
      ["the second window", memberSecond],
    ] as const) {
      const allowed = await graphql<{ data?: unknown; errors?: unknown[] }>(
        client,
        PRESENCE,
        { worldId },
      );
      expect(
        allowed.errors,
        `${label} should be able to read as a member`,
      ).toBeFalsy();
    }

    await graphql(
      page,
      `
        mutation ($worldId: UUID!, $userId: UUID!) {
          removeMember(worldId: $worldId, userId: $userId)
        }
      `,
      { worldId, userId: memberId },
    );

    // FR-012: the change reaches every live client of the affected account,
    // not only the one that happened to ask next.
    for (const [label, client] of [
      ["the first window", memberFirst],
      ["the second window", memberSecond],
    ] as const) {
      const refused = await graphql<{ errors?: unknown[] }>(client, PRESENCE, {
        worldId,
      });
      expect(
        refused.errors,
        `${label} must lose access when the membership does`,
      ).toBeTruthy();
    }

    await memberContext.close();
    await memberSecond.context().close();
  });
});
