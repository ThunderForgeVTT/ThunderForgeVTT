import { expect, test, type Page } from "@playwright/test";
import {
  graphql,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";

/**
 * specs/026-content-collections, T048: what an anonymous caller can and cannot
 * learn.
 *
 * Every assertion here is from a **fresh browser context that has never signed
 * in**, because that is the only thing that can tell you what a stranger sees.
 *
 * # What is deliberately not asserted here
 *
 * FR-009c's rate limit. It is real and it is tested — `graphql/share_rate_limit.rs`
 * covers the threshold, and that the limiter ignores
 * `THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT` even though the auth limiter honours
 * it. It is not exercised here because the limiter keys on the caller's IP and
 * every request in this suite arrives from `127.0.0.1`: tripping it would
 * refuse the *next* spec's anonymous requests for a full minute, turning one
 * assertion into a shard-wide flake. A test that has to break its neighbours to
 * run is worse than the unit test that already covers the same behaviour.
 */

/** The one sentence every unavailable code answers with (FR-009d). */
async function unavailableSentence(
  visitor: Page,
  shareCode: string,
): Promise<string> {
  await visitor.goto(`/collection/${shareCode}`);
  const badge = visitor.getByTestId("collection-unavailable");
  await expect(badge).toBeVisible({ timeout: 20_000 });
  return (await badge.innerText()).trim();
}

test.describe("spec 026: what a stranger can learn", () => {
  test("three unavailable states read identically, and nothing names the world", async ({
    page,
    browser,
  }) => {
    test.setTimeout(300_000);

    const suffix = uniqueSuffix();
    const worldName = `E2E Anon Source ${suffix}`;
    const worldId = await registerAndCreateWorld(page, worldName, "e2eanon");

    const loreTitle = `A Rumour ${suffix}`;
    const lore = await graphql<{ data: { createLoreEntry: { id: string } } }>(
      page,
      `
        mutation C($input: CreateLoreEntryInput!) {
          createLoreEntry(input: $input) {
            id
          }
        }
      `,
      { input: { worldId, title: loreTitle, content: "Overheard." } },
    );
    const loreId = lore.data.createLoreEntry.id;

    // Two collections: one to revoke, one to delete. Both must end up reading
    // the same as a code that never existed.
    const toRevoke = await graphql<{
      data: { createCollection: { id: string } };
    }>(
      page,
      `
        mutation C($input: CreateCollectionInput!) {
          createCollection(input: $input) {
            id
          }
        }
      `,
      { input: { worldId, name: `Revoked ${suffix}` } },
    );
    const revokedCollectionId = toRevoke.data.createCollection.id;

    const toDelete = await graphql<{
      data: { createCollection: { id: string } };
    }>(
      page,
      `
        mutation C($input: CreateCollectionInput!) {
          createCollection(input: $input) {
            id
          }
        }
      `,
      { input: { worldId, name: `Deleted ${suffix}` } },
    );
    const deletedCollectionId = toDelete.data.createCollection.id;

    for (const collectionId of [revokedCollectionId, deletedCollectionId]) {
      await graphql(
        page,
        `
          mutation A($input: AddCollectionMemberInput!) {
            addCollectionMember(input: $input) {
              id
            }
          }
        `,
        { input: { collectionId, memberType: "lore", memberId: loreId } },
      );
    }

    const shareOf = async (collectionId: string) => {
      const shared = await graphql<{
        data: { createCollectionShareLink: { id: string; shareCode: string } };
      }>(
        page,
        `
          mutation S($collectionId: UUID!) {
            createCollectionShareLink(collectionId: $collectionId) {
              id
              shareCode
            }
          }
        `,
        { collectionId },
      );
      return shared.data.createCollectionShareLink;
    };

    const revokedShare = await shareOf(revokedCollectionId);
    const deletedShare = await shareOf(deletedCollectionId);

    const visitorContext = await browser.newContext();
    const visitor = await visitorContext.newPage();
    try {
      // Both links work first, from a context with no account at all.
      await visitor.goto(`/collection/${revokedShare.shareCode}`);
      await expect(
        visitor.getByRole("heading", { name: `Revoked ${suffix}` }),
      ).toBeVisible({ timeout: 20_000 });

      // SC-007a: the anonymous view carries nothing about the source world.
      const shown = await visitor.locator("body").innerText();
      expect(shown).not.toContain(worldId);
      expect(shown).not.toContain(worldName);

      await graphql(
        page,
        `
          mutation R($shareId: UUID!) {
            revokeCollectionShareLink(shareId: $shareId)
          }
        `,
        { shareId: revokedShare.id },
      );
      await graphql(
        page,
        `
          mutation D($collectionId: UUID!) {
            deleteCollection(collectionId: $collectionId)
          }
        `,
        { collectionId: deletedCollectionId },
      );

      // FR-009d: a revoked share, a deleted collection and a code that never
      // existed must be indistinguishable. Compared to **each other** rather
      // than to a hardcoded string, so rewording the sentence keeps the test
      // meaningful instead of breaking it.
      const revoked = await unavailableSentence(
        visitor,
        revokedShare.shareCode,
      );
      const deleted = await unavailableSentence(
        visitor,
        deletedShare.shareCode,
      );
      const nonexistent = await unavailableSentence(
        visitor,
        "thiscodeneverexisted",
      );

      expect(revoked).not.toBe("");
      expect(deleted).toBe(revoked);
      expect(nonexistent).toBe(revoked);

      // And none of the three says which case it was, or names anything.
      for (const sentence of [revoked, deleted, nonexistent]) {
        expect(sentence.toLowerCase()).not.toContain("revok");
        expect(sentence.toLowerCase()).not.toContain("delet");
        expect(sentence).not.toContain(worldName);
      }
    } finally {
      await visitorContext.close();
    }
  });
});
