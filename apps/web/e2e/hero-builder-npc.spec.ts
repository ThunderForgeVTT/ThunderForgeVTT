import { createHash } from "node:crypto";
import { expect, test, type Page } from "./fixtures/test";
import { openAdminPage } from "./fixtures/admin";
import {
  graphql,
  inviteAndJoinAsPlayer,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";
import {
  liftPauseAsOperator,
  pauseWorldAsOperator,
} from "./fixtures/playPause";
import {
  joinPlayer,
  openDarkDnd5eScene,
  placeCreature,
} from "./fixtures/visionTable";
import { must } from "../playtest/table";
import {
  HERO_PARTS,
  PRESET_HEROES,
  renderToken,
} from "../../../packages/heroes/src/index.ts";

/**
 * Spec 044, US3 and US4: "Build look" where a Game Master already is.
 *
 * The owner asked (2026-09-15) for the builder under an NPC's edit page, and
 * then for it on each compendium row and as a quick way to a named NPC with a
 * face. Every save here is checked against the server's answer, not the
 * dialog's word for it: the rows `worldActors { images }` returns, and the
 * bytes served for them.
 */

interface ImageRow {
  role: string;
  url: string;
  assetId: string;
}

/** Every image of every actor in the world, as the server stores them. */
async function imagesOf(
  page: Page,
  worldId: string,
): Promise<Record<string, ImageRow[]>> {
  const { worldActors } = await must<{
    worldActors: { id: string; images: ImageRow[] }[];
  }>(
    page,
    `query ($worldId: UUID!) {
      worldActors(worldId: $worldId) { id images { role url assetId } }
    }`,
    { worldId },
  );
  return Object.fromEntries(worldActors.map((a) => [a.id, a.images]));
}

async function npc(
  page: Page,
  worldId: string,
  label: string,
  gameSystemId?: string,
): Promise<string> {
  const { createActor } = await must<{ createActor: { id: string } }>(
    page,
    `mutation ($input: CreateActorInput!) { createActor(input: $input) { id } }`,
    { input: { worldId, label, isNpc: true, gameSystemId } },
  );
  return createActor.id;
}

async function useDnd5e(page: Page, worldId: string): Promise<void> {
  await must(
    page,
    `mutation ($input: UpdateWorldGameSystemInput!) {
      updateWorldGameSystem(input: $input) { id }
    }`,
    { input: { worldId, gameSystemId: "dnd5e" } },
  );
}

async function setRace(page: Page, actorId: string, race: string) {
  await must(
    page,
    `mutation ($input: GraphQLUpdateActorSystemDataInput!) {
      updateActorSystemData(input: $input) { id }
    }`,
    {
      input: {
        actorId,
        gameSystemId: "dnd5e",
        dataType: "trait_data",
        // The slot's validator wants its other strings present too.
        data: { class: "Wizard", level: 1, race },
      },
    },
  );
}

/** The served bytes are WebP, whatever the builder uploaded (SC-007). */
async function expectServedWebp(page: Page, url: string): Promise<Buffer> {
  const response = await page.request.get(url);
  expect(response.status()).toBe(200);
  expect(response.headers()["content-type"]).toBe("image/webp");
  const bytes = await response.body();
  expect(bytes.subarray(0, 4).toString("ascii")).toBe("RIFF");
  expect(bytes.subarray(8, 12).toString("ascii")).toBe("WEBP");
  return bytes;
}

/** The builder dialog, open and past reading the sheet. */
async function openedBuilder(page: Page) {
  const dialog = page.getByTestId("hero-builder-dialog");
  await expect(dialog).toBeVisible({ timeout: 15_000 });
  await expect(dialog.getByTestId("hero-builder")).toBeVisible({
    timeout: 15_000,
  });
  return dialog;
}

/** Answer the next `uploadActorImage` for `role` with a refusal, once. */
async function failNextUpload(page: Page, role: string): Promise<void> {
  let failed = false;
  await page.route("**/api/graphql", async (route) => {
    const body = route.request().postDataBuffer()?.toString("utf8") ?? "";
    if (
      !failed &&
      body.includes("uploadActorImage") &&
      body.includes(`"role":"${role}"`)
    ) {
      failed = true;
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: null,
          errors: [{ message: "The image store did not answer." }],
        }),
      });
      return;
    }
    await route.fallback();
  });
}

const OTHER_HAIR = HERO_PARTS.hair[1];

test.describe("Build look on an NPC's edit page (US3)", () => {
  test("a Game Master builds a look in a full-screen dialog and both roles are stored as WebP", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Build Look ${uniqueSuffix()}`,
    );
    const actorId = await npc(page, worldId, "Mirelda");
    const editUrl = `/world/${worldId}/compendium/npc/${actorId}/edit`;
    await page.goto(editUrl);
    const started = Date.now();

    await page.getByTestId("actor-imagery-build").click();
    const dialog = await openedBuilder(page);

    // Full-screen, over the page, and the URL never changed (FR-019).
    // Polled: the dialog grows in from the middle, so the first frames
    // measure a box a little inside the viewport.
    const viewport = page.viewportSize()!;
    await expect
      .poll(async () => {
        const box = await dialog.boundingBox();
        return box && [box.x, box.y, box.width, box.height].map(Math.round);
      })
      .toEqual([0, 0, viewport.width, viewport.height]);
    expect(new URL(page.url()).pathname).toBe(editUrl);

    // It opened on the NPC's name (FR-023).
    await expect(dialog.getByTestId("hero-text-name")).toHaveValue("Mirelda");

    await dialog.getByTestId(`hero-choice-hair-${OTHER_HAIR}`).check();
    await dialog.getByTestId("hero-dialog-save").click();
    await expect(dialog).toBeHidden({ timeout: 20_000 });
    console.log(`SC-007: edit page to saved look in ${Date.now() - started}ms`);

    // Both roles show in the panel without a reload.
    for (const role of ["portrait", "token"]) {
      await expect(
        page.getByTestId(`actor-imagery-preview-${role}`),
      ).toBeVisible({ timeout: 15_000 });
    }
    await expect(page.getByTestId("actor-imagery-status")).toContainText(
      "Portrait and token saved.",
    );
    expect(new URL(page.url()).pathname).toBe(editUrl);

    const stored = (await imagesOf(page, worldId))[actorId];
    expect(stored.map((row) => row.role).sort()).toEqual(["portrait", "token"]);
    for (const row of stored) await expectServedWebp(page, row.url);
  });

  test("a row's Build look saves both roles and leaves the Game Master on the list; a Player has no row control (FR-028a)", async ({
    page,
    browser,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Row Build ${uniqueSuffix()}`,
    );
    const actorId = await npc(page, worldId, "Tobble");
    await must(
      page,
      `mutation ($actorId: UUID!) {
        setActorVisibleToPlayers(actorId: $actorId, visible: true) { id }
      }`,
      { actorId },
    );

    const listUrl = `/world/${worldId}/compendium?tab=npcs`;
    await page.goto(listUrl);
    await expect(page.getByTestId(`npc-catalog-row-${actorId}`)).toBeVisible({
      timeout: 15_000,
    });
    // No art yet, and the list says so beside the control that fixes it.
    await expect(
      page.getByTestId(`npc-catalog-lacks-art-${actorId}`),
    ).toBeVisible();

    await page.getByTestId(`npc-catalog-build-${actorId}`).click();
    const dialog = await openedBuilder(page);
    await expect(dialog.getByTestId("hero-text-name")).toHaveValue("Tobble");
    await dialog.getByTestId("hero-dialog-save").click();
    await expect(dialog).toBeHidden({ timeout: 20_000 });

    expect(new URL(page.url()).pathname + new URL(page.url()).search).toBe(
      listUrl,
    );
    const portrait = page.getByTestId(`npc-catalog-portrait-${actorId}`);
    await expect(portrait).toBeVisible({ timeout: 15_000 });
    const stored = (await imagesOf(page, worldId))[actorId];
    // The row shows the stored portrait (as its thumbnail).
    expect(await portrait.getAttribute("src")).toContain(
      stored.find((row) => row.role === "portrait")!.assetId,
    );
    await expect(
      page.getByTestId(`npc-catalog-lacks-art-${actorId}`),
    ).toHaveCount(0);

    const player = await inviteAndJoinAsPlayer(browser, page, worldId);
    try {
      await player.goto(listUrl);
      await expect(
        player.getByTestId(`npc-catalog-row-${actorId}`),
      ).toBeVisible({ timeout: 15_000 });
      await expect(
        player.getByTestId(`npc-catalog-build-${actorId}`),
      ).toHaveCount(0);
      await expect(player.getByTestId("quick-npc")).toHaveCount(0);
    } finally {
      await player.context().close();
    }
  });

  test("the dice roll by the sheet's race: High Elf opens on elf, Moonkin and a Genie NPC on any (FR-007a)", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Race Dice ${uniqueSuffix()}`,
    );
    await useDnd5e(page, worldId);
    const elf = await npc(page, worldId, "Aelar", "dnd5e");
    await setRace(page, elf, "High Elf");
    const moonkin = await npc(page, worldId, "Hootle", "dnd5e");
    await setRace(page, moonkin, "Moonkin");

    await page.goto(`/world/${worldId}/compendium/npc/${elf}/edit`);
    await page.getByTestId("actor-imagery-build").click();
    let dialog = await openedBuilder(page);
    await expect(dialog.getByTestId("hero-race")).toHaveValue("elf");
    for (let roll = 0; roll < 3; roll += 1) {
      await dialog.getByTestId("hero-roll").click();
      await expect(
        dialog.getByTestId("hero-choice-ears-pointed"),
      ).toBeChecked();
    }
    await dialog.getByTestId("hero-dialog-close").click();
    await expect(dialog).toBeHidden();

    await page.goto(`/world/${worldId}/compendium/npc/${moonkin}/edit`);
    await page.getByTestId("actor-imagery-build").click();
    dialog = await openedBuilder(page);
    await expect(dialog.getByTestId("hero-race")).toHaveValue("");
    await dialog.getByTestId("hero-dialog-close").click();

    // A second world on the same account, left on the Genie default.
    const { createWorld } = await must<{ createWorld: { id: string } }>(
      page,
      `mutation ($input: GraphQLCreateWorldInput!) {
        createWorld(input: $input) { id }
      }`,
      { input: { name: `E2E Genie Race ${uniqueSuffix()}` } },
    );
    const genieWorld = createWorld.id;
    const genie = await npc(page, genieWorld, "Zephyra");
    await page.goto(`/world/${genieWorld}/compendium/npc/${genie}/edit`);
    await page.getByTestId("actor-imagery-build").click();
    dialog = await openedBuilder(page);
    await expect(dialog.getByTestId("hero-race")).toHaveValue("");
  });

  test("replacing art warns first, a failed role retries alone, closing keeps the art, and a paused world keeps the hero (FR-025)", async ({
    page,
    browser,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Build Failures ${uniqueSuffix()}`,
    );
    const actorId = await npc(page, worldId, "Grom");
    const editUrl = `/world/${worldId}/compendium/npc/${actorId}/edit`;
    await page.goto(editUrl);

    // Give it art the ordinary way first.
    const grom = PRESET_HEROES.find((preset) => preset.slug === "grom")!;
    await page.getByTestId("actor-imagery-input-token").setInputFiles({
      name: "grom.svg",
      mimeType: "image/svg+xml",
      buffer: Buffer.from(renderToken(grom.spec)),
    });
    await expect(page.getByTestId("actor-imagery-preview-token")).toBeVisible({
      timeout: 15_000,
    });
    const before = (await imagesOf(page, worldId))[actorId];

    // Closing without saving leaves the images as they were.
    await page.getByTestId("actor-imagery-build").click();
    let dialog = await openedBuilder(page);
    await dialog.getByTestId(`hero-choice-hair-${OTHER_HAIR}`).check();
    await dialog.getByTestId("hero-dialog-close").click();
    await expect(dialog).toBeHidden();
    expect((await imagesOf(page, worldId))[actorId]).toEqual(before);

    // Saving over art says so first; keeping it stores nothing.
    await page.getByTestId("actor-imagery-build").click();
    dialog = await openedBuilder(page);
    await dialog.getByTestId("hero-dialog-save").click();
    await expect(
      dialog.getByTestId("hero-dialog-replace-warning"),
    ).toBeVisible();
    await dialog.getByTestId("hero-dialog-cancel-replace").click();
    expect((await imagesOf(page, worldId))[actorId]).toEqual(before);

    // One role fails: that role is reported, the other saved, and the
    // failed one retries alone.
    await failNextUpload(page, "token");
    await dialog.getByTestId("hero-dialog-save").click();
    await dialog.getByTestId("hero-dialog-confirm-replace").click();
    await expect(
      dialog.getByTestId("hero-dialog-result-portrait"),
    ).toHaveAttribute("data-status", "saved", { timeout: 20_000 });
    await expect(
      dialog.getByTestId("hero-dialog-result-token"),
    ).toHaveAttribute("data-status", "failed");
    await expect(dialog.getByTestId("hero-dialog-retry-portrait")).toHaveCount(
      0,
    );
    const halfway = (await imagesOf(page, worldId))[actorId];
    expect(halfway.find((r) => r.role === "token")).toEqual(
      before.find((r) => r.role === "token"),
    );
    expect(halfway.find((r) => r.role === "portrait")).toBeTruthy();

    await dialog.getByTestId("hero-dialog-retry-token").click();
    await expect(dialog).toBeHidden({ timeout: 20_000 });
    const after = (await imagesOf(page, worldId))[actorId];
    expect(after.find((r) => r.role === "token")!.assetId).not.toBe(
      before.find((r) => r.role === "token")!.assetId,
    );
    expect(after.find((r) => r.role === "portrait")).toEqual(
      halfway.find((r) => r.role === "portrait"),
    );

    // A paused world: the save is refused, nothing stored, the hero kept.
    await page.getByTestId("actor-imagery-build").click();
    dialog = await openedBuilder(page);
    await dialog.getByTestId("hero-text-name").fill("Grom the Paused");
    const admin = await openAdminPage(browser);
    const { pause } = await pauseWorldAsOperator(
      admin,
      worldId,
      "e2e: a builder save while paused",
    );
    try {
      await dialog.getByTestId("hero-dialog-save").click();
      await dialog.getByTestId("hero-dialog-confirm-replace").click();
      await expect(dialog.getByTestId("hero-dialog-paused")).toBeVisible({
        timeout: 20_000,
      });
      await expect(dialog.getByTestId("hero-text-name")).toHaveValue(
        "Grom the Paused",
      );
      expect(new URL(page.url()).pathname).toBe(editUrl);
      expect((await imagesOf(page, worldId))[actorId]).toEqual(after);
    } finally {
      await liftPauseAsOperator(admin, pause.id, "e2e: done");
      await admin.context().close();
    }
  });

  test("a Viewer is offered no build or save, and a direct upload is refused (B5)", async ({
    page,
    browser,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Build Viewer ${uniqueSuffix()}`,
    );
    const actorId = await npc(page, worldId, "Quill");
    await must(
      page,
      `mutation ($actorId: UUID!) {
        setActorVisibleToPlayers(actorId: $actorId, visible: true) { id }
      }`,
      { actorId },
    );
    const player = await inviteAndJoinAsPlayer(browser, page, worldId);
    try {
      await player.goto(`/world/${worldId}/compendium/npc/${actorId}/edit`);
      await player.waitForLoadState("networkidle");
      await expect(player.getByTestId("actor-imagery-build")).toHaveCount(0);
      await expect(player.getByTestId("hero-dialog-save")).toHaveCount(0);

      const csrf = (await player.context().cookies()).find(
        (cookie) => cookie.name === "csrf_token",
      )?.value;
      const grom = PRESET_HEROES.find((preset) => preset.slug === "grom")!;
      const response = await player.request.post("/api/graphql", {
        headers: csrf ? { "x-csrf-token": csrf } : {},
        multipart: {
          operations: JSON.stringify({
            query: `mutation ($actorId: UUID!, $role: String!, $file: Upload!) {
              uploadActorImage(actorId: $actorId, role: $role, file: $file) { id }
            }`,
            variables: { actorId, role: "portrait", file: null },
          }),
          map: JSON.stringify({ "0": ["variables.file"] }),
          "0": {
            name: "hero.svg",
            mimeType: "image/svg+xml",
            buffer: Buffer.from(renderToken(grom.spec)),
          },
        },
      });
      const answer = (await response.json()) as {
        data?: { uploadActorImage?: unknown } | null;
        errors?: unknown[];
      };
      expect(answer.data?.uploadActorImage ?? null).toBeNull();
      expect(answer.errors?.length ?? 0).toBeGreaterThan(0);
      expect((await imagesOf(page, worldId))[actorId]).toEqual([]);
    } finally {
      await player.context().close();
    }
  });
});

test.describe("Quick NPC (US4)", () => {
  test("three named NPCs with different faces from an empty compendium; reroll, Open in builder, and a failed upload", async ({
    page,
    browser,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Quick NPC ${uniqueSuffix()}`,
    );
    await page.goto(`/world/${worldId}/compendium?tab=npcs`);
    await expect(page.getByTestId("quick-npc")).toBeVisible({
      timeout: 15_000,
    });

    const quickDialog = page.getByTestId("quick-npc-dialog");
    const previewHtml = () =>
      quickDialog
        .getByTestId("quick-npc-preview")
        .getByTestId("hero-preview-portrait")
        .innerHTML();

    const names = ["Alda", "Brisk", "Corvin"];
    for (const [index, name] of names.entries()) {
      const started = Date.now();
      await page.getByTestId("quick-npc").click();
      await expect(quickDialog).toBeVisible({ timeout: 15_000 });
      await quickDialog.getByTestId("quick-npc-name").fill(name);

      if (index === 0) {
        // Reroll changes the face.
        const first = await previewHtml();
        await quickDialog.getByTestId("quick-npc-reroll").click();
        await expect.poll(previewHtml).not.toBe(first);

        // Open in builder carries the current hero, and its choice comes back.
        const carried = await previewHtml();
        await quickDialog.getByTestId("quick-npc-open-builder").click();
        const builder = await openedBuilder(page);
        await expect(builder.getByTestId("hero-text-name")).toHaveValue(name);
        const strip = (html: string) =>
          html.replace(/hb[a-zA-Z0-9_-]*?-|quick-npc-/g, "");
        expect(
          strip(await builder.getByTestId("hero-preview-portrait").innerHTML()),
        ).toBe(strip(carried));
        await builder.getByTestId(`hero-choice-hair-${OTHER_HAIR}`).check();
        await builder.getByTestId("hero-dialog-use").click();
        await expect(builder).toBeHidden();
        await expect.poll(previewHtml).not.toBe(carried);
      } else {
        await quickDialog.getByTestId("quick-npc-reroll").click();
      }

      await quickDialog.getByTestId("quick-npc-create").click();
      await expect(quickDialog).toBeHidden({ timeout: 20_000 });
      console.log(`SC-008: Quick NPC ${name} in ${Date.now() - started}ms`);
      await expect(page.getByTestId("npc-catalog-table")).toContainText(name);
    }

    const { worldActors } = await must<{
      worldActors: { id: string; label: string; images: ImageRow[] }[];
    }>(
      page,
      `query ($worldId: UUID!) {
        worldActors(worldId: $worldId) { id label images { role url assetId } }
      }`,
      { worldId },
    );
    const made = worldActors.filter((actor) => names.includes(actor.label));
    expect(made.map((actor) => actor.label).sort()).toEqual(names);
    const faces = new Set<string>();
    for (const actor of made) {
      expect(actor.images.map((row) => row.role).sort()).toEqual([
        "portrait",
        "token",
      ]);
      const portrait = actor.images.find((row) => row.role === "portrait")!;
      const bytes = await expectServedWebp(page, portrait.url);
      faces.add(createHash("sha256").update(bytes).digest("hex"));
      await expect(
        page.getByTestId(`npc-catalog-portrait-${actor.id}`),
      ).toBeVisible();
    }
    expect(faces.size).toBe(3);

    // A failed upload keeps the NPC, lists it as lacking art, and offers the
    // row's Build look (FR-028).
    await failNextUpload(page, "portrait");
    await page.getByTestId("quick-npc").click();
    await quickDialog.getByTestId("quick-npc-name").fill("Dorn");
    await quickDialog.getByTestId("quick-npc-create").click();
    await expect(quickDialog.getByTestId("quick-npc-lacks-art")).toBeVisible({
      timeout: 20_000,
    });
    await quickDialog.getByTestId("quick-npc-close").click();
    await expect(quickDialog).toBeHidden();
    const dorn = (
      await must<{ worldActors: { id: string; label: string }[] }>(
        page,
        `query ($worldId: UUID!) { worldActors(worldId: $worldId) { id label } }`,
        { worldId },
      )
    ).worldActors.find((actor) => actor.label === "Dorn")!;
    expect(dorn).toBeTruthy();
    await expect(
      page.getByTestId(`npc-catalog-lacks-art-${dorn.id}`),
    ).toBeVisible();
    await expect(
      page.getByTestId(`npc-catalog-build-${dorn.id}`),
    ).toBeVisible();

    const player = await inviteAndJoinAsPlayer(browser, page, worldId);
    try {
      await player.goto(`/world/${worldId}/compendium?tab=npcs`);
      await expect(player.getByTestId("world-compendium-page")).toBeVisible({
        timeout: 15_000,
      });
      await expect(player.getByTestId("quick-npc")).toHaveCount(0);
    } finally {
      await player.context().close();
    }
  });
});

test.describe("What a built look must not weaken", () => {
  test("a built NPC placed twice wears its face on both tokens, and a hidden NPC's portrait stays hidden while its token art is served", async ({
    page,
    browser,
  }) => {
    const table = await openDarkDnd5eScene(page, "Built Faces");
    const { actorId } = await placeCreature(table, {
      label: "Veiled Hag",
      x: 100,
      y: 100,
    });
    await placeCreature(table, { label: "Other", x: 300, y: 300 });

    await page.goto(`/world/${table.worldId}/compendium/npc/${actorId}/edit`);
    await page.getByTestId("actor-imagery-build").click();
    const dialog = await openedBuilder(page);
    await dialog.getByTestId("hero-dialog-save").click();
    await expect(dialog).toBeHidden({ timeout: 20_000 });

    // A second token of the same actor.
    await must(
      page,
      `mutation ($input: GraphQLCreateTokenInput!) {
        createToken(input: $input) { tokenId }
      }`,
      {
        input: {
          sceneId: table.sceneId,
          actorId,
          x: 200,
          y: 100,
          tokenType: "npc",
        },
      },
    );

    const images = (await imagesOf(page, table.worldId))[actorId];
    const token = images.find((row) => row.role === "token")!;
    const portrait = images.find((row) => row.role === "portrait")!;

    const tokensOf = async (viewer: Page) =>
      (
        await graphql<{
          data: {
            tokens: { actorId: string | null; photoUrl: string | null }[];
          };
        }>(
          viewer,
          `
            query ($sceneId: UUID!) {
              tokens(sceneId: $sceneId) {
                actorId
                photoUrl
              }
            }
          `,
          { sceneId: table.sceneId },
        )
      ).data.tokens.filter((t) => t.actorId === actorId);

    const placed = await tokensOf(page);
    expect(placed).toHaveLength(2);
    expect(placed[0].photoUrl).toContain(token.assetId);
    expect(placed[1].photoUrl).toBe(placed[0].photoUrl);

    // The NPC was never shown to players.
    const { page: player } = await joinPlayer(browser, table);
    try {
      expect((await player.request.get(portrait.url)).status()).toBe(403);
      const served = await player.request.get(token.url);
      expect(served.status()).toBe(200);
      expect(served.headers()["content-type"]).toBe("image/webp");
    } finally {
      await player.context().close();
    }
  });
});
