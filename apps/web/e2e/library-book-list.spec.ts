import { execFileSync } from "node:child_process";
import { expect, test, type Page } from "@playwright/test";
import {
  freshCredentials,
  graphql,
  inviteAndJoinAsPlayer,
  register,
} from "./fixtures/helpers";

/**
 * The book list, end to end (spec 050 US3, T065; FR-080, FR-085, FR-086).
 *
 * Three claims, and each is the kind a unit test can only assert about code
 * rather than prove about a running product:
 *
 * 1. **A second world does not store a second copy.** Measured in the
 *    database, in rows and in bytes, before and after — not inferred from the
 *    absence of a copy step in the source.
 * 2. **Switching a book off leaves nothing behind.** The claim that nothing
 *    was copied is only worth as much as the evidence that nothing was, so
 *    this looks for the content in the world's own tables as well as on the
 *    page.
 * 3. **A player sees the list and cannot change it** — not by the page, which
 *    renders them no controls, and not by asking the server directly.
 * 4. **A Trusted Player can** (spec 050 decision 8, ADR-099), and what they
 *    switch on comes from the **owner's** shelf, not their own (FR-010a) —
 *    shown with a Trusted Player who holds a book of their own.
 *
 * Ticking a book while creating a world and switching one on afterwards are
 * both exercised, one per world, and the two rows they leave are compared: FR-033
 * says they must be the same state, and the only honest way to show that is
 * to look at both.
 *
 * The book is built here rather than shipped, following
 * `account-library.spec.ts`: a real book is copyrighted, and a binary fixture
 * tells nobody what it contains.
 */

function pdfOf(lines: string[]): string {
  const content = lines.join("\n");
  const objects = [
    "<< /Type /Catalog /Pages 2 0 R >>",
    "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>",
    `<< /Length ${content.length} >>\nstream\n${content}\nendstream`,
    "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
  ];

  let pdf = "%PDF-1.4\n";
  const offsets: number[] = [];
  objects.forEach((body, index) => {
    offsets.push(pdf.length);
    pdf += `${index + 1} 0 obj\n${body}\nendobj\n`;
  });
  const xref = pdf.length;
  pdf += `xref\n0 ${objects.length + 1}\n0000000000 65535 f \n`;
  for (const offset of offsets) {
    pdf += `${offset.toString().padStart(10, "0")} 00000 n \n`;
  }
  pdf += `trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
  return pdf;
}

/** Two creatures, as the dnd5e creature pattern declares them. */
const MONSTERS = pdfOf([
  "BT /F1 18 Tf 72 720 Td (ADULT RED DRAGON) Tj ET",
  "BT /F1 9 Tf 72 700 Td (Gargantuan dragon, chaotic evil) Tj ET",
  "BT /F1 9 Tf 72 686 Td (Armor Class 22) Tj ET",
  "BT /F1 9 Tf 72 672 Td (Hit Points 546) Tj ET",
  "BT /F1 9 Tf 72 658 Td (Speed 40 ft., fly 80 ft.) Tj ET",
  "BT /F1 18 Tf 72 600 Td (GOBLIN) Tj ET",
  "BT /F1 9 Tf 72 580 Td (Small humanoid, neutral evil) Tj ET",
  "BT /F1 9 Tf 72 566 Td (Armor Class 15) Tj ET",
  "BT /F1 9 Tf 72 552 Td (Hit Points 7) Tj ET",
]);

const SYSTEM = "dnd5e";
const BOOK = "Monster Manual.pdf";

type Gql<T> = { data?: T; errors?: { message: string }[] };

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

function uuid(value: string): string {
  if (!UUID_PATTERN.test(value)) {
    throw new Error(`Refusing to put a non-UUID into SQL: ${value}`);
  }
  return value;
}

/**
 * Reads this shard's database, following `account-standing.spec.ts`.
 *
 * **Read-only, and only to measure.** Everything this spec changes it changes
 * through the product. What the product does not expose — how many bytes a
 * book occupies, and whether a world's own tables hold a copy of an entry — is
 * the evidence the claims rest on, and asking the product to report on its
 * own storage would be asking the thing under test to mark its own work.
 */
function sql(statement: string): string {
  const container =
    process.env.THUNDERFORGE_POSTGRES_CONTAINER ?? "thunderforge-postgres";
  const database = process.env.THUNDERFORGE_DB_NAME ?? "thunderforge";
  const dbUser = process.env.THUNDERFORGE_DB_USER ?? "postgres";
  return execFileSync(
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
    { input: statement, encoding: "utf-8", stdio: ["pipe", "pipe", "inherit"] },
  ).trim();
}

/** Every byte of content one book holds: its entries, as Postgres sizes them. */
function storedContent(compendiumId: string): { rows: number; bytes: number } {
  const [rows, bytes] = sql(
    `SELECT count(*), COALESCE(sum(pg_column_size(e.*)), 0)
       FROM compendium_entries e
      WHERE e.compendium_id = '${uuid(compendiumId)}';`,
  ).split("|");
  return { rows: Number(rows), bytes: Number(bytes) };
}

/**
 * Whether a world's own tables hold anything named like the book's entries.
 *
 * The places a copy would have to live if one had been made: the world's
 * actors, items, abilities and lore. Under the book list there is no step
 * that writes to any of them, and this is the check that there is not one
 * nobody meant.
 */
function copiesInWorld(worldId: string, name: string): number {
  const id = uuid(worldId);
  const safe = name.replace(/[^A-Za-z ]/g, "");
  return Number(
    sql(
      `SELECT
         (SELECT count(*) FROM world_actors WHERE world_id = '${id}' AND label ILIKE '${safe}')
       + (SELECT count(*) FROM world_items WHERE world_id = '${id}' AND name ILIKE '${safe}')
       + (SELECT count(*) FROM world_abilities WHERE world_id = '${id}' AND name ILIKE '${safe}')
       + (SELECT count(*) FROM world_lore_entries WHERE world_id = '${id}' AND title ILIKE '${safe}');`,
    ),
  );
}

/** The book-list row a world has for a book, or null. */
function linkOf(
  worldId: string,
  compendiumId: string,
): { hash: string; parser: string } | null {
  const row = sql(
    `SELECT base_source_hash, base_parser_version FROM world_books
      WHERE world_id = '${uuid(worldId)}'
        AND compendium_id = '${uuid(compendiumId)}';`,
  );
  if (!row) return null;
  const [hash, parser] = row.split("|");
  return { hash, parser };
}

/** Read the book in from the shelf, and return its compendium id. */
async function readInTheBook(page: Page): Promise<string> {
  await page.goto("/library");
  await page.getByTestId("import-system").selectOption(SYSTEM);
  await page.getByTestId("import-file").setInputFiles({
    name: BOOK,
    mimeType: "application/pdf",
    buffer: Buffer.from(MONSTERS, "latin1"),
  });
  await expect(page.getByTestId("found-total")).toBeVisible({
    timeout: 120_000,
  });
  await page.getByTestId("submit-import").click();
  await expect(page.getByTestId("library-shelf")).toContainText(BOOK, {
    timeout: 60_000,
  });

  const href = await page
    .getByTestId("library-shelf")
    .getByTestId("open-book")
    .first()
    .getAttribute("href");
  const id = /\/library\/([^/]+)$/.exec(href ?? "")?.[1];
  expect(id, "the shelf should link to the book").toBeTruthy();
  return id as string;
}

/** A world on the book's system, made without the creation form. */
async function aWorld(page: Page, name: string): Promise<string> {
  const made = await graphql<Gql<{ createWorld: { id: string } }>>(
    page,
    `
      mutation CW($input: GraphQLCreateWorldInput!) {
        createWorld(input: $input) {
          id
        }
      }
    `,
    { input: { name, gameSystemId: SYSTEM } },
  );
  const id = made.data?.createWorld?.id;
  expect(id, JSON.stringify(made.errors)).toBeTruthy();
  return id as string;
}

async function openBooks(page: Page, worldId: string) {
  await page.goto(`/world/${worldId}/compendium?tab=books`);
  await expect(page.getByTestId("world-book-list")).toBeVisible({
    timeout: 30_000,
  });
}

test.describe.configure({ mode: "serial" });

test.describe("The book list (spec 050 US3)", () => {
  test("a second world stores no second copy, and switching off leaves nothing", async ({
    page,
  }) => {
    test.setTimeout(300_000);

    await register(page, freshCredentials("booklist"));
    const compendiumId = await readInTheBook(page);
    const beforeAnyWorld = storedContent(compendiumId);
    expect(beforeAnyWorld.rows).toBe(2);

    // World one: the book ticked **while the world is being created** — the
    // creation path of FR-033.
    await page.goto("/worlds/create");
    await page.locator("#world-name").fill("The First Table");
    const offered = await page.request.get("/api/systems").then(
      (response) =>
        response.json() as Promise<{
          systems: { id: string; title: string }[];
        }>,
    );
    const title = offered.systems.find((system) => system.id === SYSTEM)?.title;
    expect(title, "the book's system must be installed").toBeTruthy();
    await page.getByRole("combobox", { name: "Game system" }).click();
    await page.getByRole("option", { name: title as string }).click();
    await page.getByTestId(`create-world-book-${compendiumId}`).check();
    await page.getByRole("button", { name: /create world/i }).click();
    await page.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 30_000 });
    const firstWorld = /\/world\/([^/]+)\/staging$/.exec(
      new URL(page.url()).pathname,
    )?.[1] as string;

    await openBooks(page, firstWorld);
    await expect(page.getByTestId("book-list")).toContainText(BOOK);
    const afterOneWorld = storedContent(compendiumId);

    // World two: made empty, then the book switched on from its Books tab —
    // the later path of FR-033.
    const secondWorld = await aWorld(page, "The Second Table");
    await openBooks(page, secondWorld);
    await expect(page.getByTestId("book-list-empty")).toBeVisible();
    await page
      .getByTestId(`offered-book-${compendiumId}`)
      .getByTestId("switch-on-book")
      .click();
    await expect(page.getByTestId("book-list")).toContainText(BOOK);
    const afterTwoWorlds = storedContent(compendiumId);

    // FR-080, SC-002: stored content did not grow — not by a row, not by a
    // byte — when either world switched the book on.
    expect(afterOneWorld).toEqual(beforeAnyWorld);
    expect(afterTwoWorlds).toEqual(beforeAnyWorld);

    // FR-033: the creation tick and the later switch left the same state.
    const firstLink = linkOf(firstWorld, compendiumId);
    const secondLink = linkOf(secondWorld, compendiumId);
    expect(firstLink).not.toBeNull();
    expect(secondLink).toEqual(firstLink);

    // FR-031: the content is genuinely there, fetched — the flat numbers
    // above are not flat because nothing happened.
    await page.getByTestId("browse-book").click();
    await expect(page.getByTestId("world-entry-count")).toContainText("of 2");
    await expect(page.getByTestId("world-book-browser")).toContainText(
      "GOBLIN",
    );
    await expect(page.getByTestId("world-book-browser")).toContainText(
      `${BOOK}, page 1`,
    );
    expect(copiesInWorld(secondWorld, "GOBLIN")).toBe(0);

    // FR-013: what goes is named before it goes, and asking takes nothing.
    await page.getByTestId("switch-off-book").click();
    await expect(page.getByTestId("switch-off-report")).toContainText(
      "2 entries",
    );
    await expect(page.getByTestId("in-use")).toContainText("GOBLIN");
    expect(linkOf(secondWorld, compendiumId)).not.toBeNull();

    // FR-032, FR-086: and then it goes, leaving nothing in the world.
    await page.getByTestId("switch-off-confirm").click();
    await expect(page.getByTestId("book-list-empty")).toBeVisible();
    await expect(page.getByTestId("world-book-list")).not.toContainText(
      "GOBLIN",
    );
    expect(linkOf(secondWorld, compendiumId)).toBeNull();
    expect(copiesInWorld(secondWorld, "GOBLIN")).toBe(0);
    expect(copiesInWorld(secondWorld, "ADULT RED DRAGON")).toBe(0);

    const refetch = await graphql<Gql<unknown>>(
      page,
      `
        query E($w: UUID!, $c: UUID!) {
          worldCompendiumEntries(worldId: $w, compendiumId: $c) {
            total
          }
        }
      `,
      { w: secondWorld, c: compendiumId },
    );
    expect(
      refetch.errors?.length,
      "a switched-off book must not be served to the world it left",
    ).toBeTruthy();

    // The shelf and the other world are untouched: switching off is a link
    // going, not content going.
    expect(storedContent(compendiumId)).toEqual(beforeAnyWorld);
    expect(linkOf(firstWorld, compendiumId)).not.toBeNull();
  });

  /**
   * FR-035, FR-036, FR-085, SC-003a: the player sees what the table is
   * running, and nothing on the page or on the wire lets them change it.
   */
  test("a player sees the book list and cannot change it", async ({
    browser,
    page,
  }) => {
    test.setTimeout(300_000);

    await register(page, freshCredentials("booklistgm"));
    const compendiumId = await readInTheBook(page);
    const worldId = await aWorld(page, "A Table With Players");
    await openBooks(page, worldId);
    await page
      .getByTestId(`offered-book-${compendiumId}`)
      .getByTestId("switch-on-book")
      .click();
    await expect(page.getByTestId("book-list")).toContainText(BOOK);

    const player = await inviteAndJoinAsPlayer(
      browser,
      page,
      worldId,
      "booklistplayer",
    );
    try {
      await openBooks(player, worldId);

      // They see it.
      await expect(player.getByTestId("book-list")).toContainText(BOOK);
      await expect(player.getByTestId("world-book-list")).toContainText(
        "Only its Game Masters and Trusted Players change this list",
      );

      // They are given nothing to change it with, and nothing of its content.
      await expect(player.getByTestId("switch-off-book")).toHaveCount(0);
      await expect(player.getByTestId("switch-on-book")).toHaveCount(0);
      await expect(player.getByTestId("browse-book")).toHaveCount(0);
      await expect(player.getByTestId("books-offered")).toHaveCount(0);
      await expect(player.locator("body")).not.toContainText("GOBLIN");
      await expect(player.locator("body")).not.toContainText(
        "ADULT RED DRAGON",
      );

      // And asking the server directly changes nothing either. A control that
      // is merely hidden is not a permission.
      const off = await graphql<Gql<unknown>>(
        player,
        `
          mutation Off($w: UUID!, $c: UUID!) {
            switchOffCompendium(worldId: $w, compendiumId: $c, confirm: true) {
              switchedOff
            }
          }
        `,
        { w: worldId, c: compendiumId },
      );
      expect(off.errors?.length, JSON.stringify(off)).toBeTruthy();

      const read = await graphql<Gql<unknown>>(
        player,
        `
          query E($w: UUID!, $c: UUID!) {
            worldCompendiumEntries(worldId: $w, compendiumId: $c) {
              entries {
                name
              }
            }
          }
        `,
        { w: worldId, c: compendiumId },
      );
      expect(read.errors?.length, JSON.stringify(read)).toBeTruthy();

      expect(linkOf(worldId, compendiumId)).not.toBeNull();
      await player.reload();
      await expect(player.getByTestId("book-list")).toContainText(BOOK, {
        timeout: 30_000,
      });
    } finally {
      await player.context().close();
    }
  });

  /**
   * Spec 050 decision 8, FR-010a, ADR-099: the Owner makes somebody a Trusted
   * Player from the Players page, and that person switches a book on — the
   * owner's book, offered from the owner's shelf, while a book on their own
   * shelf is never offered. A Player at the same table still cannot.
   */
  test("a trusted player switches the owner's book on, and a player still cannot", async ({
    browser,
    page,
  }) => {
    test.setTimeout(480_000);

    await register(page, freshCredentials("booklistowner"));
    const ownersBook = await readInTheBook(page);
    const worldId = await aWorld(page, "A Table With A Trusted Friend");

    const trusted = await inviteAndJoinAsPlayer(
      browser,
      page,
      worldId,
      "booklisttrusted",
    );
    let player: Page | null = null;
    try {
      // Granted through the page an Owner actually uses. The trusted friend
      // is the only other member, so theirs is the only role control.
      await page.goto(`/world/${worldId}/players`);
      const roleSelect = page
        .getByTestId("players-list")
        .locator('select[data-testid^="player-role-select-"]');
      await expect(roleSelect).toHaveCount(1, { timeout: 30_000 });
      await roleSelect.selectOption("TrustedPlayer");
      await expect(roleSelect).toHaveValue("TrustedPlayer", {
        timeout: 10_000,
      });

      // A book of their own, of the same system, so that "the owner's shelf"
      // is a claim with something to be wrong about.
      const theirOwnBook = await readInTheBook(trusted);
      expect(theirOwnBook).not.toBe(ownersBook);

      player = await inviteAndJoinAsPlayer(
        browser,
        page,
        worldId,
        "booklistplain",
      );

      await openBooks(trusted, worldId);
      await expect(trusted.getByTestId("book-list-empty")).toBeVisible();
      await expect(trusted.getByTestId("books-offered")).toContainText(
        "From the world owner's library",
      );
      await expect(
        trusted.getByTestId(`offered-book-${theirOwnBook}`),
      ).toHaveCount(0);
      await trusted
        .getByTestId(`offered-book-${ownersBook}`)
        .getByTestId("switch-on-book")
        .click();
      await expect(trusted.getByTestId("book-list")).toContainText(BOOK);

      // In the database: the owner's book is on, switched on by somebody who
      // is not the owner, and the Trusted Player's own book is not.
      expect(linkOf(worldId, ownersBook)).not.toBeNull();
      expect(linkOf(worldId, theirOwnBook)).toBeNull();
      expect(
        sql(
          `SELECT count(*) FROM world_books wb JOIN worlds w ON w.id = wb.world_id
            WHERE wb.world_id = '${uuid(worldId)}'
              AND wb.switched_on_by <> w.created_by;`,
        ),
      ).toBe("1");

      // Asking the server for their own book directly is refused too.
      const own = await graphql<Gql<unknown>>(
        trusted,
        `
          mutation On($w: UUID!, $c: UUID!) {
            switchOnCompendium(worldId: $w, compendiumId: $c) {
              compendiumId
            }
          }
        `,
        { w: worldId, c: theirOwnBook },
      );
      expect(own.errors?.length, JSON.stringify(own)).toBeTruthy();
      expect(linkOf(worldId, theirOwnBook)).toBeNull();

      // Arranging the books is not reading them: browsing stays a Game
      // Master's, and the Trusted Player is not offered it.
      await expect(trusted.getByTestId("switch-off-book")).toHaveCount(1);
      await expect(trusted.getByTestId("browse-book")).toHaveCount(0);

      // The Player at the same table: shown the list, given nothing to change
      // it with, and refused by the server when they ask anyway.
      await openBooks(player, worldId);
      await expect(player.getByTestId("book-list")).toContainText(BOOK);
      await expect(player.getByTestId("switch-off-book")).toHaveCount(0);
      await expect(player.getByTestId("switch-on-book")).toHaveCount(0);
      await expect(player.getByTestId("books-offered")).toHaveCount(0);

      const offered = await graphql<Gql<unknown>>(
        player,
        `
          query O($w: UUID!) {
            compendiumsOfferedToWorld(worldId: $w) {
              id
            }
          }
        `,
        { w: worldId },
      );
      expect(offered.errors?.length, JSON.stringify(offered)).toBeTruthy();
      const off = await graphql<Gql<unknown>>(
        player,
        `
          mutation Off($w: UUID!, $c: UUID!) {
            switchOffCompendium(worldId: $w, compendiumId: $c, confirm: true) {
              switchedOff
            }
          }
        `,
        { w: worldId, c: ownersBook },
      );
      expect(off.errors?.length, JSON.stringify(off)).toBeTruthy();
      expect(linkOf(worldId, ownersBook)).not.toBeNull();

      // And the Trusted Player can take it off again, report first.
      await trusted.getByTestId("switch-off-book").click();
      await expect(trusted.getByTestId("switch-off-report")).toContainText(
        "2 entries",
      );
      await trusted.getByTestId("switch-off-confirm").click();
      await expect(trusted.getByTestId("book-list-empty")).toBeVisible();
      expect(linkOf(worldId, ownersBook)).toBeNull();
    } finally {
      await player?.context().close();
      await trusted.context().close();
    }
  });
});
