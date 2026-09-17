import { execFileSync } from "node:child_process";
import { expect, test, type Page } from "./fixtures/test";
import {
  freshCredentials,
  graphql,
  inviteAndJoinAsPlayer,
  register,
} from "./fixtures/helpers";

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
 * 5. **A Trusted Player may change what a world inherited, and a Player may
 *    not** (FR-020a, ADR-099) — the Player refused by the server, not merely
 *    shown no controls.
 *
 * The book is built here rather than shipped, following
 * `library-book-list.spec.ts`: a real book is copyrighted, and a binary
 * fixture tells nobody what it contains.
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
const ADDED = "Mire Hag";

type Gql<T> = { data?: T; errors?: { message: string }[] };

type ReadValue = { state: string; value?: string };

interface Entry {
  id: string;
  kind: string;
  name: string;
  fieldValues: Record<string, ReadValue>;
  proseText: string | null;
  state: string;
  origin: string;
  mayBeShared: boolean;
  before: { fieldValues: Record<string, ReadValue> } | null;
}

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

function uuid(value: string): string {
  if (!UUID_PATTERN.test(value)) {
    throw new Error(`Refusing to put a non-UUID into SQL: ${value}`);
  }
  return value;
}

/**
 * Reads this shard's database, following `library-book-list.spec.ts`.
 *
 * **Read-only, and only to measure.** Everything this spec changes it changes
 * through the product. Whether the base is byte-identical is the evidence the
 * central claim rests on, and asking the product to vouch for its own storage
 * would be asking the thing under test to mark its own work.
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

/**
 * Every stored byte of a book's entries, in a stable order, as Postgres's own
 * text rendering of each whole row — id, name, fields, prose, page, timestamps
 * and all. Equal strings are the same bytes; any write to any column of any
 * entry changes it.
 */
function baseAsStored(compendiumId: string): string {
  return sql(
    `SELECT string_agg(e::text, E'\\n' ORDER BY e.id)
       FROM compendium_entries e
      WHERE e.compendium_id = '${uuid(compendiumId)}';`,
  );
}

/** The origin column of each delta a world holds, by entry name. */
function deltaOrigins(worldId: string): Record<string, string> {
  const rows = sql(
    `SELECT name || '|' || form || '|' || origin FROM world_entry_deltas
      WHERE world_id = '${uuid(worldId)}' ORDER BY name;`,
  );
  return Object.fromEntries(
    rows
      .split("\n")
      .filter(Boolean)
      .map((row) => {
        const [name, form, origin] = row.split("|");
        return [name, `${form} ${origin}`];
      }),
  );
}

function deltaCount(worldId: string): number {
  return Number(
    sql(
      `SELECT count(*) FROM world_entry_deltas WHERE world_id = '${uuid(worldId)}';`,
    ),
  );
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

/** A world on the book's system with the book switched on. */
async function aWorldRunning(
  page: Page,
  name: string,
  compendiumId: string,
): Promise<string> {
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
  const worldId = made.data?.createWorld?.id;
  expect(worldId, JSON.stringify(made.errors)).toBeTruthy();

  const on = await graphql<Gql<unknown>>(
    page,
    `
      mutation On($w: UUID!, $c: UUID!) {
        switchOnCompendium(worldId: $w, compendiumId: $c) {
          compendiumId
        }
      }
    `,
    { w: worldId, c: compendiumId },
  );
  expect(on.errors, JSON.stringify(on.errors)).toBeUndefined();
  return worldId as string;
}

/** What a world reads from the book, through the product, hidden included. */
async function worldReads(
  page: Page,
  worldId: string,
  compendiumId: string,
): Promise<Entry[]> {
  const read = await graphql<
    Gql<{ worldCompendiumEntries: { entries: Entry[] } }>
  >(
    page,
    `
      query E($w: UUID!, $c: UUID!) {
        worldCompendiumEntries(
          worldId: $w
          compendiumId: $c
          showHidden: true
        ) {
          entries {
            id
            kind
            name
            fieldValues
            proseText
            state
            origin
            mayBeShared
            before {
              fieldValues
            }
          }
        }
      }
    `,
    { w: worldId, c: compendiumId },
  );
  expect(read.errors, JSON.stringify(read.errors)).toBeUndefined();
  return read.data?.worldCompendiumEntries.entries ?? [];
}

function named(entries: Entry[], name: string): Entry | undefined {
  return entries.find((entry) => entry.name === name);
}

/** The first declared field the reader found a value for on this entry. */
function aReadField(entry: Entry): [string, string] {
  const found = Object.entries(entry.fieldValues).find(
    ([, value]) => value.state !== "unread" && value.value,
  );
  expect(found, JSON.stringify(entry.fieldValues)).toBeTruthy();
  const [field, value] = found as [string, ReadValue];
  return [field, value.value as string];
}

async function browseTheBook(page: Page, worldId: string) {
  await page.goto(`/world/${worldId}/compendium?tab=books`);
  await expect(page.getByTestId("book-list")).toContainText(BOOK, {
    timeout: 30_000,
  });
  await page.getByTestId("browse-book").click();
  await expect(page.getByTestId("world-book-browser")).toBeVisible();
}

/** One entry's row in the browser, opened. */
async function openEntry(page: Page, name: string) {
  const row = page
    .getByTestId("world-book-browser")
    .locator(`li[data-name="${name}"]`);
  await expect(row).toHaveCount(1, { timeout: 15_000 });
  const details = row.locator("details");
  if ((await details.getAttribute("open")) === null) {
    await row.locator("summary").click();
  }
  return row;
}

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

  /**
   * FR-020a, ADR-099: a Trusted Player changes what the world inherited — and
   * a Player at the same table is refused every change by the server, not
   * merely given no controls.
   */
  test("a trusted player may change what a world inherited, and a player may not", async ({
    browser,
    page,
  }) => {
    test.setTimeout(480_000);

    await register(page, freshCredentials("deltasowner"));
    const compendiumId = await readInTheBook(page);
    const imported = baseAsStored(compendiumId);
    const worldId = await aWorldRunning(page, "A Trusted Table", compendiumId);

    const trusted = await inviteAndJoinAsPlayer(
      browser,
      page,
      worldId,
      "deltastrusted",
    );
    let player: Page | null = null;
    try {
      await page.goto(`/world/${worldId}/players`);
      const roleSelect = page
        .getByTestId("players-list")
        .locator('select[data-testid^="player-role-select-"]');
      await expect(roleSelect).toHaveCount(1, { timeout: 30_000 });
      await roleSelect.selectOption("TrustedPlayer");
      await expect(roleSelect).toHaveValue("TrustedPlayer", {
        timeout: 10_000,
      });

      player = await inviteAndJoinAsPlayer(
        browser,
        page,
        worldId,
        "deltasplayer",
      );

      // The Trusted Player, through the page.
      const goblinBefore = named(
        await worldReads(trusted, worldId, compendiumId),
        "GOBLIN",
      ) as Entry;
      const [field, bookValue] = aReadField(goblinBefore);
      await browseTheBook(trusted, worldId);
      let goblin = await openEntry(trusted, "GOBLIN");
      await goblin.getByTestId("edit-entry").click();
      await goblin
        .locator(`[data-edit-field="${field}"]`)
        .fill(`${bookValue}1`);
      await goblin.getByTestId("save-entry").click();
      goblin = await openEntry(trusted, "GOBLIN");
      await expect(goblin).toHaveAttribute("data-state", "CHANGED");
      expect(deltaOrigins(worldId)).toEqual({ GOBLIN: "Changed Uploaded" });
      expect(
        sql(
          `SELECT count(*) FROM world_entry_deltas d JOIN worlds w ON w.id = d.world_id
            WHERE d.world_id = '${uuid(worldId)}' AND d.changed_by <> w.created_by;`,
        ),
        "the change is recorded as the Trusted Player's",
      ).toBe("1");

      // The Player: no controls to change anything with...
      await player.goto(`/world/${worldId}/compendium?tab=books`);
      await expect(player.getByTestId("book-list")).toContainText(BOOK, {
        timeout: 30_000,
      });
      await expect(player.getByTestId("browse-book")).toHaveCount(0);
      await expect(player.getByTestId("edit-entry")).toHaveCount(0);

      // ...and every change refused when they ask the server directly.
      const attempts: [string, string, Record<string, unknown>][] = [
        [
          "changeWorldEntry",
          "$f: JSON",
          { f: { [field]: { state: "clear", value: "999" } } },
        ],
        ["hideWorldEntry", "", {}],
        ["restoreWorldEntry", "", {}],
      ];
      for (const [mutation, extraArgs, extraVars] of attempts) {
        const fieldArg = extraArgs ? " fieldValues: $f" : "";
        const tried = await graphql<Gql<unknown>>(
          player,
          `
            mutation M($w: UUID!, $c: UUID!, $k: String!, $n: String! ${extraArgs}) {
              ${mutation}(worldId: $w, compendiumId: $c, kind: $k, name: $n${fieldArg}) {
                state
              }
            }
          `,
          {
            w: worldId,
            c: compendiumId,
            k: goblinBefore.kind,
            n: "GOBLIN",
            ...extraVars,
          },
        );
        expect(
          tried.errors?.length,
          `${mutation}: ${JSON.stringify(tried)}`,
        ).toBeTruthy();
      }
      const added = await graphql<Gql<unknown>>(
        player,
        `
          mutation A($w: UUID!, $c: UUID!) {
            addWorldEntry(
              worldId: $w
              compendiumId: $c
              kind: "creature"
              name: "Player's Pet"
              proseText: "Mine."
            ) {
              state
            }
          }
        `,
        { w: worldId, c: compendiumId },
      );
      expect(added.errors?.length, JSON.stringify(added)).toBeTruthy();

      // Nothing the Player tried landed: the one change is still the Trusted
      // Player's, and the base is what the import wrote.
      expect(deltaOrigins(worldId)).toEqual({ GOBLIN: "Changed Uploaded" });
      const now = named(
        await worldReads(trusted, worldId, compendiumId),
        "GOBLIN",
      );
      expect(now?.fieldValues[field]?.value).toBe(`${bookValue}1`);
      expect(baseAsStored(compendiumId)).toBe(imported);
    } finally {
      await player?.context().close();
      await trusted.context().close();
    }
  });
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
