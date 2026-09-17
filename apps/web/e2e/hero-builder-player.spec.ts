import { expect, test, type Page } from "./fixtures/test";
import {
  graphql,
  inviteAndJoinAsPlayer,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";
import { must } from "../playtest/table";
import {
  PRESET_HEROES,
  renderToken,
} from "../../../packages/heroes/src/index.ts";

/**
 * Spec 044 US5, phase (c): a player's own character.
 *
 * The player holding a character may change its look and nothing else; the
 * Game Master may withdraw that for the world or for one character, and is
 * untouched by either. Every refusal is tried twice (SC-010): by looking for
 * the control, and by calling the mutations directly as that player — a
 * hidden button proves nothing about the server.
 *
 * A world member holds at most one character (`world_actor_claims` is unique
 * on the member), so "another character unaffected by a lock" is a second
 * player's character.
 */

const NOT_HOLDER =
  "Only the player who holds this character may change its art.";
const WORLD_OFF =
  "The Game Master has turned off players changing their character's art in this world.";
const LOCKED = "The Game Master has locked this character's look.";

interface ImageRow {
  role: string;
  assetId: string;
}

async function imagesOf(gm: Page, worldId: string, actorId: string) {
  const { worldActors } = await must<{
    worldActors: { id: string; images: ImageRow[] }[];
  }>(
    gm,
    `query ($worldId: UUID!) {
      worldActors(worldId: $worldId) { id images { role assetId } }
    }`,
    { worldId },
  );
  return worldActors.find((a) => a.id === actorId)?.images ?? [];
}

/**
 * Both imagery mutations, sent as `player`. Returns each one's refusal
 * message, or null when it was allowed.
 */
async function tryImageryDirectly(
  player: Page,
  actorId: string,
): Promise<{ upload: string | null; remove: string | null }> {
  const csrf = (await player.context().cookies()).find(
    (cookie) => cookie.name === "csrf_token",
  )?.value;
  const grom = PRESET_HEROES.find((preset) => preset.slug === "grom")!;
  const uploaded = await player.request.post("/api/graphql", {
    headers: csrf ? { "x-csrf-token": csrf } : {},
    multipart: {
      operations: JSON.stringify({
        query: `mutation ($actorId: UUID!, $role: String!, $file: Upload!) {
          uploadActorImage(actorId: $actorId, role: $role, file: $file) { id }
        }`,
        variables: { actorId, role: "token", file: null },
      }),
      map: JSON.stringify({ "0": ["variables.file"] }),
      "0": {
        name: "hero.svg",
        mimeType: "image/svg+xml",
        buffer: Buffer.from(renderToken(grom.spec)),
      },
    },
  });
  const upload = (await uploaded.json()) as {
    errors?: { message: string }[];
  };
  const remove = await graphql<{ errors?: { message: string }[] }>(
    player,
    `
      mutation ($actorId: UUID!) {
        removeActorImage(actorId: $actorId, role: "portrait")
      }
    `,
    { actorId },
  );
  return {
    upload: upload.errors?.[0]?.message ?? null,
    remove: remove.errors?.[0]?.message ?? null,
  };
}

/** Refused both ways with `message`: no control on the page, and the
 *  server's own words from both mutations. */
async function expectRefused(
  player: Page,
  worldId: string,
  actorId: string,
  message: string,
  { onPage }: { onPage: boolean },
) {
  await player.goto(`/world/${worldId}/actor/${actorId}/view`);
  await player.waitForLoadState("networkidle");
  await expect(player.getByTestId("actor-imagery-build")).toHaveCount(0);
  await expect(player.getByTestId("actor-imagery-input-portrait")).toHaveCount(
    0,
  );
  if (onPage) {
    await expect(player.getByTestId("actor-imagery-refusal")).toHaveText(
      message,
    );
  }
  expect(await tryImageryDirectly(player, actorId)).toEqual({
    upload: message,
    remove: message,
  });
}

/** Offered the controls on the view page. */
async function expectOffered(player: Page, worldId: string, actorId: string) {
  await player.goto(`/world/${worldId}/actor/${actorId}/view`);
  await expect(player.getByTestId("actor-imagery-build")).toBeVisible({
    timeout: 15_000,
  });
  await expect(player.getByTestId("actor-imagery-refusal")).toHaveCount(0);
}

async function openedBuilder(page: Page) {
  const dialog = page.getByTestId("hero-builder-dialog");
  await expect(dialog).toBeVisible({ timeout: 15_000 });
  await expect(dialog.getByTestId("hero-builder")).toBeVisible({
    timeout: 15_000,
  });
  return dialog;
}

test.describe("A player's own character (US5)", () => {
  test("the holder builds a look, and every withdrawal refuses them with its own words while the Game Master is untouched", async ({
    page: gm,
    browser,
  }) => {
    test.setTimeout(240_000);
    const worldId = await registerAndCreateWorld(
      gm,
      `E2E Player Look ${uniqueSuffix()}`,
    );
    await must(
      gm,
      `mutation ($input: UpdateWorldAllowPlayerCreatedActorsInput!) {
        updateWorldAllowPlayerCreatedActors(input: $input) { id }
      }`,
      { input: { worldId, allow: true } },
    );

    const mirela = await inviteAndJoinAsPlayer(browser, gm, worldId);
    const tobin = await inviteAndJoinAsPlayer(browser, gm, worldId);
    try {
      // FR-034: "Create your own character" builds a look, then creates.
      await mirela.goto(`/world/${worldId}/actor-select`);
      await mirela.locator("#new-character-name").fill("Mirela");
      await mirela.getByTestId("create-own-build-look").click();
      const dialog = await openedBuilder(mirela);
      await dialog.getByTestId("hero-dialog-use").click();
      await expect(dialog).toBeHidden();
      await expect(mirela.getByTestId("create-own-look-state")).toContainText(
        "will be stored",
      );
      await mirela.getByTestId("create-own-submit").click();
      await mirela.waitForURL(new RegExp(`/world/${worldId}$`), {
        timeout: 20_000,
      });

      const { myActorClaim } = await must<{
        myActorClaim: { actorId: string; worldMemberId: string } | null;
      }>(
        mirela,
        `query ($worldId: UUID!) { myActorClaim(worldId: $worldId) { actorId worldMemberId } }`,
        { worldId },
      );
      const mirelaActor = myActorClaim!.actorId;
      const built = await imagesOf(gm, worldId, mirelaActor);
      expect(built.map((row) => row.role).sort()).toEqual([
        "portrait",
        "token",
      ]);

      const { createAndClaimActor } = await must<{
        createAndClaimActor: { actorId: string };
      }>(
        tobin,
        `mutation ($worldId: UUID!) {
          createAndClaimActor(worldId: $worldId, name: "Tobin") { actorId }
        }`,
        { worldId },
      );
      const tobinActor = createAndClaimActor.actorId;

      // FR-030: the holder is offered the controls on the view page, which
      // is the only page they reach (a Viewer is sent away from /edit).
      await expectOffered(mirela, worldId, mirelaActor);
      expect(await tryImageryDirectly(mirela, mirelaActor)).toEqual({
        upload: null,
        remove: null,
      });

      // Another player is not the holder.
      expect(await tryImageryDirectly(tobin, mirelaActor)).toEqual({
        upload: NOT_HOLDER,
        remove: NOT_HOLDER,
      });

      // FR-030a: the Game Master turns the world setting off, on the world's
      // settings, and every holder is refused.
      await gm.goto(`/world/${worldId}`);
      const worldToggle = gm
        .getByTestId("allow-player-actor-art-toggle")
        .locator("input");
      await expect(worldToggle).toBeChecked({ timeout: 15_000 });
      await worldToggle.click();
      await expect(worldToggle).not.toBeChecked();
      await expect(worldToggle).toBeEnabled();
      await expectRefused(mirela, worldId, mirelaActor, WORLD_OFF, {
        onPage: true,
      });
      await expectRefused(tobin, worldId, tobinActor, WORLD_OFF, {
        onPage: true,
      });
      await worldToggle.click();
      await expect(worldToggle).toBeEnabled();
      await expectOffered(mirela, worldId, mirelaActor);

      // FR-030b: a lock refuses that character only.
      await gm.goto(`/world/${worldId}/actor/${mirelaActor}/view`);
      const lock = gm.getByRole("checkbox", {
        name: "Lock this character's look (portrait and token)",
      });
      await expect(lock).not.toBeChecked({ timeout: 15_000 });
      await lock.click();
      await expect(lock).toBeChecked();
      await expect(lock).toBeEnabled();
      await expectRefused(mirela, worldId, mirelaActor, LOCKED, {
        onPage: true,
      });
      await expectOffered(tobin, worldId, tobinActor);
      expect(await tryImageryDirectly(tobin, tobinActor)).toEqual({
        upload: null,
        remove: null,
      });

      // FR-032: the Game Master replaces the locked character's art.
      const beforeGm = new Set(
        (await imagesOf(gm, worldId, mirelaActor)).map((row) => row.assetId),
      );
      await gm.goto(`/world/${worldId}/actor/${mirelaActor}/view`);
      await gm.getByTestId("actor-imagery-build").click();
      const gmDialog = await openedBuilder(gm);
      await gmDialog.getByTestId("hero-dialog-save").click();
      const confirm = gmDialog.getByTestId("hero-dialog-confirm-replace");
      if (await confirm.isVisible().catch(() => false)) await confirm.click();
      await expect(gmDialog).toBeHidden({ timeout: 20_000 });
      const replaced = await imagesOf(gm, worldId, mirelaActor);
      expect(replaced.map((row) => row.role).sort()).toEqual([
        "portrait",
        "token",
      ]);
      expect(replaced.some((row) => beforeGm.has(row.assetId))).toBe(false);

      // Unlocked, then released: no longer the holder.
      await lock.click();
      await expect(lock).not.toBeChecked();
      await expect(lock).toBeEnabled();
      await expectOffered(mirela, worldId, mirelaActor);
      await must(
        gm,
        `mutation ($actorId: UUID!) { unclaimActor(actorId: $actorId) { id } }`,
        { actorId: mirelaActor },
      );
      expect(await tryImageryDirectly(mirela, mirelaActor)).toEqual({
        upload: NOT_HOLDER,
        remove: NOT_HOLDER,
      });
      await mirela.goto(`/world/${worldId}/actor/${mirelaActor}/view`);
      await mirela.waitForLoadState("networkidle");
      await expect(mirela.getByTestId("actor-imagery-build")).toHaveCount(0);

      // The grant never reached the sheet: a holder may not rename.
      const renamed = await graphql<{ errors?: unknown[] }>(
        tobin,
        `
          mutation ($input: UpdateActorInput!) {
            updateActor(input: $input) {
              id
            }
          }
        `,
        { input: { actorId: tobinActor, label: "Renamed" } },
      );
      expect(renamed.errors?.length ?? 0).toBeGreaterThan(0);
    } finally {
      await mirela.context().close();
      await tobin.context().close();
    }
  });
});
