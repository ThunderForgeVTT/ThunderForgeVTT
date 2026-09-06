import { expect, test, type Page } from "@playwright/test";
import {
  graphql,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";

/**
 * ADR-071: a shared ability, item or actor opens **without an account**, on the
 * same terms `sharedCollection` has had since ADR-070.
 *
 * Every assertion is from a fresh browser context that has never signed in,
 * because that is the only thing that can tell you what a stranger sees. The
 * unit tests prove `shared_*_impl` resolves for a caller with no session; only
 * this can prove the page in front of it does not bounce them to `/login`,
 * which is where this behaviour was actually blocked before — a resolver that
 * answers anonymously behind a route that redirects is the same wall in a
 * different place.
 *
 * # What is deliberately not asserted here
 *
 * The rate limit, for the reason `collection-anonymous-access.spec.ts` gives:
 * it keys on the caller's IP, every request in this suite arrives from
 * `127.0.0.1`, and tripping it would refuse the next spec's anonymous requests
 * for a full minute. `graphql/share_rate_limit.rs` covers the threshold, and
 * each share module has its own `the_anonymous_read_is_rate_limited` test.
 */

/**
 * Opens a share URL in a context with no account, waits for the named artifact
 * to actually be on screen, and returns everything the page shows.
 *
 * It waits for the heading rather than for a loader to clear. Two earlier
 * drafts read the body too early and reported the feature broken: once while
 * "Loading shared ability" was still up, and once during the app shell's own
 * "Checking instance setup" gate, which runs before any route renders. Waiting
 * on the thing actually being asserted is the only wait that cannot be early.
 */
async function visitAsStranger(
  visitor: Page,
  path: string,
  name: string,
): Promise<string> {
  await visitor.goto(path);
  await expect(
    visitor.getByRole("heading", { name }),
    `${path} must render for a signed-out visitor`,
  ).toBeVisible({ timeout: 30_000 });
  return (await visitor.locator("body").innerText()).trim();
}

test.describe("ADR-071: a stranger opens all three singleton shares", () => {
  test("ability, item and actor links each render with no account, and name no world", async ({
    page,
    browser,
  }) => {
    test.setTimeout(300_000);

    const suffix = uniqueSuffix();
    const worldName = `E2E Singleton Source ${suffix}`;
    const worldId = await registerAndCreateWorld(page, worldName, "e2esingle");

    const abilityName = `Thunderclap ${suffix}`;
    const ability = await graphql<{ data: { createAbility: { id: string } } }>(
      page,
      `
        mutation CA($input: CreateAbilityInput!) {
          createAbility(input: $input) {
            id
          }
        }
      `,
      {
        input: {
          worldId,
          name: abilityName,
          description: "A crack of nearby thunder.",
          classification: "SPELL",
          // Not GM-only: a GM-only ability is refused by the share path, and
          // this journey is about the path that works.
          gmOnly: false,
        },
      },
    );

    const itemName = `Storm Lantern ${suffix}`;
    const item = await graphql<{ data: { createItem: { id: string } } }>(
      page,
      `
        mutation CI($input: CreateItemInput!) {
          createItem(input: $input) {
            id
          }
        }
      `,
      { input: { worldId, name: itemName, description: "It never gutters." } },
    );

    const actorLabel = `Stormcaller ${suffix}`;
    const actor = await graphql<{ data: { createActor: { id: string } } }>(
      page,
      `
        mutation CAc($input: CreateActorInput!) {
          createActor(input: $input) {
            id
          }
        }
      `,
      {
        input: {
          worldId,
          label: actorLabel,
          isNpc: true,
          actorType: "npc",
        },
      },
    );

    const shareCodes = {
      ability: (
        await graphql<{
          data: { createAbilityShareLink: { id: string; shareCode: string } };
        }>(
          page,
          `
            mutation S($id: UUID!) {
              createAbilityShareLink(abilityId: $id) {
                id
                shareCode
              }
            }
          `,
          { id: ability.data.createAbility.id },
        )
      ).data.createAbilityShareLink,
      item: (
        await graphql<{
          data: { createItemShareLink: { id: string; shareCode: string } };
        }>(
          page,
          `
            mutation S($id: UUID!) {
              createItemShareLink(itemId: $id) {
                id
                shareCode
              }
            }
          `,
          { id: item.data.createItem.id },
        )
      ).data.createItemShareLink,
      actor: (
        await graphql<{
          data: { createActorShareLink: { id: string; shareCode: string } };
        }>(
          page,
          `
            mutation S($id: UUID!) {
              createActorShareLink(actorId: $id) {
                id
                shareCode
              }
            }
          `,
          { id: actor.data.createActor.id },
        )
      ).data.createActorShareLink,
    };

    const visitorContext = await browser.newContext();
    const visitor = await visitorContext.newPage();
    try {
      const cases = [
        {
          path: `/shared/ability/${shareCodes.ability.shareCode}`,
          name: abilityName,
        },
        { path: `/shared/item/${shareCodes.item.shareCode}`, name: itemName },
        {
          path: `/shared/actor/${shareCodes.actor.shareCode}`,
          name: actorLabel,
        },
      ];

      for (const { path, name } of cases) {
        const shown = await visitAsStranger(visitor, path, name);

        // And the visitor was not sent to a login screen on the way.
        expect(
          new URL(visitor.url()).pathname,
          `${path} must not redirect`,
        ).toBe(path);

        // The preview carries nothing identifying the source world.
        expect(shown).not.toContain(worldId);
        expect(shown).not.toContain(worldName);
      }

      // Revoking still reaches the stranger, and says nothing about why.
      await graphql(
        page,
        `
          mutation R($shareId: UUID!) {
            revokeAbilityShareLink(shareId: $shareId)
          }
        `,
        { shareId: shareCodes.ability.id },
      );

      await visitor.goto(`/shared/ability/${shareCodes.ability.shareCode}`);
      await expect(
        visitor.getByTestId("shared-ability-unavailable"),
      ).toBeVisible({ timeout: 30_000 });
      const revoked = (await visitor.locator("body").innerText()).trim();
      expect(revoked).not.toContain(abilityName);
      // The refusal says the link is unavailable, never which of the four
      // reasons it is — telling them apart is the probe.
      expect(revoked.toLowerCase()).not.toContain("revok");
    } finally {
      await visitorContext.close();
    }
  });
});
