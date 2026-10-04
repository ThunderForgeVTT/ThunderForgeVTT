import type { Page } from "@playwright/test";

import {
  freshCredentials,
  graphql,
  openDockTab,
  register,
  uniqueSuffix,
} from "./fixtures/helpers";
import { expect, test } from "./fixtures/test";

/**
 * The first thing a Game Master does: name a world and say what game it is.
 *
 * Every other spec makes its world through `registerAndCreateWorld`, which
 * leaves the picker alone and so only ever proves the realm's default. This
 * one chooses. It asks these things of the choice, because each is a place
 * the system could be dropped on the way from the form to the table:
 *
 * - the **world** is stored on the system that was picked;
 * - a **character made without naming a system** — which is how the app's
 *   own dialogs make them — is on the world's system, not the realm's
 *   default — and one that names a different system is refused;
 * - opening it from the **play dock** shows that system's sheet.
 *
 * The two systems below are the ones meant to be ready for a table full
 * time. A realm that does not offer one fails the test rather than skipping
 * it: a missing full-time system is the finding.
 */

interface Offered {
  systems: { id: string; title: string }[];
  defaultId: string | null;
}

async function createWorldOn(
  page: Page,
  systemId: string,
): Promise<{ worldId: string; offered: Offered }> {
  await register(page, freshCredentials("e2esys"));
  await page.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });

  const offered = await page.request
    .get("/api/systems")
    .then((response) => response.json() as Promise<Offered>);
  const title = offered.systems.find((system) => system.id === systemId)?.title;
  expect(title, `the realm must offer ${systemId}`).toBeTruthy();

  await page.locator("#world-name").fill(`E2E ${systemId} ${uniqueSuffix()}`);
  const picker = page.getByRole("combobox", { name: "Game system" });
  await picker.click();
  await page.getByRole("option", { name: title!, exact: true }).click();
  await expect(picker).toHaveText(title!);

  await page.getByRole("button", { name: /create world/i }).click();
  await page.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });
  const worldId = /\/world\/([^/]+)\/staging$/.exec(
    new URL(page.url()).pathname,
  )?.[1];
  if (!worldId) throw new Error(`no world id in ${page.url()}`);
  return { worldId, offered };
}

for (const { systemId, sheet } of [
  { systemId: "roll_for_shoes", sheet: "rfs-sheet" },
  { systemId: "dnd5e", sheet: "dnd5e-actor-sheet" },
]) {
  test(`a world created as ${systemId} is one, and so are its characters`, async ({
    page,
  }) => {
    const { worldId } = await createWorldOn(page, systemId);

    const world = await graphql<{
      data?: { world?: { gameSystemId: string | null } };
      errors?: { message: string }[];
    }>(
      page,
      `
        query ($id: UUID!) {
          world(id: $id) {
            gameSystemId
          }
        }
      `,
      { id: worldId },
    );
    expect(
      world.data?.world?.gameSystemId,
      `world read failed: ${JSON.stringify(world.errors ?? world)}`,
    ).toBe(systemId);

    // No `gameSystemId` in the input: the same call the app's dialogs make.
    const actor = await graphql<{
      data?: { createActor?: { id: string; gameSystemId: string | null } };
      errors?: { message: string }[];
    }>(
      page,
      `
        mutation ($input: CreateActorInput!) {
          createActor(input: $input) {
            id
            gameSystemId
          }
        }
      `,
      { input: { worldId, label: "Newcomer", isNpc: false } },
    );
    expect(
      actor.data?.createActor?.gameSystemId,
      `actor refused: ${JSON.stringify(actor.errors ?? actor)}`,
    ).toBe(systemId);
    const actorId = actor.data!.createActor!.id;

    // Naming a system the world does not play is refused, not stored.
    const other = systemId === "dnd5e" ? "roll_for_shoes" : "dnd5e";
    const stray = await graphql<{
      data?: { createActor?: { id: string } | null };
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
          label: "Stray",
          isNpc: false,
          gameSystemId: other,
        },
      },
    );
    expect(stray.data?.createActor ?? null).toBeNull();
    expect(stray.errors?.[0]?.message ?? "").toContain(other);

    await page.goto(`/world/${worldId}/play`);
    await openDockTab(page, "actors");
    // A Game Master's View opens the character in a new tab, so the map stays
    // in front of them; the sheet is read there, not in the dock.
    const [sheetTab] = await Promise.all([
      page.waitForEvent("popup"),
      page.getByTestId(`actor-view-${actorId}`).click(),
    ]);
    await expect(sheetTab.getByTestId(sheet)).toBeVisible({ timeout: 15_000 });
  });
}
