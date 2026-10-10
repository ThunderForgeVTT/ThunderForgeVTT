import { test, expect, type Page } from "./fixtures/test";
import {
  graphql,
  registerAndCreateWorld,
  setWorldSystem,
} from "./fixtures/helpers";

/**
 * Spec 088 US3 (SC-004, FR-034 to FR-036, contracts/layouts.md): the actor
 * view uses the width it has. One column on a phone, two from 1024 px, three
 * from 1536 px, the content capped at 1800 px and centred, and no sideways
 * scroll from 320 px to 3840 px. The world page's half is
 * `world-layout-widths.spec.ts`, in the worlds slice.
 */

const LONG_DESCRIPTION = Array.from(
  { length: 12 },
  () =>
    "A long description runs on, sentence after sentence, so that a line held to the full page would be far too long to read.",
).join(" ");

async function createCharacter(page: Page, worldId: string): Promise<string> {
  const result = await graphql<{
    data?: { createActor?: { id: string } };
    errors?: { message: string }[];
  }>(
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
        label: "Wide Hero",
        isNpc: false,
        gameSystemId: "dnd5e",
        description: LONG_DESCRIPTION,
      },
    },
  );
  const id = result.data?.createActor?.id;
  if (!id) {
    throw new Error(`createActor: ${JSON.stringify(result.errors ?? result)}`);
  }
  return id;
}

/** The column count, read from the grid's computed template. */
function columns(page: Page): Promise<number> {
  return page.getByTestId("actor-view-grid").evaluate(
    (grid) =>
      getComputedStyle(grid)
        .gridTemplateColumns.split(" ")
        .filter((track) => track.trim() !== "").length,
  );
}

function scrollsSideways(page: Page): Promise<boolean> {
  return page.evaluate(
    () => document.documentElement.scrollWidth > window.innerWidth,
  );
}

const WIDTHS: { width: number; columns: number; capped: boolean }[] = [
  { width: 320, columns: 1, capped: false },
  { width: 375, columns: 1, capped: false },
  { width: 1280, columns: 2, capped: false },
  { width: 2560, columns: 3, capped: true },
  { width: 3840, columns: 3, capped: true },
];

test("the actor view takes one, two or three columns, capped at 1800 px, with no sideways scroll", async ({
  page,
}) => {
  test.setTimeout(120_000);
  const worldId = await registerAndCreateWorld(
    page,
    "Layout widths",
    "e2ewidths",
  );
  await setWorldSystem(page, worldId, "dnd5e");
  const actorId = await createCharacter(page, worldId);

  for (const { width, columns: expected, capped } of WIDTHS) {
    await test.step(`${width} px`, async () => {
      await page.setViewportSize({ width, height: 1000 });
      await page.goto(`/world/${worldId}/actor/${actorId}/view`);
      await expect(page.getByTestId("actor-view-grid")).toBeVisible({
        timeout: 15_000,
      });
      await expect(
        page.getByRole("heading", { name: "Wide Hero" }),
      ).toBeVisible();

      await expect.poll(() => columns(page)).toBe(expected);
      expect(await scrollsSideways(page), "no sideways scroll").toBe(false);

      const box = await page.getByTestId("actor-view").boundingBox();
      expect(box).not.toBeNull();
      if (capped) {
        expect(Math.abs(box!.width - 1800)).toBeLessThanOrEqual(1);
      }

      // Long text is held to a readable measure inside its card.
      const prose = page.getByText(LONG_DESCRIPTION.slice(0, 40));
      const proseBox = await prose.boundingBox();
      expect(proseBox).not.toBeNull();
      expect(proseBox!.width).toBeLessThanOrEqual(Math.min(width, 65 * 16) + 1);
    });
  }
});
