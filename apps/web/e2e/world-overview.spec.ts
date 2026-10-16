import { test, expect } from "./fixtures/test";
import { expectNoAxeViolations } from "./fixtures/axe";
import {
  graphql,
  inviteAndJoinAsPlayer,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";
import { createScene } from "./fixtures/world-cache";

/**
 * Spec 100 US3: the world's overview gets the session ready — where we
 * start (scene cards, "Play here"), who is coming (the party) and where we
 * left off (the notes), with Play naming the scene it resumes in.
 */

async function activeSceneOf(page: Parameters<typeof graphql>[0], id: string) {
  const res = await graphql<{
    data?: { world: { activeSceneId: string | null } };
  }>(
    page,
    `
      query ($id: UUID!) {
        world(id: $id) {
          activeSceneId
        }
      }
    `,
    { id },
  );
  return res.data?.world.activeSceneId ?? null;
}

test.describe("Spec 100 US3: the overview gets the session ready", () => {
  test("a Game Master's 'Play here' moves the table and opens it (SC-004)", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Overview ${uniqueSuffix()}`,
    );
    const cellar = await createScene(page, worldId, "The Cellar");

    await page.goto(`/world/${worldId}`);
    const strip = page.getByTestId("recent-scenes");
    const card = strip
      .getByTestId("recent-scene")
      .filter({ hasText: "The Cellar" });
    // A new scene is hidden from players, and its Game Master is told so.
    await expect(card).toContainText("Hidden from players");
    await expect(card).not.toHaveAttribute("data-active", "true");
    await expectNoAxeViolations(page, "[data-testid=recent-scenes]");

    await card.getByTestId("scene-play-here").click();
    await page.waitForURL(new RegExp(`/world/${worldId}/play$`), {
      timeout: 15_000,
    });
    expect(await activeSceneOf(page, worldId)).toBe(cellar);

    // Back at the overview, the table's scene leads and Play says where.
    await page.goto(`/world/${worldId}`);
    await expect(strip.getByTestId("recent-scene").first()).toHaveAttribute(
      "data-active",
      "true",
    );
    await expect(strip.getByTestId("recent-scene").first()).toContainText(
      "The Cellar",
    );
    await expect(page.getByTestId("play-button")).toHaveText(
      /Resume in The Cellar/,
    );

    // Every world page's Play says the same, not only the overview's.
    for (const section of ["players", "scenes", "compendium"]) {
      await page.goto(`/world/${worldId}/${section}`);
      await expect(page.getByTestId("play-button")).toHaveText(
        /Resume in The Cellar/,
      );
    }
  });

  test("the party fills in as players join; each section has an empty state", async ({
    page,
    browser,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Overview Party ${uniqueSuffix()}`,
    );

    await page.goto(`/world/${worldId}`);
    const party = page.getByTestId("world-players-card");
    await expect(party.getByTestId("party-empty")).toBeVisible();
    await expect(page.getByTestId("notes-empty")).toBeVisible();

    const player = await inviteAndJoinAsPlayer(
      browser,
      page,
      worldId,
      "e2eovparty",
    );
    await player.close();

    await page.reload();
    const seat = party.getByTestId("party-seat");
    await expect(seat).toHaveCount(1);
    await expect(seat).toContainText(/e2eovparty/);
    await expect(seat).toContainText("No character yet");
  });

  test("scenes, party and notes sit side by side on a wide screen", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Overview Wide ${uniqueSuffix()}`,
    );
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto(`/world/${worldId}`);
    const party = await page.getByTestId("world-players-card").boundingBox();
    const notes = await page.getByTestId("session-notes-panel").boundingBox();
    expect(party && notes, "both sections are drawn").toBeTruthy();
    // Beside, not under: their tops line up within a row.
    expect(Math.abs(party!.y - notes!.y)).toBeLessThan(8);

    await page.setViewportSize({ width: 390, height: 844 });
    const narrowParty = await page
      .getByTestId("world-players-card")
      .boundingBox();
    const narrowNotes = await page
      .getByTestId("session-notes-panel")
      .boundingBox();
    expect(narrowNotes!.y).toBeGreaterThan(
      narrowParty!.y + narrowParty!.height - 1,
    );
  });
});
