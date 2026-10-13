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
  named,
  openEntry,
  readInTheBook,
  sql,
  uuid,
  worldReads,
} from "./fixtures/library-deltas";

/**
 * A world's own changes to the books it inherited, end to end (spec 050 US2,
 * T077; FR-020 to FR-028, FR-052, FR-052a, FR-081, FR-087).
 *
 * The claims, each of which a unit test can only assert about code:
 *
 * 1. **An edit in one world reaches no other world, and not the base.** The
 *    base is compared **byte for byte** against what the import wrote, read
 *    straight out of Postgres, rather than trusted to be untouched because
 *    no code path writes it.
 * 2. **An addition is absent elsewhere; a hidden entry is present elsewhere.**
 * 3. **Origin is per entry** (FR-087): on one screen of one uploaded book, the
 *    entry this world changed says it stays with the account, and the entry
 *    this world added says it may be shared — on the page, on the wire, and in
 *    the column the sharing rules are enforced against.
 * 4. **What an entry was, and putting it back** (FR-024).
 */

test.describe.configure({ mode: "serial" });

test.describe("A world's changes to its books (spec 050 US2)", () => {
  test("an edit in one world reaches no other world and not the base, and origin is per entry", async ({
    page,
  }) => {
    test.setTimeout(300_000);

    await register(page, freshCredentials("deltas"));
    const compendiumId = await readInTheBook(page);
    // What the import produced, before any world exists.
    const imported = baseAsStored(compendiumId);
    expect(imported).toContain("GOBLIN");

    const here = await aWorldRunning(page, "The Changed Table", compendiumId);
    const there = await aWorldRunning(page, "The Other Table", compendiumId);

    const goblinBefore = named(
      await worldReads(page, here, compendiumId),
      "GOBLIN",
    );
    expect(goblinBefore?.state).toBe("INHERITED");
    const [field, bookValue] = aReadField(goblinBefore as Entry);
    const retuned = `${bookValue}9`;

    await browseTheBook(page, here);

    // Changed, through the page a Game Master uses.
    let goblin = await openEntry(page, "GOBLIN");
    await goblin.getByTestId("edit-entry").click();
    await goblin.locator(`[data-edit-field="${field}"]`).fill(retuned);
    await goblin.getByTestId("save-entry").click();
    goblin = await openEntry(page, "GOBLIN");
    await expect(goblin).toHaveAttribute("data-state", "CHANGED");
    await expect(goblin.getByTestId("world-entry-state")).toHaveText(
      "Changed in this world",
    );
    await expect(goblin.locator(`[data-field="${field}"]`)).toHaveText(retuned);
    // FR-024: what it was, beside what it is.
    await expect(
      goblin
        .getByTestId("entry-before")
        .locator(`[data-before-field="${field}"]`),
    ).toHaveText(bookValue);

    // Hidden.
    const dragon = await openEntry(page, "ADULT RED DRAGON");
    await dragon.getByTestId("hide-entry").click();
    await expect(
      page
        .getByTestId("world-book-browser")
        .locator('li[data-name="ADULT RED DRAGON"]'),
    ).toHaveCount(0);

    // Added.
    await page.getByTestId("add-entry-kind").fill("creature");
    await page.getByTestId("add-entry-name").fill(ADDED);
    await page
      .getByTestId("add-entry-text")
      .fill("Lives in the fen and bargains in teeth.");
    await page.getByTestId("add-entry-submit").click();
    const hag = await openEntry(page, ADDED);
    await expect(hag).toHaveAttribute("data-state", "ADDED");

    // FR-087, on the page: the changed entry and the added one, side by side,
    // with opposite rights.
    goblin = await openEntry(page, "GOBLIN");
    const goblinOrigin = goblin.getByTestId("world-entry-origin");
    await expect(goblinOrigin).toHaveAttribute("data-origin", "UPLOADED");
    await expect(goblinOrigin).toHaveAttribute("data-may-be-shared", "false");
    await expect(goblinOrigin).toHaveText("Uploaded — stays with this account");
    const hagOrigin = hag.getByTestId("world-entry-origin");
    await expect(hagOrigin).toHaveAttribute("data-origin", "AUTHORED");
    await expect(hagOrigin).toHaveAttribute("data-may-be-shared", "true");
    await expect(hagOrigin).toHaveText("Authored here — may be shared");

    // FR-087, on the wire and in the column the sharing rules read.
    const readHere = await worldReads(page, here, compendiumId);
    expect(named(readHere, "GOBLIN")).toMatchObject({
      state: "CHANGED",
      origin: "UPLOADED",
      mayBeShared: false,
    });
    expect(named(readHere, "ADULT RED DRAGON")).toMatchObject({
      state: "HIDDEN",
      origin: "UPLOADED",
      mayBeShared: false,
    });
    expect(named(readHere, ADDED)).toMatchObject({
      state: "ADDED",
      origin: "AUTHORED",
      mayBeShared: true,
    });
    expect(deltaOrigins(here)).toEqual({
      "ADULT RED DRAGON": "Hidden Uploaded",
      GOBLIN: "Changed Uploaded",
      [ADDED]: "Added Authored",
    });

    // FR-081, SC-005: the other world reads the book as the book has it —
    // the goblin unchanged, the dragon still there, no hag.
    const readThere = await worldReads(page, there, compendiumId);
    const otherGoblin = named(readThere, "GOBLIN") as Entry;
    expect(otherGoblin.state).toBe("INHERITED");
    expect(otherGoblin.fieldValues[field]?.value).toBe(bookValue);
    expect(named(readThere, "ADULT RED DRAGON")?.state).toBe("INHERITED");
    expect(named(readThere, ADDED)).toBeUndefined();
    expect(readThere).toHaveLength(2);
    expect(deltaCount(there)).toBe(0);

    await browseTheBook(page, there);
    const otherBrowser = page.getByTestId("world-book-browser");
    await expect(otherBrowser).toContainText("ADULT RED DRAGON");
    await expect(otherBrowser).not.toContainText(ADDED);
    await expect(
      otherBrowser.locator('li[data-name="GOBLIN"]'),
    ).toHaveAttribute("data-state", "INHERITED");

    // And the base: byte-identical to what the import wrote.
    expect(baseAsStored(compendiumId)).toBe(imported);

    // FR-024: put the goblin back, from the page.
    await browseTheBook(page, here);
    goblin = await openEntry(page, "GOBLIN");
    await goblin.getByTestId("restore-entry").click();
    goblin = await openEntry(page, "GOBLIN");
    await expect(goblin).toHaveAttribute("data-state", "INHERITED");
    await expect(goblin.locator(`[data-field="${field}"]`)).toHaveText(
      bookValue,
    );
    await expect(goblin.getByTestId("entry-before")).toHaveCount(0);

    // 050 FR-013 / 049 FR-046: switching the book off names what this world's
    // own changes lose, before anything is lost.
    await page.getByTestId("switch-off-book").click();
    const lost = page.getByTestId("deltas-lost");
    await expect(lost).toContainText('hidden: creature "ADULT RED DRAGON"');
    await expect(lost).not.toContainText(ADDED);
    await expect(lost).not.toContainText("GOBLIN");
    // Decision 5: the addition is named as staying, before anything goes.
    await expect(page.getByTestId("additions-kept")).toContainText(
      `added: creature "${ADDED}"`,
    );
    expect(deltaCount(here)).toBe(2);

    // And then: the hide goes with the book, the addition stays.
    await page.getByTestId("switch-off-confirm").click();
    await expect(page.getByTestId("book-list-empty")).toBeVisible();
    expect(deltaOrigins(here)).toEqual({ [ADDED]: "Added Authored" });

    // Still listed, still readable, still authored and shareable — on its
    // own, naming the book it was written beside.
    const kept = page
      .getByTestId("kept-additions")
      .locator(`li[data-name="${ADDED}"]`);
    await expect(kept).toHaveCount(1);
    await expect(kept).toContainText(`written beside ${BOOK}`);
    await expect(kept).toContainText("bargains in teeth");
    await expect(kept.getByTestId("world-entry-origin")).toHaveAttribute(
      "data-origin",
      "AUTHORED",
    );
    await expect(kept.getByTestId("world-entry-origin")).toHaveAttribute(
      "data-may-be-shared",
      "true",
    );
    const keptOnTheWire = await graphql<
      Gql<{ worldAdditionsWithoutBook: Entry[] }>
    >(
      page,
      `
        query K($w: UUID!) {
          worldAdditionsWithoutBook(worldId: $w) {
            name
            state
            origin
            mayBeShared
          }
        }
      `,
      { w: here },
    );
    expect(keptOnTheWire.data?.worldAdditionsWithoutBook).toEqual([
      { name: ADDED, state: "ADDED", origin: "AUTHORED", mayBeShared: true },
    ]);

    // Switched back on: the addition rejoins the book's page as the same
    // entry — one row, no copy — and the hide does not come back.
    const keptRow = sql(
      `SELECT id FROM world_entry_deltas WHERE world_id = '${uuid(here)}';`,
    );
    await page
      .getByTestId(`offered-book-${compendiumId}`)
      .getByTestId("switch-on-book")
      .click();
    await expect(page.getByTestId("book-list")).toContainText(BOOK);
    await expect(page.getByTestId("kept-additions")).toHaveCount(0);
    await page.getByTestId("browse-book").click();
    const browser = page.getByTestId("world-book-browser");
    await expect(browser.locator(`li[data-name="${ADDED}"]`)).toHaveCount(1);
    await expect(browser.locator(`li[data-name="${ADDED}"]`)).toHaveAttribute(
      "data-state",
      "ADDED",
    );
    await expect(
      browser.locator('li[data-name="ADULT RED DRAGON"]'),
    ).toHaveAttribute("data-state", "INHERITED");
    expect(deltaOrigins(here)).toEqual({ [ADDED]: "Added Authored" });
    expect(
      sql(
        `SELECT id FROM world_entry_deltas WHERE world_id = '${uuid(here)}';`,
      ),
      "the same row, not a second copy",
    ).toBe(keptRow);

    expect(baseAsStored(compendiumId)).toBe(imported);
  });
});
