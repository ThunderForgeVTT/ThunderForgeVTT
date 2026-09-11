import { execFileSync } from "node:child_process";
import { expect, test, type Page } from "@playwright/test";
import {
  currentSharingTermsVersion,
  graphql,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";

/**
 * Spec 039 US6, quickstart Scenario F (T054): a takedown reaches the copies
 * people took, and does not take an adopter's world with it.
 *
 * Two people, two browser contexts. `user1` shares a collection holding a lore
 * entry and an ability; `user2` copies it into their own world, renames the
 * copied entry, and writes a note of their own beside it. A takedown against
 * `user1`'s entry must then:
 *
 *  - disable `user2`'s copy too, and nothing else of theirs (FR-023, FR-023c);
 *  - leave the copy a row in their world, not a deletion (FR-023a);
 *  - tell `user2`, on their standing page, that they are accused of nothing
 *    (FR-023b);
 *  - stop the collection link serving the entry (FR-022).
 *
 * And a successful counter-notice must bring both back, the copy with
 * `user2`'s rename intact, without `user2` asking (FR-023d).
 *
 * # How the waiting period is simulated
 *
 * As in `dmca-counter-notice.spec.ts`: one `UPDATE` moving already-recorded
 * restoration dates into the past. This one also moves the copy's, because the
 * point being tested is that the copy was forwarded with the same date — the
 * `RETURNING` count of two is that assertion.
 */

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/** The waiting period, elapsed — for the notice's case and its children. */
function elapseWaitingPeriod(caseId: string): number {
  if (!UUID.test(caseId)) {
    throw new Error(`Refusing to run SQL for a non-UUID case id: ${caseId}`);
  }
  const container =
    process.env.THUNDERFORGE_POSTGRES_CONTAINER ?? "thunderforge-postgres";
  const database = process.env.THUNDERFORGE_DB_NAME ?? "thunderforge";
  const dbUser = process.env.THUNDERFORGE_DB_USER ?? "postgres";

  const output = execFileSync(
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
      input:
        "UPDATE content_moderation_actions " +
        "SET restoration_due_at = NOW() - INTERVAL '1 day' " +
        `WHERE (case_id = '${caseId}' OR parent_case_id = '${caseId}') ` +
        "AND action_type = 'counter_notice_forwarded' RETURNING id;",
      encoding: "utf-8",
      stdio: ["pipe", "pipe", "inherit"],
    },
  );
  return output
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => UUID.test(line)).length;
}

type LoreRow = { id: string; title: string; slug: string };

async function loreIn(page: Page, worldId: string): Promise<LoreRow[]> {
  const result = await graphql<{ data: { worldLoreEntries: LoreRow[] } }>(
    page,
    `
      query L($worldId: UUID!) {
        worldLoreEntries(worldId: $worldId) {
          id
          title
          slug
        }
      }
    `,
    { worldId },
  );
  return result.data.worldLoreEntries;
}

async function loreContent(
  page: Page,
  worldId: string,
  slug: string,
): Promise<string | null> {
  const result = await graphql<{
    data: { loreEntry: { content: string } | null };
  }>(
    page,
    `
      query E($worldId: UUID!, $slug: String!) {
        loreEntry(worldId: $worldId, slug: $slug) {
          content
        }
      }
    `,
    { worldId, slug },
  );
  return result.data.loreEntry?.content ?? null;
}

async function createLore(
  page: Page,
  worldId: string,
  title: string,
  content: string,
): Promise<string> {
  const made = await graphql<{ data: { createLoreEntry: { id: string } } }>(
    page,
    `
      mutation C($input: CreateLoreEntryInput!) {
        createLoreEntry(input: $input) {
          id
        }
      }
    `,
    { input: { worldId, title, content } },
  );
  return made.data.createLoreEntry.id;
}

test.describe("spec 039 US6: a takedown reaches everywhere the work went", () => {
  test("the copy goes dark with its source, the adopter's own work does not, and both come back", async ({
    page,
    browser,
  }) => {
    test.setTimeout(300_000);
    const suffix = uniqueSuffix();

    // ---- 1. user1 shares a collection: a lore entry and an ability ---------
    const sourceWorldId = await registerAndCreateWorld(
      page,
      `E2E Reach Source ${suffix}`,
      "e2ereachsrc",
    );
    const sourceTitle = `The Ember Chronicle ${suffix}`;
    const sourceContent = "Written in the first age, and nowhere else.";
    const sourceLoreId = await createLore(
      page,
      sourceWorldId,
      sourceTitle,
      sourceContent,
    );
    const ability = await graphql<{ data: { createAbility: { id: string } } }>(
      page,
      `
        mutation A($input: CreateAbilityInput!) {
          createAbility(input: $input) {
            id
          }
        }
      `,
      {
        input: {
          worldId: sourceWorldId,
          name: `Ember Ward ${suffix}`,
          classification: "SPELL",
          gmOnly: false,
        },
      },
    );

    const collection = await graphql<{
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
      { input: { worldId: sourceWorldId, name: `Embers ${suffix}` } },
    );
    const collectionId = collection.data.createCollection.id;
    for (const [memberType, memberId] of [
      ["lore", sourceLoreId],
      ["ability", ability.data.createAbility.id],
    ]) {
      const added = await graphql<{ errors?: { message: string }[] }>(
        page,
        `
          mutation M($input: AddCollectionMemberInput!) {
            addCollectionMember(input: $input) {
              id
            }
          }
        `,
        { input: { collectionId, memberType, memberId } },
      );
      expect(added.errors, `adding the ${memberType}`).toBeUndefined();
    }

    const shared = await graphql<{
      data: { createCollectionShareLink: { shareCode: string } };
    }>(
      page,
      `
        mutation S($collectionId: UUID!, $attestation: AttestationInput!) {
          createCollectionShareLink(
            collectionId: $collectionId
            attestation: $attestation
          ) {
            shareCode
          }
        }
      `,
      {
        collectionId,
        attestation: { termsVersionId: await currentSharingTermsVersion(page) },
      },
    );
    const shareCode = shared.data.createCollectionShareLink.shareCode;

    // ---- 2. user2 adopts it, renames the copy, writes a note beside it -----
    const adopterContext = await browser.newContext();
    const adopter = await adopterContext.newPage();
    try {
      const adopterWorldId = await registerAndCreateWorld(
        adopter,
        `E2E Reach Adopter ${suffix}`,
        "e2ereachadopt",
      );
      const copied = await graphql<{
        errors?: { message: string }[];
      }>(
        adopter,
        `
          mutation Copy($shareCode: String!, $destinationWorldId: UUID!) {
            copySharedCollectionToWorld(
              shareCode: $shareCode
              destinationWorldId: $destinationWorldId
            ) {
              created {
                id
              }
            }
          }
        `,
        { shareCode, destinationWorldId: adopterWorldId },
      );
      expect(copied.errors).toBeUndefined();

      const copy = (await loreIn(adopter, adopterWorldId)).find(
        (row) => row.title === sourceTitle,
      );
      expect(copy, "the lore entry was copied").toBeTruthy();

      const renamedTitle = `${sourceTitle} (with my notes)`;
      const renamed = await graphql<{
        data: { updateLoreEntry: { slug: string } };
        errors?: { message: string }[];
      }>(
        adopter,
        `
          mutation U($input: UpdateLoreEntryInput!) {
            updateLoreEntry(input: $input) {
              id
              slug
            }
          }
        `,
        { input: { loreEntryId: copy!.id, title: renamedTitle } },
      );
      expect(renamed.errors).toBeUndefined();
      // A rename regenerates the slug, and the entry is found by slug below.
      copy!.slug = renamed.data.updateLoreEntry.slug;

      const noteTitle = `My own note ${suffix}`;
      await createLore(
        adopter,
        adopterWorldId,
        noteTitle,
        "Mine, and nobody else's.",
      );

      // ---- 3. A takedown against user1's entry ---------------------------
      const notice = await graphql<{
        data: { submitTakedownNotice: { caseId: string } };
      }>(
        page,
        `
          mutation N($input: SubmitTakedownNoticeInput!) {
            submitTakedownNotice(input: $input) {
              caseId
            }
          }
        `,
        {
          input: {
            entityType: "WORLD_LORE_ENTRY",
            entityId: sourceLoreId,
            claimantName: `Reach Claimant ${suffix}`,
            claimantContact: "claimant@example.test",
            copyrightedWorkDescription: "The Ember Chronicle, first edition",
            infringingMaterialLocation: `The collection Embers ${suffix}`,
            goodFaithStatement: true,
            accuracyStatement: true,
            signature: `Reach Claimant ${suffix}`,
          },
        },
      );
      const caseId = notice.data.submitTakedownNotice.caseId;

      // ---- 4. In user2's world: the copy is dark, their note is not ------
      const afterTakedown = await loreIn(adopter, adopterWorldId);
      expect(
        afterTakedown.map((row) => row.title),
        "the copy is withheld from the list",
      ).not.toContain(renamedTitle);
      expect(
        afterTakedown.map((row) => row.title),
        "the adopter's own note is untouched (FR-023c)",
      ).toContain(noteTitle);
      // Still a row in their world, answered with the placeholder — not gone.
      // Spec 015's placeholder carries the notice in the title and nothing in
      // the body.
      const placeholder = await graphql<{
        data: {
          loreEntry: {
            title: string;
            content: string;
            moderated: boolean;
          } | null;
        };
      }>(
        adopter,
        `
          query E($worldId: UUID!, $slug: String!) {
            loreEntry(worldId: $worldId, slug: $slug) {
              title
              content
              moderated
            }
          }
        `,
        { worldId: adopterWorldId, slug: copy!.slug },
      );
      const seen = placeholder.data.loreEntry;
      expect(seen, "disabled, not deleted (FR-023a)").not.toBeNull();
      expect(seen!.moderated).toBe(true);
      expect(seen!.title).toContain("takedown");
      expect(seen!.content).not.toContain(sourceContent);

      // Told, on their own standing page, and not accused.
      await adopter.goto("/settings/standing");
      const told = adopter.getByTestId("standing-notice").filter({
        hasText: renamedTitle,
      });
      await expect(told).toBeVisible({ timeout: 15_000 });
      await expect(told).toContainText("You are not accused of anything");
      await expect(told).toContainText("not a strike against you");
      await expect(told).not.toContainText(`Reach Claimant ${suffix}`);
      await expect(adopter.getByTestId("standing-strike")).toHaveCount(0);

      // ---- 5. The link no longer serves the entry (FR-022) ---------------
      const preview = await graphql<{
        data: {
          sharedCollection: {
            members: { name: string }[];
            withheldCount: number;
          };
        };
      }>(
        page,
        `
          query P($shareCode: String!) {
            sharedCollection(shareCode: $shareCode) {
              members {
                name
              }
              withheldCount
            }
          }
        `,
        { shareCode },
      );
      expect(
        preview.data.sharedCollection.members.map((m) => m.name),
      ).not.toContain(sourceTitle);
      expect(preview.data.sharedCollection.withheldCount).toBe(1);

      // ---- 6. user1 files a counter-notice; the waiting period passes ----
      const counter = await graphql<{ errors?: { message: string }[] }>(
        page,
        `
          mutation K($input: SubmitCounterNoticeInput!) {
            submitCounterNotice(input: $input) {
              caseId
            }
          }
        `,
        {
          input: {
            caseId,
            removedMaterialDescription: sourceTitle,
            goodFaithMistakeStatement: true,
            consentToJurisdiction: true,
            contactInformation: "sharer@example.test",
            signature: "The Sharer",
          },
        },
      );
      expect(counter.errors).toBeUndefined();
      expect(
        elapseWaitingPeriod(caseId),
        "forwarded on the source and on the copy, with one date",
      ).toBe(2);

      // ---- 7. Both come back, the copy intact, nobody having asked -------
      expect(
        (await loreIn(page, sourceWorldId)).map((row) => row.title),
      ).toContain(sourceTitle);
      const restored = await loreIn(adopter, adopterWorldId);
      expect(
        restored.map((row) => row.title),
        "the copy comes back with the adopter's rename intact (FR-023d)",
      ).toContain(renamedTitle);
      expect(await loreContent(adopter, adopterWorldId, copy!.slug)).toBe(
        sourceContent,
      );

      await adopter.goto("/settings/standing");
      await expect(
        adopter
          .getByTestId("standing-notice")
          .filter({ hasText: `${renamedTitle} is back` }),
      ).toBeVisible({ timeout: 15_000 });
    } finally {
      await adopterContext.close();
    }
  });
});
