import { execFileSync } from "node:child_process";
import { expect, test, type Locator, type Page } from "./fixtures/test";
import {
  currentSharingTermsVersion,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";
import { must } from "../playtest/table";
import {
  HERO_COLORS,
  HERO_FLAGS,
  HERO_PARTS,
  minimalSpec,
  PRESET_HEROES,
  renderToken,
  SIZES,
  validateHero,
  type HeroSpec,
  type ResolvedHero,
} from "../../../packages/heroes/src/index.ts";

/**
 * Spec 044, US6 and US7 (phase d): a saved look re-opens as a hero.
 *
 * The builder stores the spec that drew each image beside it, and opening
 * the builder again starts from that spec, so every control shows the choice
 * it was saved with. What the server holds is read back through
 * `worldActors { images { heroSpec } }`, and what the builder shows is
 * compared with that spec resolved by the catalogue at run time — no list of
 * parts, choices or colours is written here.
 */

interface ImageRow {
  role: string;
  assetId: string;
  heroSpec: Record<string, unknown> | null;
}

async function imagesOf(
  page: Page,
  worldId: string,
  actorId: string,
): Promise<ImageRow[]> {
  const { worldActors } = await must<{
    worldActors: { id: string; images: ImageRow[] }[];
  }>(
    page,
    `query ($worldId: UUID!) {
      worldActors(worldId: $worldId) { id images { role assetId heroSpec } }
    }`,
    { worldId },
  );
  const images = worldActors.find((actor) => actor.id === actorId)?.images;
  return [...(images ?? [])].sort((a, b) => a.role.localeCompare(b.role));
}

function specOf(images: ImageRow[], role: string) {
  return images.find((row) => row.role === role)?.heroSpec ?? null;
}

async function npc(page: Page, worldId: string, label: string) {
  const { createActor } = await must<{ createActor: { id: string } }>(
    page,
    `mutation ($input: CreateActorInput!) { createActor(input: $input) { id } }`,
    { input: { worldId, label, isNpc: true } },
  );
  return createActor.id;
}

async function openedBuilder(page: Page) {
  const dialog = page.getByTestId("hero-builder-dialog");
  await expect(dialog).toBeVisible({ timeout: 15_000 });
  await expect(dialog.getByTestId("hero-builder")).toBeVisible({
    timeout: 15_000,
  });
  await expect(dialog.getByTestId("hero-dialog-loading")).toHaveCount(0, {
    timeout: 15_000,
  });
  return dialog;
}

function resolved(spec: unknown): ResolvedHero {
  const checked = validateHero(spec);
  if (!checked.ok) {
    throw new Error(`stored spec does not validate: ${checked.problems}`);
  }
  return checked.hero;
}

/**
 * Every control of the builder shows `spec`: each part and the size checked
 * on its choice, each flag on or off, each colour's hex, the name and title,
 * and a colour the spec does not set shown as following its default.
 */
async function expectBuilderShows(dialog: Locator, spec: unknown) {
  const hero = resolved(spec) as unknown as Record<string, unknown>;
  const set = new Set(Object.keys(spec as object));
  await expect(dialog.getByTestId("hero-text-name")).toHaveValue(
    String(hero.name),
  );
  await expect(dialog.getByTestId("hero-text-title")).toHaveValue(
    typeof hero.title === "string" ? hero.title : "",
  );
  for (const field of ["size", ...Object.keys(HERO_PARTS)]) {
    await expect(
      dialog.getByTestId(`hero-choice-${field}-${String(hero[field])}`),
      `${field} shows ${String(hero[field])}`,
    ).toBeChecked();
  }
  for (const field of HERO_FLAGS) {
    const flag = dialog.getByTestId(`hero-flag-${field}`);
    if (hero[field] === true) await expect(flag).toBeChecked();
    else await expect(flag).not.toBeChecked();
  }
  for (const field of HERO_COLORS) {
    await expect(
      dialog.getByTestId(`hero-hex-${field}`),
      `${field} shows ${String(hero[field])}`,
    ).toHaveValue(String(hero[field]));
    const following = dialog.getByTestId(`hero-following-${field}`);
    if (set.has(field)) await expect(following).toBeHidden();
    else await expect(following).toBeVisible();
  }
}

/**
 * Rolls a whole hero from a fixed seed and then sets a size and a part the
 * dice leave alone for a person, so the look saved differs from the default
 * in parts, colours and size alike.
 */
async function buildLook(dialog: Locator, seed: string) {
  await dialog.getByTestId("hero-seed").fill(seed);
  await dialog.getByTestId("hero-roll-seed").click();
  const size = SIZES[SIZES.length - 1];
  await dialog.getByTestId(`hero-choice-size-${size}`).check();
  const [part, choices] = Object.entries(HERO_PARTS)[
    Object.keys(HERO_PARTS).length - 1
  ] as [string, readonly string[]];
  await dialog
    .getByTestId(`hero-choice-${part}-${choices[choices.length - 1]}`)
    .check();
}

/** An `UPDATE` against the per-shard database, as `playPause.ts` does it. */
function sql(statement: string): string {
  const container =
    process.env.THUNDERFORGE_POSTGRES_CONTAINER ?? "thunderforge-postgres";
  const database = process.env.THUNDERFORGE_DB_NAME ?? "thunderforge";
  const dbUser = process.env.THUNDERFORGE_DB_USER ?? "postgres";
  return execFileSync(
    "docker",
    [
      "exec",
      "-i",
      container,
      "psql",
      "-U",
      dbUser,
      "-d",
      database,
      "-v",
      "ON_ERROR_STOP=1",
      "-t",
      "-A",
    ],
    {
      input: statement,
      encoding: "utf-8",
      stdio: ["pipe", "pipe", "inherit"],
    },
  );
}

const UUID = /^[0-9a-f-]{36}$/;

test.describe("A saved look re-opens as a hero (US6, US7)", () => {
  test("a built NPC re-opens with every choice after a reload, and again after a collection copy into another world", async ({
    page,
  }) => {
    test.setTimeout(180_000);
    const suffix = uniqueSuffix();
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Saved Look ${suffix}`,
    );
    const actorId = await npc(page, worldId, "Mirelda");
    const editUrl = `/world/${worldId}/compendium/npc/${actorId}/edit`;
    await page.goto(editUrl);

    await page.getByTestId("actor-imagery-build").click();
    let dialog = await openedBuilder(page);
    await expect(dialog).toHaveAttribute("data-opened-from", "name");
    await buildLook(dialog, `saved-look-${suffix}`);
    await dialog.getByTestId("hero-dialog-save").click();
    await expect(dialog).toBeHidden({ timeout: 20_000 });

    // Both roles carry the same spec: valid, the smallest that draws the
    // hero (so derived colours keep following), and with no race (B5a).
    const stored = await imagesOf(page, worldId, actorId);
    expect(stored.map((row) => row.role)).toEqual(["portrait", "token"]);
    const saved = specOf(stored, "portrait")!;
    expect(saved).not.toBeNull();
    expect(specOf(stored, "token")).toEqual(saved);
    expect(saved).not.toHaveProperty("race");
    resolved(saved);
    expect(saved).toEqual(minimalSpec(saved as unknown as HeroSpec));

    await page.reload();
    await page.getByTestId("actor-imagery-build").click();
    dialog = await openedBuilder(page);
    await expect(dialog).toHaveAttribute("data-opened-from", "portrait");
    await expect(dialog.getByTestId("hero-dialog-stored-note")).toHaveCount(0);
    await expectBuilderShows(dialog, saved);
    await dialog.getByTestId("hero-dialog-close").click();
    await expect(dialog).toBeHidden();

    // Through a collection into a second world (FR-039).
    const { createCollection } = await must<{
      createCollection: { id: string };
    }>(
      page,
      `mutation ($input: CreateCollectionInput!) {
        createCollection(input: $input) { id }
      }`,
      { input: { worldId, name: `Faces ${suffix}` } },
    );
    await must(
      page,
      `mutation ($input: AddCollectionMemberInput!) {
        addCollectionMember(input: $input) { id }
      }`,
      {
        input: {
          collectionId: createCollection.id,
          memberType: "actor",
          memberId: actorId,
        },
      },
    );
    const { createCollectionShareLink } = await must<{
      createCollectionShareLink: { shareCode: string };
    }>(
      page,
      `mutation ($collectionId: UUID!, $attestation: AttestationInput!) {
        createCollectionShareLink(
          collectionId: $collectionId
          attestation: $attestation
        ) { shareCode }
      }`,
      {
        collectionId: createCollection.id,
        attestation: { termsVersionId: await currentSharingTermsVersion(page) },
      },
    );
    const { createWorld } = await must<{ createWorld: { id: string } }>(
      page,
      `mutation ($input: GraphQLCreateWorldInput!) {
        createWorld(input: $input) { id }
      }`,
      { input: { name: `E2E Saved Look Copy ${suffix}` } },
    );
    const { copySharedCollectionToWorld } = await must<{
      copySharedCollectionToWorld: {
        created: { memberType: string; id: string }[];
      };
    }>(
      page,
      `mutation ($shareCode: String!, $destinationWorldId: UUID!) {
        copySharedCollectionToWorld(
          shareCode: $shareCode
          destinationWorldId: $destinationWorldId
        ) { created { memberType id } }
      }`,
      {
        shareCode: createCollectionShareLink.shareCode,
        destinationWorldId: createWorld.id,
      },
    );
    const copyId = copySharedCollectionToWorld.created.find(
      (record) => record.memberType === "actor",
    )!.id;
    expect(copyId).toMatch(UUID);
    expect(copyId).not.toBe(actorId);

    const copied = await imagesOf(page, createWorld.id, copyId);
    expect(copied.map((row) => row.role)).toEqual(["portrait", "token"]);
    expect(specOf(copied, "portrait")).toEqual(saved);

    await page.goto(`/world/${createWorld.id}/compendium/npc/${copyId}/edit`);
    await page.getByTestId("actor-imagery-build").click();
    dialog = await openedBuilder(page);
    await expect(dialog).toHaveAttribute("data-opened-from", "portrait");
    await expectBuilderShows(dialog, saved);
  });

  test("a look saved from a compendium row re-opens from the edit page with the same choices", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Row Saved ${uniqueSuffix()}`,
    );
    const actorId = await npc(page, worldId, "Tobble");
    await page.goto(`/world/${worldId}/compendium?tab=npcs`);
    await page.getByTestId(`npc-catalog-build-${actorId}`).click();
    let dialog = await openedBuilder(page);
    await buildLook(dialog, `row-look-${actorId}`);
    await dialog.getByTestId("hero-dialog-save").click();
    await expect(dialog).toBeHidden({ timeout: 20_000 });

    const saved = specOf(await imagesOf(page, worldId, actorId), "portrait");
    expect(saved).not.toBeNull();

    await page.goto(`/world/${worldId}/compendium/npc/${actorId}/edit`);
    await page.getByTestId("actor-imagery-build").click();
    dialog = await openedBuilder(page);
    await expect(dialog).toHaveAttribute("data-opened-from", "portrait");
    await expectBuilderShows(dialog, saved);
  });

  test("a token replaced by an uploaded file clears its spec, and the builder opens on the portrait's and says so (FR-035, B8)", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Token Replaced ${uniqueSuffix()}`,
    );
    const actorId = await npc(page, worldId, "Grom");
    await page.goto(`/world/${worldId}/compendium/npc/${actorId}/edit`);
    await page.getByTestId("actor-imagery-build").click();
    let dialog = await openedBuilder(page);
    await buildLook(dialog, `replaced-${actorId}`);
    await dialog.getByTestId("hero-dialog-save").click();
    await expect(dialog).toBeHidden({ timeout: 20_000 });
    const built = await imagesOf(page, worldId, actorId);
    const saved = specOf(built, "portrait");
    expect(specOf(built, "token")).toEqual(saved);

    const grom = PRESET_HEROES.find((preset) => preset.slug === "grom")!;
    const tokenBefore = built.find((row) => row.role === "token")!.assetId;
    await page.getByTestId("actor-imagery-input-token").setInputFiles({
      name: "grom.svg",
      mimeType: "image/svg+xml",
      buffer: Buffer.from(renderToken(grom.spec)),
    });
    await expect
      .poll(
        async () =>
          (await imagesOf(page, worldId, actorId)).find(
            (row) => row.role === "token",
          )?.assetId,
        { timeout: 15_000 },
      )
      .not.toBe(tokenBefore);
    const after = await imagesOf(page, worldId, actorId);
    expect(specOf(after, "token")).toBeNull();
    expect(specOf(after, "portrait")).toEqual(saved);

    await page.getByTestId("actor-imagery-build").click();
    dialog = await openedBuilder(page);
    await expect(dialog).toHaveAttribute("data-opened-from", "portrait");
    await expect(dialog.getByTestId("hero-dialog-stored-note")).toContainText(
      "token is no longer a built hero",
    );
    await expectBuilderShows(dialog, saved);
  });

  test("an actor with no stored spec opens on its name, even with an uploaded portrait", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `E2E No Spec ${uniqueSuffix()}`,
    );
    const actorId = await npc(page, worldId, "Quill");
    await page.goto(`/world/${worldId}/compendium/npc/${actorId}/edit`);
    const grom = PRESET_HEROES.find((preset) => preset.slug === "grom")!;
    await page.getByTestId("actor-imagery-input-portrait").setInputFiles({
      name: "portrait.svg",
      mimeType: "image/svg+xml",
      buffer: Buffer.from(renderToken(grom.spec)),
    });
    await expect(
      page.getByTestId("actor-imagery-preview-portrait"),
    ).toBeVisible({ timeout: 15_000 });
    expect(
      specOf(await imagesOf(page, worldId, actorId), "portrait"),
    ).toBeNull();

    await page.getByTestId("actor-imagery-build").click();
    const dialog = await openedBuilder(page);
    await expect(dialog).toHaveAttribute("data-opened-from", "name");
    await expect(dialog.getByTestId("hero-dialog-stored-note")).toHaveCount(0);
    await expectBuilderShows(dialog, { name: "Quill" });
  });

  test("a stored spec that no longer validates is shown by field and the stored images are left alone (US6 scenario 4, SC-011)", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Stale Spec ${uniqueSuffix()}`,
    );
    const actorId = await npc(page, worldId, "Vessa");
    await page.goto(`/world/${worldId}/compendium/npc/${actorId}/edit`);
    await page.getByTestId("actor-imagery-build").click();
    let dialog = await openedBuilder(page);
    await buildLook(dialog, `stale-${actorId}`);
    await dialog.getByTestId("hero-dialog-save").click();
    await expect(dialog).toBeHidden({ timeout: 20_000 });

    // The catalogue "loses" a choice: the stored spec names one that is not
    // there. The server would refuse such a spec (B7), so it is written
    // straight to the row, as a catalogue change would leave it.
    const [part] = Object.keys(HERO_PARTS);
    expect(actorId).toMatch(UUID);
    const updated = sql(
      `UPDATE world_actor_images
         SET hero_spec = jsonb_set(hero_spec, '{${part}}', '"a-choice-since-removed"')
       WHERE actor_id = '${actorId}' AND hero_spec IS NOT NULL
       RETURNING role;`,
    );
    // psql prints the command tag ("UPDATE 2") after the returned rows.
    const roles = updated
      .split("\n")
      .map((line) => line.trim())
      .filter((line) => line !== "" && !line.startsWith("UPDATE "));
    expect(roles.sort()).toEqual(["portrait", "token"]);
    const before = await imagesOf(page, worldId, actorId);

    await page.reload();
    await page.getByTestId("actor-imagery-build").click();
    dialog = page.getByTestId("hero-builder-dialog");
    await expect(dialog.getByTestId("hero-dialog-stored-invalid")).toBeVisible({
      timeout: 15_000,
    });
    const problems = dialog.getByTestId("hero-problems");
    await expect(problems.locator(`li[data-field="${part}"]`)).toHaveCount(1);
    // Nothing is drawn from defaults, and nothing can be saved from it.
    await expect(dialog.getByTestId("hero-builder").locator("svg")).toHaveCount(
      0,
    );
    await expect(dialog.getByTestId("hero-dialog-save")).toHaveCount(0);
    await dialog.getByTestId("hero-dialog-close").click();
    await expect(dialog).toBeHidden();
    expect(await imagesOf(page, worldId, actorId)).toEqual(before);

    // Starting again is the user's choice, and it starts from the name.
    await page.getByTestId("actor-imagery-build").click();
    await dialog.getByTestId("hero-dialog-start-from-name").click();
    await expect(dialog.getByTestId("hero-builder")).toBeVisible();
    await expect(dialog.getByTestId("hero-text-name")).toHaveValue("Vessa");
    expect(await imagesOf(page, worldId, actorId)).toEqual(before);
  });
});
