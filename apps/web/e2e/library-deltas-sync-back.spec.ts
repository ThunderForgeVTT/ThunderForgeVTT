import { expect, test } from "./fixtures/test";
import { freshCredentials, graphql, register } from "./fixtures/helpers";
import {
  aReadField,
  aWorldRunning,
  baseAsStored,
  type Entry,
  type Gql,
  named,
  readInTheBook,
  sql,
  SYSTEM,
  uuid,
  UUID_PATTERN,
  worldReads,
} from "./fixtures/library-deltas";

/**
 * A world's own changes to the books it inherited, synced back to a
 * collection (spec 050 US6, T100): a later world inherits them, the syncing
 * world no longer holds them, the sync can be regretted, and no route
 * exists from a world to a book.
 */

test.describe.configure({ mode: "serial" });

test.describe("A world's changes to its books (spec 050 US2)", () => {
  test("syncing a collection back lands as a version a later world inherits, leaves the syncing world, can be regretted, and never reaches a book", async ({
    page,
  }) => {
    test.setTimeout(300_000);

    await register(page, freshCredentials("syncback"));
    const bookId = await readInTheBook(page);
    const COLLECTION = "Fen Folk";

    // A collection on the shelf, written through the product: version 3.
    const made = await graphql<Gql<{ createShelfCollection: { id: string } }>>(
      page,
      `
        mutation N($t: String!, $s: String!) {
          createShelfCollection(title: $t, systemId: $s) {
            id
          }
        }
      `,
      { t: COLLECTION, s: SYSTEM },
    );
    const collectionId = made.data?.createShelfCollection.id as string;
    expect(collectionId, JSON.stringify(made.errors)).toMatch(UUID_PATTERN);
    for (const [name, text] of [
      ["Mire Hag", "Lives in the fen and bargains in teeth."],
      ["Bog Eel", "Slick, and sore about it."],
    ]) {
      const wrote = await graphql<Gql<unknown>>(
        page,
        `
          mutation W($c: UUID!, $n: String!, $p: String) {
            writeShelfCollectionEntry(
              collectionId: $c
              kind: "creature"
              name: $n
              proseText: $p
            ) {
              id
            }
          }
        `,
        { c: collectionId, n: name, p: text },
      );
      expect(wrote.errors, JSON.stringify(wrote.errors)).toBeUndefined();
    }
    const versionOf = (id: string) =>
      sql(`SELECT base_version FROM compendiums WHERE id = '${uuid(id)}';`);
    expect(versionOf(collectionId)).toBe("3");

    // The syncing table runs the collection and the book; a neighbour runs
    // the collection and has changed the eel the syncing table hides.
    const fen = await aWorldRunning(page, "The Fen Table", collectionId);
    const on = await graphql<Gql<unknown>>(
      page,
      `
        mutation On($w: UUID!, $c: UUID!) {
          switchOnCompendium(worldId: $w, compendiumId: $c) {
            compendiumId
          }
        }
      `,
      { w: fen, c: bookId },
    );
    expect(on.errors, JSON.stringify(on.errors)).toBeUndefined();
    const river = await aWorldRunning(page, "The River Table", collectionId);

    const goblin = named(
      await worldReads(page, fen, bookId),
      "GOBLIN",
    ) as Entry;
    const [field] = aReadField(goblin);
    for (const [query, variables] of [
      [
        `mutation C($w: UUID!, $c: UUID!) {
           changeWorldEntry(worldId: $w, compendiumId: $c, kind: "creature", name: "Mire Hag", proseText: "She has moved to the river.") { state }
         }`,
        { w: fen, c: collectionId },
      ],
      [
        `mutation H($w: UUID!, $c: UUID!) {
           hideWorldEntry(worldId: $w, compendiumId: $c, kind: "creature", name: "Bog Eel") { state }
         }`,
        { w: fen, c: collectionId },
      ],
      [
        `mutation A($w: UUID!, $c: UUID!) {
           addWorldEntry(worldId: $w, compendiumId: $c, kind: "creature", name: "Reed Wisp", proseText: "A light where no one should be.") { state }
         }`,
        { w: fen, c: collectionId },
      ],
      [
        `mutation C($w: UUID!, $c: UUID!) {
           changeWorldEntry(worldId: $w, compendiumId: $c, kind: "creature", name: "Bog Eel", proseText: "Our eel bites.") { state }
         }`,
        { w: river, c: collectionId },
      ],
      [
        `mutation C($w: UUID!, $c: UUID!, $f: JSON) {
           changeWorldEntry(worldId: $w, compendiumId: $c, kind: "${goblin.kind}", name: "GOBLIN", fieldValues: $f) { state }
         }`,
        { w: fen, c: bookId, f: { [field]: { state: "clear", value: "99" } } },
      ],
    ] as const) {
      const done = await graphql<Gql<unknown>>(page, query, variables);
      expect(done.errors, JSON.stringify(done.errors)).toBeUndefined();
    }

    // FR-103: the plan is shown, change by change, before anything lands.
    await page.goto(`/world/${fen}/compendium?tab=books`);
    const collectionRow = page.getByTestId(`world-book-${collectionId}`);
    await expect(collectionRow).toBeVisible({ timeout: 30_000 });
    await collectionRow.getByTestId("browse-book").click();
    await collectionRow.getByTestId("sync-back-ask").click();
    const plan = collectionRow.getByTestId("sync-back-plan");
    await expect(plan).toBeVisible({ timeout: 15_000 });
    await expect(plan.getByTestId("sync-back-change")).toHaveCount(3);
    await expect(plan).toContainText("Rewritten: creature “Mire Hag”");
    await expect(plan).toContainText(
      "Lives in the fen and bargains in teeth. → She has moved to the river.",
    );
    await expect(plan).toContainText("Taken out: creature “Bog Eel”");
    await expect(plan).toContainText("Added: creature “Reed Wisp”");
    await expect(plan).toContainText("kept as version 3");
    expect(versionOf(collectionId), "nothing lands on showing").toBe("3");

    await plan.getByTestId("sync-back-confirm").click();
    const outcome = collectionRow.getByTestId("sync-back-outcome");
    await expect(outcome).toContainText("is at version 4", { timeout: 15_000 });
    await expect(outcome).toContainText("Version 3 is kept in your library");
    await expect(outcome.getByTestId("sync-back-stranded")).toContainText(
      "The River Table",
    );
    await expect(outcome).toContainText("Bog Eel");

    // FR-105: the syncing table no longer holds what it synced; the
    // neighbour's change is kept, stranded, as after a re-read.
    expect(versionOf(collectionId)).toBe("4");
    expect(
      sql(
        `SELECT count(*) FROM world_entry_deltas WHERE world_id = '${uuid(fen)}' AND compendium_id = '${uuid(collectionId)}';`,
      ),
    ).toBe("0");
    expect(
      sql(
        `SELECT count(*) FROM world_entry_deltas WHERE world_id = '${uuid(river)}';`,
      ),
      "reported, not discarded",
    ).toBe("1");
    const asTheShelfHasIt = (entries: Entry[]) =>
      entries
        .map((entry) => `${entry.name}|${entry.state}|${entry.proseText}`)
        .sort();
    const synced = [
      "Mire Hag|INHERITED|She has moved to the river.",
      "Reed Wisp|INHERITED|A light where no one should be.",
    ];
    expect(asTheShelfHasIt(await worldReads(page, fen, collectionId))).toEqual(
      synced,
    );

    // A table started afterwards inherits the synced change.
    const later = await aWorldRunning(page, "The Later Table", collectionId);
    expect(
      asTheShelfHasIt(await worldReads(page, later, collectionId)),
    ).toEqual(synced);

    // FR-089, FR-101, FR-102: no route to a book read in, through every
    // surface that offers a sync. The browsed book says why, in place of the
    // control...
    const bookBytes = baseAsStored(bookId);
    const bookRow = page.getByTestId(`world-book-${bookId}`);
    await bookRow.getByTestId("browse-book").click();
    await expect(bookRow.getByTestId("sync-back-never")).toContainText(
      "was read in from a book, so your changes stay in this world",
    );
    await expect(bookRow.getByTestId("sync-back-ask")).toHaveCount(0);
    // ...so does the switch-off report, which offers the sync for a
    // collection...
    await bookRow.getByTestId("switch-off-book").click();
    const report = page.getByTestId("switch-off-report");
    await expect(report.getByTestId("sync-back-never")).toBeVisible({
      timeout: 15_000,
    });
    await expect(report.getByTestId("sync-back-ask")).toHaveCount(0);
    await report.getByRole("button", { name: "Keep it on" }).click();
    // ...and the server refuses both fields, however they are asked.
    const planRefused = await graphql<Gql<unknown>>(
      page,
      `
        query P($w: UUID!, $c: UUID!) {
          worldSyncBackPlan(worldId: $w, compendiumId: $c) {
            stamp
          }
        }
      `,
      { w: fen, c: bookId },
    );
    expect(planRefused.errors?.[0]?.message).toContain(
      "so your changes stay in this world",
    );
    const syncRefused = await graphql<Gql<unknown>>(
      page,
      `
        mutation S($w: UUID!, $c: UUID!) {
          syncBackToCollection(
            worldId: $w
            compendiumId: $c
            stamp: "anything"
          ) {
            baseVersion
          }
        }
      `,
      { w: fen, c: bookId },
    );
    expect(syncRefused.errors?.[0]?.message).toContain(
      "so your changes stay in this world",
    );
    expect(baseAsStored(bookId), "the book, byte for byte").toBe(bookBytes);
    expect(versionOf(bookId)).toBe("1");
    expect(
      sql(
        `SELECT count(*) FROM shelf_collection_versions WHERE compendium_id = '${uuid(bookId)}';`,
      ),
    ).toBe("0");
    expect(
      sql(
        `SELECT count(*) FROM world_entry_deltas WHERE world_id = '${uuid(fen)}' AND compendium_id = '${uuid(bookId)}';`,
      ),
      "the book's change stays in the world",
    ).toBe("1");

    // US6 scenario 6: the sync is regretted from the library, and every
    // table reads the collection as it was.
    await page.goto("/library");
    const shelfRow = page
      .getByTestId("library-shelf")
      .locator('li[data-testid^="library-book-"]')
      .filter({ hasText: COLLECTION });
    await shelfRow.getByTestId("collection-history").click();
    const version3 = shelfRow.getByTestId("collection-version-3");
    await expect(version3).toContainText("Synced from The Fen Table", {
      timeout: 15_000,
    });
    await version3.getByTestId("restore-version").click();
    await shelfRow.getByTestId("restore-version-confirm").click();
    await expect(shelfRow.getByTestId("collection-version-4")).toContainText(
      "Went back to version 3",
      { timeout: 15_000 },
    );
    expect(versionOf(collectionId)).toBe("5");
    expect(
      asTheShelfHasIt(await worldReads(page, later, collectionId)),
    ).toEqual([
      "Bog Eel|INHERITED|Slick, and sore about it.",
      "Mire Hag|INHERITED|Lives in the fen and bargains in teeth.",
    ]);
    // The neighbour's change attaches again.
    expect(
      named(await worldReads(page, river, collectionId), "Bog Eel")?.proseText,
    ).toBe("Our eel bites.");
  });
});
