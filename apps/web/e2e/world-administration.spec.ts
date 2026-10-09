import {
  freshCredentials,
  graphql,
  inviteAndJoinAsPlayer,
  register,
} from "./fixtures/helpers";
import { expect, test } from "./fixtures/test";

/**
 * A Player cannot administer the world they play in.
 *
 * A Game Master on vtt-dev reported: "players can delete the world? why can
 * players generate a join link? or see world settings?" The world dashboard
 * drew "Manage settings", both delete controls and the Campaign Settings
 * panel (join links, campaign switches) for every member. The server already
 * refused the writes; the controls were simply never hidden.
 *
 * The same report said "manage settings — I'm the GM, I can't invite my
 * players": that button was a permanently disabled placeholder. It now opens
 * the world's settings, which carry an invite card for the Game Master.
 *
 * Proven here, in the browser and against the real server:
 *  1. A Player sees no settings, delete or join-link control on the dashboard,
 *     no settings entry in the world nav, and only the read-only view of the
 *     system settings page.
 *  2. The server refuses the Player's deleteWorld, generateInviteCode,
 *     worldInvites and self-promotion, and the world is still there.
 *  3. The Game Master's "Manage settings" leads to a page where they can copy
 *     a join link.
 */

type GqlResult<T> = { data?: T | null; errors?: { message: string }[] };

test("a Player is shown no administration and the server refuses it", async ({
  page,
  browser,
}) => {
  test.setTimeout(150_000);
  await register(page, freshCredentials("e2eadmgm"));

  const world = await graphql<GqlResult<{ createWorld: { id: string } }>>(
    page,
    `
      mutation ($input: GraphQLCreateWorldInput!) {
        createWorld(input: $input) {
          id
        }
      }
    `,
    { input: { name: `E2E Administration ${Date.now()}` } },
  );
  const worldId = world.data?.createWorld?.id;
  expect(worldId, JSON.stringify(world.errors ?? world)).toBeTruthy();

  // A character for the Player to claim: a Player without one is sent to
  // Actor Selection before the dashboard renders, and the players on vtt-dev
  // had theirs.
  const actor = await graphql<GqlResult<{ createActor: { id: string } }>>(
    page,
    `
      mutation ($input: CreateActorInput!) {
        createActor(input: $input) {
          id
        }
      }
    `,
    { input: { worldId, label: "Birdie", isNpc: false } },
  );
  const actorId = actor.data?.createActor?.id;
  expect(actorId, JSON.stringify(actor.errors ?? actor)).toBeTruthy();
  await graphql(
    page,
    `
      mutation ($actorId: UUID!) {
        setActorAvailability(actorId: $actorId, available: true) {
          id
        }
      }
    `,
    { actorId },
  );

  const player = await inviteAndJoinAsPlayer(
    browser,
    page,
    worldId!,
    "e2eadmp",
  );
  const claim = await graphql<GqlResult<{ claimActor: { actorId: string } }>>(
    player,
    `
      mutation ($worldId: UUID!, $actorId: UUID!) {
        claimActor(worldId: $worldId, actorId: $actorId) {
          actorId
        }
      }
    `,
    { worldId, actorId },
  );
  expect(
    claim.data?.claimActor?.actorId,
    `claim refused: ${JSON.stringify(claim.errors ?? claim)}`,
  ).toBe(actorId);

  await test.step("the Game Master's dashboard carries the controls", async () => {
    await page.goto(`/world/${worldId}`);
    await expect(page.getByTestId("world-manage-settings")).toBeVisible({
      timeout: 15_000,
    });
    await expect(page.getByTestId("world-delete")).toBeVisible();
    // Spec 088: links are made on the players page; the dashboard points there.
    await expect(
      page.getByTestId("campaign-settings-players-link"),
    ).toBeVisible();
    await page.goto(`/world/${worldId}/players`);
    await expect(
      page.getByRole("button", { name: "Generate Join Link" }),
    ).toBeVisible({ timeout: 15_000 });
  });

  await test.step("the Player's dashboard carries none of them", async () => {
    await player.goto(`/world/${worldId}`);
    await expect(player).toHaveURL(new RegExp(`/world/${worldId}$`));
    await expect(player.getByText("World dashboard")).toBeVisible({
      timeout: 15_000,
    });
    // The game system is still shown, as a fact rather than a way in.
    await expect(player.getByText("Game system")).toBeVisible();
    await expect(player.getByTestId("world-manage-settings")).toHaveCount(0);
    await expect(player.getByTestId("world-delete")).toHaveCount(0);
    await expect(player.getByTestId("world-delete-permanently")).toHaveCount(0);
    await expect(player.getByText(/delete/i)).toHaveCount(0);
    await expect(player.getByTestId("world-system-settings-link")).toHaveCount(
      0,
    );
    await expect(player.getByText("Campaign Settings")).toHaveCount(0);
    await expect(
      player.getByRole("button", { name: "Generate Join Link" }),
    ).toHaveCount(0);
    await player.goto(`/world/${worldId}/players`);
    await expect(player.getByTestId("world-nav-overview")).toBeVisible({
      timeout: 15_000,
    });
    await expect(player.getByTestId("world-links-panel")).toHaveCount(0);
  });

  await test.step("the Player's world nav has no settings entry", async () => {
    await player.goto(`/world/${worldId}/staging`);
    await expect(player.getByTestId("world-nav-overview")).toBeVisible({
      timeout: 15_000,
    });
    await expect(player.getByTestId("world-nav-system-settings")).toHaveCount(
      0,
    );
    await expect(player.getByTestId("session-setup-invite-link")).toHaveCount(
      0,
    );
  });

  await test.step("typed in, the settings page shows a Player a GIF, not the settings", async () => {
    await player.goto(`/world/${worldId}/settings/system`);
    await expect(player.getByTestId("settings-not-for-players")).toBeVisible({
      timeout: 15_000,
    });
    await expect(player.getByTestId("active-system-card")).toHaveCount(0);
    await expect(player.getByTestId("system-picker-card")).toHaveCount(0);
    await expect(player.getByTestId("default-scene-grid-card")).toHaveCount(0);
    await expect(
      player.getByTestId("settings-invite-players-card"),
    ).toHaveCount(0);
  });

  await test.step("the server refuses the Player", async () => {
    const attempts: [string, string, Record<string, unknown>][] = [
      [
        "deleteWorld",
        `mutation ($id: UUID!) { deleteWorld(id: $id) { id } }`,
        { id: worldId },
      ],
      [
        "generateInviteCode",
        `mutation ($input: GenerateInviteCodeInput!) {
           generateInviteCode(input: $input) { inviteCode }
         }`,
        { input: { worldId, maxUses: 5 } },
      ],
      [
        "worldInvites",
        `query ($worldId: UUID!) { worldInvites(worldId: $worldId) { inviteCode } }`,
        { worldId },
      ],
      [
        "updateMemberRole",
        `mutation ($input: UpdateMemberRoleInput!) {
           updateMemberRole(input: $input) { role }
         }`,
        // The Player promoting themselves.
        { input: { worldId, userId: "{self}", role: "Owner" } },
      ],
      [
        "renameWorld",
        `mutation ($worldId: UUID!) {
           renameWorld(worldId: $worldId, worldName: "Taken over") { id }
         }`,
        { worldId },
      ],
    ];

    const me = await graphql<GqlResult<{ me: { id: string } }>>(
      player,
      `
        query {
          me {
            id
          }
        }
      `,
      {},
    );
    const selfId = me.data?.me?.id;
    expect(selfId, JSON.stringify(me)).toBeTruthy();

    for (const [operation, document, variables] of attempts) {
      const filled = JSON.parse(
        JSON.stringify(variables).replace('"{self}"', JSON.stringify(selfId)),
      ) as Record<string, unknown>;
      const refused = await graphql<GqlResult<Record<string, unknown>>>(
        player,
        document,
        filled,
      );
      expect(
        refused.errors?.length ?? 0,
        `${operation} was allowed: ${JSON.stringify(refused)}`,
      ).toBeGreaterThan(0);
      expect(refused.data?.[operation] ?? null).toBeNull();
    }

    // The world is still there, still named, and the Player still a Player.
    const after = await graphql<
      GqlResult<{
        world: { name: string } | null;
        worldMembers: { userId: string; role: string }[];
      }>
    >(
      page,
      `
        query ($id: UUID!) {
          world(id: $id) {
            name
          }
          worldMembers(worldId: $id) {
            userId
            role
          }
        }
      `,
      { id: worldId },
    );
    expect(after.data?.world?.name ?? "").toMatch(/^E2E Administration/);
    expect(
      after.data?.worldMembers.find((member) => member.userId === selfId)?.role,
    ).toBe("Player");
  });

  await test.step("the Game Master's Manage settings leads to an invite link", async () => {
    await page.goto(`/world/${worldId}`);
    await page.getByTestId("world-manage-settings").click();
    await expect(page).toHaveURL(
      new RegExp(`/world/${worldId}/settings/system$`),
    );
    const card = page.getByTestId("settings-invite-players-card");
    await expect(card).toBeVisible({ timeout: 15_000 });
    await card.getByTestId("session-setup-generate-invite").click();
    await expect(card.getByTestId("session-setup-invite-url")).toHaveValue(
      /\/join\/[A-Za-z0-9_-]+$/,
      { timeout: 10_000 },
    );
  });

  await player.context().close();
});
