import { expect, test, type Page } from "./fixtures/test";
import { expectNoAxeViolations } from "./fixtures/axe";
import { openDockTab, registerAndCreateWorld } from "./fixtures/helpers";
import { closeTable, must, openTable, sitDown } from "../playtest/table";
import {
  PRESET_HEROES,
  renderToken,
} from "../../../packages/heroes/src/index.ts";

/**
 * One way to set an actor's art, offered in three places (owner, 2026-09-15).
 *
 * - The actor's own edit page, which had no imagery controls at all — so a
 *   player's character could not be given a face from the page it lives on.
 * - The compendium's NPC row, where the face is now the control.
 * - The play field's Place, which now says what is about to be dropped and,
 *   when the actor has no art, says so and offers the fix.
 *
 * Each proves the image shows without a reload: a portrait that only appears
 * after one is a portrait the person believes did not save.
 */

function uniqueSuffix(): string {
  return `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}

/** A real image the server will accept: a preset hero, drawn as SVG. The
 *  server transcodes it to WebP (ADR-057). */
function heroSvg(name = "grom") {
  const hero = PRESET_HEROES.find((preset) => preset.slug === name)!;
  return {
    name: `${name}.svg`,
    mimeType: "image/svg+xml",
    buffer: Buffer.from(renderToken(hero.spec)),
  };
}

async function createActor(
  page: Page,
  worldId: string,
  label: string,
  isNpc: boolean,
): Promise<string> {
  const { createActor } = await must<{ createActor: { id: string } }>(
    page,
    `mutation ($input: CreateActorInput!) { createActor(input: $input) { id } }`,
    { input: { worldId, label, isNpc } },
  );
  return createActor.id;
}

/** It drew: a browser handed bytes it cannot decode reports no width. */
async function expectDrawn(page: Page, testId: string) {
  const image = page.getByTestId(testId);
  await expect(image).toBeVisible({ timeout: 15_000 });
  await expect
    .poll(
      () =>
        image.evaluate((img: HTMLImageElement) =>
          img.complete ? img.naturalWidth : 0,
        ),
      { timeout: 10_000 },
    )
    .toBeGreaterThan(0);
}

test.describe("Setting an actor's art", () => {
  test("a character's own edit page sets a portrait and a token, shown without a reload", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Actor Art ${uniqueSuffix()}`,
    );
    const actorId = await createActor(page, worldId, "Wren Ashdown", false);

    await page.goto(`/world/${worldId}/actor/${actorId}/edit`);
    const panel = page.getByTestId("actor-imagery-panel");
    await expect(panel).toBeVisible({ timeout: 15_000 });

    // Named for the character, so the control means something out of context.
    const portraitInput = page.getByLabel("Set portrait for Wren Ashdown");
    await portraitInput.setInputFiles(heroSvg());
    await expectDrawn(page, "actor-imagery-preview-portrait");

    await page
      .getByLabel("Set token for Wren Ashdown")
      .setInputFiles(heroSvg());
    await expectDrawn(page, "actor-imagery-preview-token");

    // And once set, the same control offers to replace rather than set.
    await expect(
      page.getByLabel("Replace portrait for Wren Ashdown"),
    ).toBeAttached();

    await expectNoAxeViolations(page, '[data-testid="actor-imagery-panel"]');
  });

  test("an NPC's portrait is set from its compendium row, a refusal stays on that row, and the row does not move", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Row Portrait ${uniqueSuffix()}`,
    );
    const boblin = await createActor(page, worldId, "Boblin", true);
    const other = await createActor(page, worldId, "Grimsby", true);

    await page.goto(`/world/${worldId}/compendium?tab=npcs`);
    const row = page.getByTestId(`npc-catalog-row-${boblin}`);
    await expect(row).toBeVisible({ timeout: 15_000 });
    const before = await row.boundingBox();

    await page.getByLabel("Set portrait for Boblin").setInputFiles(heroSvg());
    await expectDrawn(page, `npc-catalog-portrait-${boblin}`);

    // The list must not jump when an image lands.
    const after = await row.boundingBox();
    expect(after?.height).toBe(before?.height);

    // Setting it did not open the row's preview as a side effect.
    await expect(row).toHaveAttribute("aria-selected", "false");

    const src = await page
      .getByTestId(`npc-catalog-portrait-${boblin}`)
      .getAttribute("src");

    // A file the server cannot decode is refused, on this row only, and the
    // portrait already there is left alone.
    await page.getByLabel("Replace portrait for Boblin").setInputFiles({
      name: "not-an-image.png",
      mimeType: "image/png",
      buffer: Buffer.from("this is not a png"),
    });
    await expect(
      page.getByTestId(`npc-catalog-portrait-error-${boblin}`),
    ).toBeVisible({ timeout: 15_000 });
    await expect(
      page.getByTestId(`npc-catalog-portrait-${boblin}`),
    ).toHaveAttribute("src", src!);
    await expect(
      page.getByTestId(`npc-catalog-portrait-error-${other}`),
    ).toHaveCount(0);

    // Keyboard: the avatar's control takes focus by Tab like any input.
    await page.getByLabel("Set portrait for Grimsby").focus();
    await expect(page.getByLabel("Set portrait for Grimsby")).toBeFocused();

    await expectNoAxeViolations(page, '[data-testid="npc-catalog-table"]');
  });
});

/** Click at a pixel offset from the canvas centre, down and up a frame apart
 * (a zero-delay pair can collapse into a press with no release in Bevy). */
async function clickCanvasAt(page: Page, dx: number, dy: number) {
  const box = await page.locator("canvas").boundingBox();
  if (!box) throw new Error("the canvas must be laid out before it is used");
  await page.mouse.move(
    box.x + box.width / 2 + dx,
    box.y + box.height / 2 + dy,
  );
  await page.mouse.down();
  await page.waitForTimeout(80);
  await page.mouse.up();
}

test("placing an actor says whether it has art, and names the token once it lands", async ({
  page,
  browser,
}, testInfo) => {
  test.setTimeout(4 * 60_000);

  const table = await openTable({
    browser,
    gm: page,
    testInfo,
    system: "dnd5e",
    players: [],
    sceneName: "The Crossroads",
  });

  try {
    const withArt = await createActor(
      table.gm,
      table.worldId,
      "Boblin the Goblin",
      true,
    );
    const withoutArt = await createActor(
      table.gm,
      table.worldId,
      "Plain Pete",
      true,
    );

    // Art set by the one way there is: the actor's own edit page.
    await table.gm.goto(`/world/${table.worldId}/actor/${withArt}/edit`);
    await table.gm
      .getByLabel("Set token for Boblin the Goblin")
      .setInputFiles(heroSvg());
    await expectDrawn(table.gm, "actor-imagery-preview-token");

    await sitDown(table, table.gm);
    await openDockTab(table.gm, "actors");

    // With art: the preview shows the image the server will draw.
    const placeBoblin = table.gm.getByTestId(`actor-place-${withArt}`);
    await placeBoblin.click();
    await expect(placeBoblin).toHaveAttribute("aria-pressed", "true");
    const preview = table.gm.getByTestId("placement-preview");
    await expect(preview).toContainText("Placing Boblin the Goblin");
    await expectDrawn(table.gm, "placement-preview-art");
    await expect(table.gm.getByTestId("placement-preview-no-art")).toHaveCount(
      0,
    );
    await expectNoAxeViolations(table.gm, '[data-testid="placement-preview"]');

    await clickCanvasAt(table.gm, -160, 0);
    await expect(preview).toBeHidden({ timeout: 15_000 });
    await expect(placeBoblin).toHaveAttribute("aria-pressed", "false");
    await expect(table.gm.getByTestId("placement-placed")).toContainText(
      "Boblin the Goblin is on the map",
    );

    // Without art: said outright, with the fix offered.
    await table.gm.getByTestId(`actor-place-${withoutArt}`).click();
    await expect(preview).toContainText("Placing Plain Pete");
    await expect(
      table.gm.getByTestId("placement-preview-no-art"),
    ).toBeVisible();
    await expect(preview).toContainText("Plain Pete has no art");
    await expect(
      table.gm.getByTestId("placement-preview-add-art"),
    ).toHaveAttribute(
      "href",
      `/world/${table.worldId}/actor/${withoutArt}/edit`,
    );
    await expectNoAxeViolations(table.gm, '[data-testid="placement-preview"]');

    // Cancel from the panel ends the carry the engine is holding.
    await table.gm.getByTestId("placement-preview-cancel").click();
    await expect(preview).toBeHidden({ timeout: 10_000 });
  } finally {
    await closeTable(table);
  }
});
