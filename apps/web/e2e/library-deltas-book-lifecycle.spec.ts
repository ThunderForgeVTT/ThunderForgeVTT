import { expect, test } from "./fixtures/test";
import { freshCredentials, graphql, register } from "./fixtures/helpers";
import {
  ADDED,
  aReadField,
  aWorldRunning,
  baseAsStored,
  BOOK,
  browseTheBook,
  deltaCount,
  deltaOrigins,
  type Entry,
  type Gql,
  MONSTERS,
  named,
  readInTheBook,
  sql,
  SYSTEM,
  uuid,
  worldReads,
} from "./fixtures/library-deltas";

/**
 * A world's own changes to the books it inherited, when the book itself
 * changes (spec 050 US2): the book removed from the shelf, and the book
 * re-read without an entry a table changed.
 */

test.describe.configure({ mode: "serial" });

test.describe("A world's changes to its books (spec 050 US2)", () => {
  /**
   * Spec 050 decision 5, carried to removal (owner, 2026-09-13; FR-061 as
   * amended): removing the book from the owner's shelf — the stronger act,
   * the book itself goes — takes a table's changes and hides and keeps what
   * it added. The removal preview says both, per world, before anything goes.
   */
  test("removing the book from the shelf keeps a table's additions", async ({
    page,
  }) => {
    test.setTimeout(300_000);

    await register(page, freshCredentials("deltasremove"));
    const compendiumId = await readInTheBook(page);
    const worldId = await aWorldRunning(page, "The Fen Table", compendiumId);

    const address = { w: worldId, c: compendiumId };
    const goblin = named(
      await worldReads(page, worldId, compendiumId),
      "GOBLIN",
    ) as Entry;
    const [field] = aReadField(goblin);
    for (const [query, variables] of [
      [
        `mutation C($w: UUID!, $c: UUID!, $f: JSON) {
           changeWorldEntry(worldId: $w, compendiumId: $c, kind: "${goblin.kind}", name: "GOBLIN", fieldValues: $f) { state }
         }`,
        { ...address, f: { [field]: { state: "clear", value: "99" } } },
      ],
      [
        `mutation H($w: UUID!, $c: UUID!) {
           hideWorldEntry(worldId: $w, compendiumId: $c, kind: "${goblin.kind}", name: "ADULT RED DRAGON") { state }
         }`,
        address,
      ],
      [
        `mutation A($w: UUID!, $c: UUID!) {
           addWorldEntry(worldId: $w, compendiumId: $c, kind: "creature", name: "${ADDED}", proseText: "Lives in the fen and bargains in teeth.") { state }
         }`,
        address,
      ],
    ] as const) {
      const done = await graphql<Gql<unknown>>(page, query, variables);
      expect(done.errors, JSON.stringify(done.errors)).toBeUndefined();
    }
    expect(Object.keys(deltaOrigins(worldId)).sort()).toEqual(
      ["ADULT RED DRAGON", "GOBLIN", ADDED].sort(),
    );

    // The preview: the book itself goes, and per world, what is lost and
    // what is kept.
    await page.goto("/library");
    await page
      .getByTestId("library-shelf")
      .locator("li")
      .filter({ hasText: BOOK })
      .getByTestId("remove-book")
      .click();
    const report = page.getByTestId("removal-report");
    await expect(report).toContainText("deletes the book itself");
    const here = report.getByTestId(`removal-world-${worldId}`);
    await expect(here).toContainText("The Fen Table");
    await expect(here.getByTestId("removal-deltas-lost")).toContainText(
      'changed: creature "GOBLIN"',
    );
    await expect(here.getByTestId("removal-deltas-lost")).toContainText(
      'hidden: creature "ADULT RED DRAGON"',
    );
    await expect(here.getByTestId("removal-deltas-lost")).not.toContainText(
      ADDED,
    );
    await expect(here.getByTestId("removal-additions-kept")).toContainText(
      `added: creature "${ADDED}"`,
    );
    expect(deltaCount(worldId), "the preview takes nothing").toBe(3);

    await page.getByTestId("remove-confirm").click();
    await expect(page.getByTestId("removal-report")).toHaveCount(0, {
      timeout: 30_000,
    });
    expect(
      sql(
        `SELECT count(*) FROM compendiums WHERE id = '${uuid(compendiumId)}';`,
      ),
    ).toBe("0");

    // In the database: only the addition, authored, detached, remembering
    // the book's title.
    expect(deltaOrigins(worldId)).toEqual({ [ADDED]: "Added Authored" });
    expect(
      sql(
        `SELECT coalesce(compendium_id::text, 'none') || '|' || written_beside_title
           FROM world_entry_deltas WHERE world_id = '${uuid(worldId)}';`,
      ),
    ).toBe(`none|${BOOK}`);

    // On the page: still listed, still readable, still authored and
    // shareable, labelled with the book that left.
    await page.goto(`/world/${worldId}/compendium?tab=books`);
    await expect(page.getByTestId("book-list-empty")).toBeVisible({
      timeout: 30_000,
    });
    const kept = page
      .getByTestId("kept-additions")
      .locator(`li[data-name="${ADDED}"]`);
    await expect(kept).toHaveCount(1);
    await expect(kept).toContainText(
      `written beside ${BOOK}, which has left the shelf`,
    );
    await expect(kept).toContainText("bargains in teeth");
    await expect(kept.getByTestId("world-entry-origin")).toHaveAttribute(
      "data-origin",
      "AUTHORED",
    );
    await expect(kept.getByTestId("world-entry-origin")).toHaveAttribute(
      "data-may-be-shared",
      "true",
    );
    const onTheWire = await graphql<
      Gql<{
        worldAdditionsWithoutBook: (Entry & {
          bookTitle: string;
          compendiumId: string | null;
        })[];
      }>
    >(
      page,
      `
        query K($w: UUID!) {
          worldAdditionsWithoutBook(worldId: $w) {
            name
            bookTitle
            compendiumId
            origin
            mayBeShared
          }
        }
      `,
      { w: worldId },
    );
    expect(onTheWire.data?.worldAdditionsWithoutBook).toEqual([
      {
        name: ADDED,
        bookTitle: BOOK,
        compendiumId: null,
        origin: "AUTHORED",
        mayBeShared: true,
      },
    ]);

    // Removed on purpose, by the one address it still has.
    await kept.getByTestId("remove-kept-addition").click();
    await expect(page.getByTestId("kept-additions")).toHaveCount(0);
    expect(deltaCount(worldId)).toBe(0);
  });

  /**
   * Spec 049 T079 to T082, 050 FR-006, FR-026, FR-027: the owner re-reads the
   * book, and this time leaves the goblin out. The base is version 2, every
   * delta is still stored, the other table's hide still applies, and the
   * goblin change that no longer has an entry is named — to the person who
   * re-read the book, as they finish, and at the table that holds it.
   */
  test("re-reading the book keeps every change, applies those that attach, and names the one that does not", async ({
    page,
  }) => {
    test.setTimeout(300_000);

    await register(page, freshCredentials("deltasreread"));
    const compendiumId = await readInTheBook(page);
    const changed = await aWorldRunning(page, "The Goblin Table", compendiumId);
    const hidden = await aWorldRunning(page, "The Dragon Table", compendiumId);

    const goblin = named(
      await worldReads(page, changed, compendiumId),
      "GOBLIN",
    ) as Entry;
    const [field] = aReadField(goblin);
    for (const [query, variables] of [
      [
        `mutation C($w: UUID!, $c: UUID!, $f: JSON) {
           changeWorldEntry(worldId: $w, compendiumId: $c, kind: "${goblin.kind}", name: "GOBLIN", fieldValues: $f) { state }
         }`,
        {
          w: changed,
          c: compendiumId,
          f: { [field]: { state: "clear", value: "99" } },
        },
      ],
      [
        `mutation H($w: UUID!, $c: UUID!) {
           hideWorldEntry(worldId: $w, compendiumId: $c, kind: "${goblin.kind}", name: "ADULT RED DRAGON") { state }
         }`,
        { w: hidden, c: compendiumId },
      ],
    ] as const) {
      const done = await graphql<Gql<unknown>>(page, query, variables);
      expect(done.errors, JSON.stringify(done.errors)).toBeUndefined();
    }
    const deltasBefore = sql(
      `SELECT string_agg(d::text, E'\\n' ORDER BY d.id) FROM world_entry_deltas d
        WHERE d.compendium_id = '${uuid(compendiumId)}';`,
    );
    const imported = baseAsStored(compendiumId);
    expect(
      sql(
        `SELECT base_version FROM compendiums WHERE id = '${uuid(compendiumId)}';`,
      ),
    ).toBe("1");

    // The same file again: the shelf recognises it and offers to replace it.
    await page.goto("/library");
    await page.getByTestId("import-system").selectOption(SYSTEM);
    await page.getByTestId("import-file").setInputFiles({
      name: BOOK,
      mimeType: "application/pdf",
      buffer: Buffer.from(MONSTERS, "latin1"),
    });
    await expect(page.getByTestId("import-duplicate")).toBeVisible({
      timeout: 30_000,
    });
    await page.getByTestId("import-replace").click();
    await expect(page.getByTestId("found-total")).toBeVisible({
      timeout: 120_000,
    });
    // This reading leaves the goblin out.
    const goblinRow = page
      .locator('[data-testid^="entry-"]')
      .filter({ hasText: "GOBLIN" });
    await expect(goblinRow).toHaveCount(1);
    await goblinRow.locator('[data-testid^="include-entry-"]').uncheck();
    await page.getByTestId("submit-import").click();

    // Told as they finish: the new version, and the stranded change by world.
    const replaced = page.getByTestId("import-replaced");
    await expect(replaced).toContainText("now reads as version 2", {
      timeout: 60_000,
    });
    const stranded = page.getByTestId("import-stranded");
    await expect(stranded).toContainText("The Goblin Table");
    await expect(stranded).toContainText("changed creature “GOBLIN”");
    await expect(stranded).toContainText("no longer has a creature");
    await expect(stranded).not.toContainText("The Dragon Table");

    // The rows: a new version of the base, and every delta exactly as it was.
    expect(
      sql(
        `SELECT base_version FROM compendiums WHERE id = '${uuid(compendiumId)}';`,
      ),
    ).toBe("2");
    expect(baseAsStored(compendiumId)).not.toBe(imported);
    expect(baseAsStored(compendiumId)).not.toContain("GOBLIN");
    expect(
      sql(
        `SELECT string_agg(d::text, E'\\n' ORDER BY d.id) FROM world_entry_deltas d
          WHERE d.compendium_id = '${uuid(compendiumId)}';`,
      ),
      "reported, not discarded",
    ).toBe(deltasBefore);

    // The hide still applies at the other table.
    expect(
      named(await worldReads(page, hidden, compendiumId), "ADULT RED DRAGON")
        ?.state,
    ).toBe("HIDDEN");

    // And the goblin's table is told too, where its books are browsed.
    await browseTheBook(page, changed);
    const unattached = page.getByTestId("unattached-deltas");
    await expect(unattached).toContainText("GOBLIN", { timeout: 15_000 });
    await expect(unattached).toContainText("no longer has a creature");
  });
});
