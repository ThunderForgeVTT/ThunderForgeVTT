import { expectNoAxeViolations } from "./fixtures/axe";
import {
  freshCredentials,
  graphql,
  inviteAndJoinAsPlayer,
  register,
} from "./fixtures/helpers";
import { expect, test } from "./fixtures/test";

/**
 * Spec 067 Story 1: a setting a game system declares, end to end.
 *
 * 5e declares `inspiration` in its manifest and nothing else — no migration,
 * no GraphQL type, no panel. This proves the host does the rest:
 *
 *  1. The Game Master finds the setting on the world's System settings page,
 *     drawn from the declaration, on by default.
 *  2. Turning it off reaches a player's open character sheet without a
 *     reload: the Inspiration control leaves.
 *  3. The choice is stored: it is still off after the Game Master reloads.
 *  4. A player reads the setting and cannot change it, in the page or by
 *     calling the mutation themselves.
 */

type GqlResult<T> = { data?: T; errors?: { message: string }[] };

const SET_SETTING = `
  mutation ($worldId: UUID!, $key: String!, $value: JSON!) {
    setWorldSystemSetting(worldId: $worldId, key: $key, value: $value) {
      key
      value
    }
  }
`;

test("a Game Master turns a declared setting off and a player's open sheet follows", async ({
  page,
  browser,
}) => {
  test.setTimeout(180_000);
  await register(page, freshCredentials("e2esetgm"));

  const world = await graphql<GqlResult<{ createWorld: { id: string } }>>(
    page,
    `
      mutation ($input: GraphQLCreateWorldInput!) {
        createWorld(input: $input) {
          id
        }
      }
    `,
    { input: { name: `E2E Settings ${Date.now()}`, gameSystemId: "dnd5e" } },
  );
  const worldId = world.data?.createWorld?.id;
  expect(worldId, JSON.stringify(world.errors ?? world)).toBeTruthy();

  const actor = await graphql<GqlResult<{ createActor: { id: string } }>>(
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
        label: "Vex the Quiet",
        isNpc: false,
        gameSystemId: "dnd5e",
      },
    },
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
    "e2esetp",
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

  // The player has their sheet open, with Inspiration on it.
  await player.goto(`/world/${worldId}/actor/${actorId}/edit`);
  await expect(player.getByTestId("dnd5e-actor-sheet")).toBeVisible({
    timeout: 15_000,
  });
  await expect(player.getByTestId("dnd5e-inspiration")).toBeVisible();

  // The Game Master finds the setting, on by default, and turns it off.
  await page.goto(`/world/${worldId}/settings/system`);
  const card = page.getByTestId("world-system-settings-card");
  await expect(card).toBeVisible({ timeout: 15_000 });
  await expect(card).toContainText("Heroic Inspiration");
  const toggle = page.getByTestId("world-system-setting-inspiration");
  await expect(toggle).toBeChecked();
  await expectNoAxeViolations(
    page,
    '[data-testid="world-system-settings-card"]',
  );
  await toggle.click();
  await expect(toggle).not.toBeChecked({ timeout: 15_000 });

  // The player's sheet follows, with no reload.
  await expect(player.getByTestId("dnd5e-inspiration")).toHaveCount(0, {
    timeout: 15_000,
  });
  await expect(player.getByTestId("dnd5e-actor-sheet")).toBeVisible();

  // Stored, not just shown.
  await page.reload();
  await expect(
    page.getByTestId("world-system-setting-inspiration"),
  ).not.toBeChecked({ timeout: 15_000 });

  // A player reads it and cannot change it.
  await player.goto(`/world/${worldId}/settings/system`);
  const playerToggle = player.getByTestId("world-system-setting-inspiration");
  await expect(playerToggle).toBeVisible({ timeout: 15_000 });
  await expect(playerToggle).not.toBeChecked();
  await expect(playerToggle).toBeDisabled();

  const refused = await graphql<GqlResult<{ setWorldSystemSetting: unknown }>>(
    player,
    SET_SETTING,
    { worldId, key: "inspiration", value: true },
  );
  expect(refused.data?.setWorldSystemSetting ?? null).toBeNull();
  expect(refused.errors?.[0]?.message ?? "").toContain("Game Master");
});
