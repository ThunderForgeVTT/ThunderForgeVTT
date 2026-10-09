import { expect, test, type Page } from "./fixtures/test";
import {
  graphql,
  inviteAndJoinAsPlayer,
  registerAndCreateWorld,
  setWorldSystem,
  uniqueSuffix,
} from "./fixtures/helpers";
import { must } from "../playtest/table";

/**
 * Hotfix (owner, 2026-10-08): "why can't my players edit their heroes from
 * the players view or their character sheet?"
 *
 * A player who claimed a character held Editor on it, and every mutation
 * behind the sheet and the builder accepted them, but the Players screen
 * offered them nothing: their hero's name was a link to its read-only page,
 * and nothing there said a sheet or a builder existed. This proves the way
 * in, end to end, through the real claim (never `setActorPermission`, which
 * is how older specs got a player Editor and why none of them saw this):
 *
 *  1. The Game Master names a character on the Players screen; a player
 *     joins and claims it on Actor Selection.
 *  2. On the Players screen the player is offered their hero's builder and
 *     sheet. The builder saves a portrait and a token the Game Master then
 *     has.
 *  3. The sheet, opened from there, is editable: a name and a score the
 *     player sets are what the Game Master reads.
 *  4. Another player, holding nothing on that hero, is offered neither, and
 *     the server refuses them the same writes.
 */

const ACTOR_SYSTEM_DATA = `
  query ($actorId: UUID!) {
    actorSystemData(actorId: $actorId) { abilityData }
  }
`;

async function strengthOf(page: Page, actorId: string) {
  const { actorSystemData } = await must<{
    actorSystemData: { abilityData: Record<string, unknown> | null } | null;
  }>(page, ACTOR_SYSTEM_DATA, { actorId });
  return actorSystemData?.abilityData?.strength ?? null;
}

async function heroOf(page: Page, worldId: string, actorId: string) {
  const { worldActors } = await must<{
    worldActors: {
      id: string;
      label: string;
      images: { role: string }[];
    }[];
  }>(
    page,
    `query ($worldId: UUID!) {
      worldActors(worldId: $worldId) { id label images { role } }
    }`,
    { worldId },
  );
  return worldActors.find((actor) => actor.id === actorId) ?? null;
}

test.describe("A player edits their own hero from the Players screen", () => {
  test("the claim's holder opens the builder and the sheet from Players, their edits reach the Game Master, and another player is offered neither", async ({
    page: gm,
    browser,
  }) => {
    test.setTimeout(240_000);
    const worldId = await registerAndCreateWorld(
      gm,
      `E2E Hero Edit ${uniqueSuffix()}`,
    );
    await setWorldSystem(gm, worldId, "dnd5e");

    // 1. Named on the Players screen, as the owner's table did it.
    const name = `Wren ${uniqueSuffix()}`;
    await gm.goto(`/world/${worldId}/players`);
    await gm.getByTestId("new-character-name").fill(name);
    await gm.getByTestId("new-character-submit").click();
    await expect(gm.getByTestId("new-character-made")).toHaveText(
      `${name} is ready to be claimed.`,
      { timeout: 10_000 },
    );

    const player = await inviteAndJoinAsPlayer(browser, gm, worldId, "e2ehero");
    const other = await inviteAndJoinAsPlayer(browser, gm, worldId, "e2eother");
    try {
      // The real claim: Actor Selection's "Select".
      await player.goto(`/world/${worldId}/actor-select`);
      const row = player
        .getByTestId("available-actor-row")
        .filter({ hasText: name });
      await expect(row).toHaveCount(1, { timeout: 15_000 });
      await row.getByRole("button", { name: "Select" }).click();
      await player.waitForURL(new RegExp(`/world/${worldId}$`), {
        timeout: 15_000,
      });
      const { myActorClaim } = await must<{
        myActorClaim: { actorId: string } | null;
      }>(
        player,
        `query ($worldId: UUID!) { myActorClaim(worldId: $worldId) { actorId } }`,
        { worldId },
      );
      const actorId = myActorClaim!.actorId;

      // 2. The builder, from the Players screen.
      await player.goto(`/world/${worldId}/players`);
      const build = player.getByTestId(`player-hero-build-${actorId}`);
      await expect(build).toBeVisible({ timeout: 15_000 });
      await build.click();
      const dialog = player.getByTestId("hero-builder-dialog");
      await expect(dialog.getByTestId("hero-builder")).toBeVisible({
        timeout: 15_000,
      });
      await dialog.getByTestId("hero-dialog-save").click();
      await expect(dialog).toBeHidden({ timeout: 20_000 });
      await expect(player).toHaveURL(new RegExp(`/world/${worldId}/players$`));
      await expect
        .poll(
          async () =>
            (await heroOf(gm, worldId, actorId))?.images
              .map((image) => image.role)
              .sort(),
          { timeout: 15_000 },
        )
        .toEqual(["portrait", "token"]);

      // 3. The sheet, from the Players screen.
      await player.getByTestId(`player-hero-sheet-${actorId}`).click();
      await expect(player).toHaveURL(
        new RegExp(`/world/${worldId}/actor/${actorId}/edit$`),
      );
      const sheet = player.getByTestId("dnd5e-actor-sheet");
      await expect(sheet).toBeVisible({ timeout: 15_000 });
      await expect(sheet).toHaveAttribute("data-editable", "true");
      const strength = player.getByTestId("dnd5e-score-strength-input");
      await strength.fill("17");
      await strength.press("Enter");
      await expect
        .poll(() => strengthOf(gm, actorId), { timeout: 15_000 })
        .toBe(17);

      const renamed = `${name} the Bold`;
      await player.locator("#actor-label").fill(renamed);
      await player.getByRole("button", { name: "Save", exact: true }).click();
      await expect(player.getByText(/^saved\.?$/i)).toBeVisible({
        timeout: 10_000,
      });

      // What the Game Master's own screens now show.
      await gm.goto(`/world/${worldId}/players`);
      await expect(gm.getByTestId("players-list")).toContainText(renamed, {
        timeout: 15_000,
      });
      await gm.goto(`/world/${worldId}/actor/${actorId}/view`);
      await expect(gm.getByTestId("dnd5e-score-strength")).toContainText("17", {
        timeout: 15_000,
      });

      // 4. Another player sees the hero on the roster and nothing to change
      // it with, and the server agrees.
      await other.goto(`/world/${worldId}/players`);
      await expect(other.getByTestId("players-list")).toContainText(renamed, {
        timeout: 15_000,
      });
      await expect(
        other.getByTestId(`player-hero-build-${actorId}`),
      ).toHaveCount(0);
      await expect(
        other.getByTestId(`player-hero-sheet-${actorId}`),
      ).toHaveCount(0);
      const refused = await graphql<{ errors?: { message: string }[] }>(
        other,
        `
          mutation ($input: GraphQLUpdateActorSystemDataInput!) {
            updateActorSystemData(input: $input) {
              actorId
            }
          }
        `,
        {
          input: {
            actorId,
            gameSystemId: "dnd5e",
            dataType: "ability_data",
            data: { strength: 3 },
          },
        },
      );
      expect(refused.errors?.length ?? 0).toBeGreaterThan(0);
      const renameRefused = await graphql<{ errors?: { message: string }[] }>(
        other,
        `
          mutation ($input: UpdateActorInput!) {
            updateActor(input: $input) {
              id
            }
          }
        `,
        { input: { actorId, label: "Stolen" } },
      );
      expect(renameRefused.errors?.length ?? 0).toBeGreaterThan(0);
      expect((await heroOf(gm, worldId, actorId))?.label).toBe(renamed);
      expect(await strengthOf(gm, actorId)).toBe(17);
    } finally {
      await player.context().close();
      await other.context().close();
    }
  });
});
