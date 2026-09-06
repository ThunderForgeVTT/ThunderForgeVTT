import { expect, test, type Page } from "@playwright/test";
import path from "node:path";
import {
  createItemViaCompendium,
  createNpcViaCompendium,
} from "./fixtures/content";
import {
  graphql,
  launchSceneByName,
  openDockTab,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";

/**
 * specs/026-content-collections, T034 and T038.
 *
 * One journey, because the claims only mean anything in sequence: author one
 * of each of the five types, gather them, share them, open the link **with no
 * account at all**, sign in, copy into a second Game Master's world, confirm
 * everything arrived, then revoke and confirm the link dies while the copy
 * does not.
 *
 * # The assertion this file exists for
 *
 * `the link opens with no account` is the one that would otherwise ship
 * broken. Every share page already in the product redirects to `/login`
 * first, and `/collection/:shareCode` must not — FR-009a, ADR-070. It is
 * asserted here in a **fresh browser context**, because a context that has
 * ever signed in cannot tell you anything about a visitor who has not.
 */

/**
 * A real map, so the copied scene has a background, walls and lighting to
 * bring with it (SC-008a). Importing one file is far less machinery than
 * uploading an image and drawing walls by hand, and it is the same fixture
 * `canvas-authoring.spec.ts` uses.
 */
const DEMO_MAP = path.resolve(__dirname, "../../../examples/maps/demo.dd2vtt");

type SceneRow = {
  sceneId: string;
  name: string;
  backgroundAssetId: string | null;
};

async function scenesOf(page: Page, worldId: string): Promise<SceneRow[]> {
  const body = await graphql<{ data: { scenes: SceneRow[] } }>(
    page,
    `
      query S($worldId: UUID!) {
        scenes(worldId: $worldId) {
          sceneId
          name
          backgroundAssetId
        }
      }
    `,
    { worldId },
  );
  return body.data.scenes;
}

/**
 * Create a scene, make it the world's active one, and import the demo map
 * into it.
 *
 * The scene has to be created and launched first. Map import imports into
 * whatever scene is active, and the scene a new world starts with is **named
 * after the world** (`create_world_impl`). Importing without this puts the
 * background on that scene, and sharing it then discloses the source world's
 * name through the member's own title — which is not the preview leaking a
 * world field, but reads identically to it in the anonymous view.
 */
async function createSceneAndImportDemoMap(
  page: Page,
  worldId: string,
  sceneName: string,
): Promise<void> {
  await page.goto(`/world/${worldId}/play`);
  await openDockTab(page, "settings");
  const newSceneButton = page.getByTestId("new-scene-button");
  await expect(newSceneButton).toBeVisible({ timeout: 30_000 });
  await newSceneButton.click();
  await page
    .locator('[data-testid="new-scene-name-input"]:visible')
    .fill(sceneName);
  await page.locator('[data-testid="create-scene-submit"]:visible').click();
  await expect(page.getByTestId("new-scene-name-input")).toBeHidden({
    timeout: 15_000,
  });
  await launchSceneByName(page, worldId, sceneName);

  await page.goto(`/world/${worldId}/play`);
  await openDockTab(page, "settings");
  const tool = page.getByTestId("map-import-tool");
  await tool.locator('input[type="file"]').setInputFiles(DEMO_MAP);
  // Generous on purpose: this spec is not measuring import time, and a tight
  // wait here reports "the spinner never stopped" for a slow-but-fine import.
  await expect(page.getByTestId("map-import-success")).toBeVisible({
    timeout: 120_000,
  });
}

/**
 * Add one member of `memberType` to the open collection card.
 *
 * `label` picks a specific one. It matters for scenes: creating a world also
 * creates a default scene **named after the world**
 * (`create_world_impl` in `mutations_worlds.rs`), so taking whichever scene
 * comes first would put the empty default in the collection instead of the
 * imported map — and would drag the world's name into the anonymous preview
 * along with it.
 */
async function addMember(
  page: Page,
  memberType: string,
  label?: string,
): Promise<void> {
  await page.getByLabel("Content type").selectOption(memberType);
  const picker = page.getByLabel("Content to add");
  // The picker repopulates when the type changes; wait for a real option
  // rather than the placeholder before selecting.
  await expect
    .poll(async () => await picker.locator("option").count(), {
      timeout: 15_000,
    })
    .toBeGreaterThan(1);
  await picker.selectOption(label === undefined ? { index: 1 } : { label });
  await page.getByRole("button", { name: "Add", exact: true }).click();
}

test.describe("spec 026: gather a world's content, share it, copy it", () => {
  test("author five types, share, open signed out, copy, then revoke", async ({
    page,
    browser,
  }) => {
    test.setTimeout(600_000);

    // ---- 1. Author one of each of the five types -------------------------
    const suffix = uniqueSuffix();
    const sourceWorldId = await registerAndCreateWorld(
      page,
      `E2E Collections Source ${suffix}`,
      "e2ecolsrc",
    );

    const sceneName = `Manor Ballroom ${suffix}`;
    await createSceneAndImportDemoMap(page, sourceWorldId, sceneName);

    const npcName = `Manor Ghost ${suffix}`;
    await createNpcViaCompendium(page, sourceWorldId, npcName);

    const itemName = `Tarnished Key ${suffix}`;
    await createItemViaCompendium(page, sourceWorldId, itemName);

    const loreTitle = `The Manor's History ${suffix}`;
    await graphql(
      page,
      `
        mutation C($input: CreateLoreEntryInput!) {
          createLoreEntry(input: $input) {
            id
            title
          }
        }
      `,
      {
        input: {
          worldId: sourceWorldId,
          title: loreTitle,
          content: "It was not always empty.",
        },
      },
    );

    const abilityName = `Chill Touch ${suffix}`;
    await graphql(
      page,
      `
        mutation C($input: CreateAbilityInput!) {
          createAbility(input: $input) {
            id
            name
          }
        }
      `,
      {
        input: {
          worldId: sourceWorldId,
          name: abilityName,
          classification: "SPELL",
          // Not GM-only: a GM-only ability is refused by FR-001a, and this
          // journey is about the path that works.
          gmOnly: false,
        },
      },
    );

    // The imported scene is the one carrying a background, walls and lights.
    const sourceScenes = await scenesOf(page, sourceWorldId);
    const imported = sourceScenes.find((s) => s.backgroundAssetId !== null);
    expect(
      imported,
      "the map import should have produced a scene",
    ).toBeTruthy();
    const importedSceneName = imported!.name;

    // ---- 2. Gather them into a collection --------------------------------
    const collectionName = `The Haunted Manor ${suffix}`;
    await page.goto(`/world/${sourceWorldId}/collections`);
    await page.getByLabel("Collection name").fill(collectionName);
    await page
      .getByLabel("Collection description")
      .fill("Everything you need to run it.");
    await page.getByRole("button", { name: "Create collection" }).click();

    await expect(
      page.getByRole("heading", { name: collectionName }),
    ).toBeVisible({ timeout: 15_000 });
    await page.getByRole("button", { name: "Open", exact: true }).click();

    await addMember(page, "scene", importedSceneName);
    for (const memberType of ["actor", "item", "lore", "ability"]) {
      await addMember(page, memberType);
    }
    await expect(
      page.getByTestId("collection-members").locator("li"),
    ).toHaveCount(5, { timeout: 15_000 });

    // ---- 3. Share it -----------------------------------------------------
    //
    // FR-026: the terms are on screen before the button, not behind a link.
    const terms = page.getByTestId("share-terms");
    await expect(terms).toBeVisible();
    await expect(terms).toContainText("responsible for what you share");
    await expect(terms).toContainText("cannot be recalled");

    await page
      .getByRole("button", { name: /I have the right to share this/ })
      .click();
    const shareUrl = await page.getByTestId("share-url").textContent();
    expect(shareUrl).toContain("/collection/");
    const sharePath = new URL(shareUrl!).pathname;

    // ---- 4. Open the link with NO ACCOUNT AT ALL -------------------------
    //
    // FR-009a. A brand-new context, never signed in, never registered. If
    // `/collection/:shareCode` were wrapped in `RequireAuthenticated` like the
    // three older share routes, this lands on `/login` instead.
    const visitorContext = await browser.newContext();
    const visitor = await visitorContext.newPage();
    try {
      await visitor.goto(sharePath);

      await expect(
        visitor.getByRole("heading", { name: collectionName }),
      ).toBeVisible({ timeout: 20_000 });
      expect(
        new URL(visitor.url()).pathname,
        "an anonymous visitor must not be redirected to sign in",
      ).toBe(sharePath);

      // US4 scenario 1: what it will add, before deciding.
      const counts = visitor.getByTestId("collection-counts");
      await expect(counts).toBeVisible();
      for (const label of ["Scene", "Actor", "Item", "Lore entr", "Abilit"]) {
        await expect(counts).toContainText(label);
      }

      // FR-016: nothing about the source world reaches this page.
      //
      // The whole document, not just the card: a world id leaking into a
      // heading, a title or an aria-label would be just as much of a leak,
      // and the server-side test for this greps the entire serialised preview
      // for the same reason.
      const previewText = await visitor.locator("body").innerText();
      expect(previewText).not.toContain(sourceWorldId);
      // The world's name. Nothing in this collection is named after it — the
      // default scene is, which is exactly why the collection holds the
      // imported scene instead. See `addMember`.
      expect(previewText).not.toContain("E2E Collections Source");

      // ---- 5. Sign in as a second Game Master ---------------------------
      //
      // Pressing copy is the one point that needs an account (FR-009b).
      await visitor.getByRole("button", { name: "Copy to a world" }).click();
      await visitor.waitForURL(/\/login\?returnTo=/, { timeout: 15_000 });

      const destinationWorldName = `E2E Collections Destination ${suffix}`;
      const destinationWorldId = await registerAndCreateWorld(
        visitor,
        destinationWorldName,
        "e2ecoldst",
      );

      // ---- 6. Copy it in ------------------------------------------------
      await visitor.goto(sharePath);
      await expect(
        visitor.getByRole("heading", { name: collectionName }),
      ).toBeVisible({ timeout: 20_000 });
      await visitor.getByRole("button", { name: "Copy to a world" }).click();
      await visitor
        .getByLabel("Destination world")
        .selectOption({ label: destinationWorldName });
      await visitor.getByRole("button", { name: "Confirm copy" }).click();

      await expect(
        visitor.getByRole("heading", { name: "Copied" }),
      ).toBeVisible({ timeout: 60_000 });

      // ---- 7. All five arrived ------------------------------------------
      await expect(
        visitor.getByTestId("copy-receipt-created").locator("li"),
      ).toHaveCount(5);

      const receiptText = await visitor
        .getByTestId("copy-receipt-created")
        .innerText();
      expect(receiptText).toContain(npcName);
      expect(receiptText).toContain(itemName);
      expect(receiptText).toContain(loreTitle);
      expect(receiptText).toContain(abilityName);

      // SC-008a: the copied scene has its background, walls and lighting in
      // the destination world, and the source world is nowhere in the query.
      const copiedScenes = await scenesOf(visitor, destinationWorldId);
      const withBackground = copiedScenes.filter(
        (s) => s.backgroundAssetId !== null,
      );
      expect(
        withBackground.length,
        "the copied scene must carry its background",
      ).toBeGreaterThan(0);

      const geometry = await graphql<{
        data: {
          walls: { wallId: string }[];
          lightSources: { lightId: string }[];
        };
      }>(
        visitor,
        `
          query G($sceneId: UUID!) {
            walls(sceneId: $sceneId) {
              wallId
            }
            lightSources(sceneId: $sceneId) {
              lightId
            }
          }
        `,
        { sceneId: withBackground[0].sceneId },
      );
      expect(
        geometry.data.walls.length,
        "walls come with the place (SC-008a)",
      ).toBeGreaterThan(0);
      expect(
        geometry.data.lightSources.length,
        "lighting comes with the place (SC-008a)",
      ).toBeGreaterThan(0);

      // ---- 8. Revoke, and the link dies within one page load -------------
      //
      // T038 / SC-005, now through the flow FR-010a made possible.
      //
      // The author's page is reloaded first, deliberately. Before FR-010a the
      // link was displayed once and never again, so revoking only worked in
      // the session that minted it — closing the tab removed the ability for
      // good. Reloading here is the assertion that this is fixed: the link
      // comes back on its own, and it is revocable from a fresh page.
      await page.reload();
      await page.getByRole("button", { name: "Open", exact: true }).click();
      await expect(page.getByTestId("share-url")).toContainText(
        "/collection/",
        { timeout: 15_000 },
      );
      await page.getByRole("button", { name: "Revoke link" }).click();

      // FR-011: the warning has to be here, at the moment of revoking.
      const confirm = page.getByTestId("revoke-confirm");
      await expect(confirm).toBeVisible();
      await expect(confirm).toContainText("Copies already made are not");

      await page.getByRole("button", { name: "Revoke the link" }).click();

      await visitor.goto(sharePath);
      await expect(
        visitor.getByRole("heading", { name: collectionName }),
      ).toHaveCount(0, { timeout: 20_000 });

      // ---- 9. The copy is untouched --------------------------------------
      const stillThere = await scenesOf(visitor, destinationWorldId);
      expect(
        stillThere.filter((s) => s.backgroundAssetId !== null).length,
        "revoking a link must not reach into a copy already taken",
      ).toBeGreaterThan(0);
    } finally {
      await visitorContext.close();
    }
  });
});
