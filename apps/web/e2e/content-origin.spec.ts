import { execFileSync } from "node:child_process";
import { expect, test, type Page } from "@playwright/test";
import {
  currentSharingTermsVersion,
  freshCredentials,
  graphql,
  register,
  uniqueSuffix,
} from "./fixtures/helpers";

/**
 * What may not leave, end to end (spec 049 US4, T055; FR-050 to FR-057,
 * ADR-097).
 *
 * The server tests prove the rule where it lives: the database refuses to put
 * uploaded content in a collection, whoever writes the row. What only the
 * running product can show is that every route a person can actually take
 * outward meets that rule — and, as much the point, that **authored content
 * beside the book is untouched**. The rule restricts an origin, not a subject,
 * so every refusal below has an allowance next to it made by the same person,
 * in the same world, through the same call.
 *
 * # The routes, as they exist in this codebase
 *
 * - **Collection adoption** — `addCollectionMember`. The door into the only
 *   container that is shared, downloaded, adopted or rescued.
 * - **Share** — `createCollectionShareLink`, read by anyone holding the link
 *   (ADR-070).
 * - **Publish** — spec 039 calls every share link a publish beyond the world;
 *   the single-artifact form is `createItemShareLink` (with its actor and
 *   ability siblings, which have the same guard).
 * - **Export** — `exportMyData`, the account download.
 * - And **adoption into another world** — `copySharedCollectionToWorld` —
 *   because that is where shared content lands.
 *
 * None of share, publish or adoption into a world takes an argument that can
 * name a book entry: they carry a collection or a world artifact. So for those
 * the refusal is shown where uploaded content would have to enter them, and
 * the assertion is that what they then carry has none of it. That is the ADR's
 * design rather than a gap in this test — an invariant at the door, not a
 * check at each route — and it is stated here so nobody reads the absence of
 * a "share this book" call as an oversight.
 *
 * # The licence, deliberately
 *
 * The document read in says on its face that it is openly licensed. It makes
 * no difference (FR-056), and nothing in the product asks.
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

/** One creature, in a book that announces an open licence. */
const OPEN_BOOK = pdfOf([
  "BT /F1 9 Tf 72 760 Td (This work is licensed under the Creative Commons Attribution 4.0 International License.) Tj ET",
  "BT /F1 18 Tf 72 720 Td (GOBLIN) Tj ET",
  "BT /F1 9 Tf 72 700 Td (Small humanoid, neutral evil) Tj ET",
  "BT /F1 9 Tf 72 686 Td (Armor Class 15) Tj ET",
  "BT /F1 9 Tf 72 672 Td (Hit Points 7) Tj ET",
]);

const SYSTEM = "dnd5e";
const BOOK = "System Reference Document (CC-BY-4.0).pdf";
const CREATURE = "GOBLIN";

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
 * Reads this shard's database, following `library-book-list.spec.ts`.
 *
 * **Read-only, and only to measure** whether a world's own tables gained a
 * copy of the creature — the one question the product has no reason to
 * answer about itself.
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

/** Anything in a world's own tables named like `name`. */
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

/** FR-053, FR-056a: the reason names the origin and the two routes left. */
function expectOriginRefusal(message: string | undefined, route: string) {
  expect(message, `${route} must refuse with a reason`).toBeTruthy();
  expect(message, route).toContain("uploaded");
  expect(message, route).toContain("author it");
  expect(message, route).toContain("system pack");
  // FR-056: nothing about a licence was weighed, so nothing about one is said.
  expect(message?.toLowerCase(), route).not.toContain("licen");
}

async function readInTheBook(page: Page): Promise<string> {
  await page.goto("/library");
  await page.getByTestId("import-system").selectOption(SYSTEM);
  await page.getByTestId("import-file").setInputFiles({
    name: BOOK,
    mimeType: "application/pdf",
    buffer: Buffer.from(OPEN_BOOK, "latin1"),
  });
  await expect(page.getByTestId("found-total")).toBeVisible({
    timeout: 120_000,
  });
  await page.getByTestId("submit-import").click();
  await expect(page.getByTestId("library-shelf")).toContainText(BOOK, {
    timeout: 60_000,
  });
  // FR-051: recorded as uploaded, with nobody asked — the licence notice in
  // the file did not change what the shelf says about it.
  await expect(page.getByTestId("book-origin")).toContainText(
    "stays with this account",
  );

  const href = await page
    .getByTestId("library-shelf")
    .getByTestId("open-book")
    .first()
    .getAttribute("href");
  const id = /\/library\/([^/]+)$/.exec(href ?? "")?.[1];
  expect(id, "the shelf should link to the book").toBeTruthy();
  return id as string;
}

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

test.describe.configure({ mode: "serial" });

test.describe("What may not leave (spec 049 US4)", () => {
  test("every route out refuses the book and carries the homebrew beside it", async ({
    page,
    browser,
  }) => {
    test.setTimeout(300_000);
    const suffix = uniqueSuffix();

    await register(page, freshCredentials("origin"));
    const compendiumId = await readInTheBook(page);
    const worldId = await aWorld(page, `The Open Table ${suffix}`);

    // FR-055: the book is fully usable at the owner's own table. Switching it
    // on is not sharing (050 FR-051) and is not refused.
    const switched = await graphql<Gql<unknown>>(
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
    expect(switched.errors, JSON.stringify(switched.errors)).toBeUndefined();

    const entries = await graphql<
      Gql<{ compendiumEntries: { entries: { id: string; name: string }[] } }>
    >(
      page,
      `
        query E($c: UUID!) {
          compendiumEntries(compendiumId: $c) {
            entries {
              id
              name
            }
          }
        }
      `,
      { c: compendiumId },
    );
    const creature = entries.data?.compendiumEntries.entries.find(
      (entry) => entry.name === CREATURE,
    );
    expect(creature, JSON.stringify(entries)).toBeTruthy();
    const entryId = creature?.id as string;

    // Authored by hand, in the same world, by the same person.
    const loreTitle = `The Warren Beneath ${suffix}`;
    const lore = await graphql<Gql<{ createLoreEntry: { id: string } }>>(
      page,
      `
        mutation L($input: CreateLoreEntryInput!) {
          createLoreEntry(input: $input) {
            id
          }
        }
      `,
      {
        input: {
          worldId,
          title: loreTitle,
          content: "Goblins of our own devising.",
        },
      },
    );
    const loreId = lore.data?.createLoreEntry.id as string;
    expect(loreId, JSON.stringify(lore.errors)).toBeTruthy();

    const itemName = `Warren Lantern ${suffix}`;
    const item = await graphql<Gql<{ createItem: { id: string } }>>(
      page,
      `
        mutation I($input: CreateItemInput!) {
          createItem(input: $input) {
            id
          }
        }
      `,
      { input: { worldId, name: itemName, description: null } },
    );
    const itemId = item.data?.createItem.id as string;
    expect(itemId, JSON.stringify(item.errors)).toBeTruthy();

    const collection = await graphql<Gql<{ createCollection: { id: string } }>>(
      page,
      `
        mutation C($input: CreateCollectionInput!) {
          createCollection(input: $input) {
            id
          }
        }
      `,
      { input: { worldId, name: `The Warren ${suffix}` } },
    );
    const collectionId = collection.data?.createCollection.id as string;
    expect(collectionId, JSON.stringify(collection.errors)).toBeTruthy();

    const ADD = `mutation A($input: AddCollectionMemberInput!) {
      addCollectionMember(input: $input) { id }
    }`;

    // ── Collection adoption ────────────────────────────────────────────────
    // Refused: the entry, and the whole book, each as uploaded.
    for (const [memberType, memberId] of [
      ["compendium_entry", entryId],
      ["compendium", compendiumId],
    ]) {
      const refused = await graphql<Gql<{ addCollectionMember: unknown }>>(
        page,
        ADD,
        { input: { collectionId, memberType, memberId } },
      );
      expect(refused.data?.addCollectionMember ?? null).toBeNull();
      expectOriginRefusal(
        refused.errors?.[0]?.message,
        `adopting a ${memberType} into a collection`,
      );
    }
    // Allowed: the homebrew beside it.
    for (const [memberType, memberId] of [
      ["lore", loreId],
      ["item", itemId],
    ]) {
      const added = await graphql<Gql<{ addCollectionMember: { id: string } }>>(
        page,
        ADD,
        { input: { collectionId, memberType, memberId } },
      );
      expect(added.errors, JSON.stringify(added.errors)).toBeUndefined();
    }

    const attestation = {
      termsVersionId: await currentSharingTermsVersion(page),
    };

    // ── Share ──────────────────────────────────────────────────────────────
    // Allowed for the authored collection; and what the link hands a signed-out
    // stranger is the homebrew, with nothing of the book in it.
    const shared = await graphql<
      Gql<{ createCollectionShareLink: { shareCode: string } }>
    >(
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
      { collectionId, attestation },
    );
    const shareCode = shared.data?.createCollectionShareLink.shareCode;
    expect(shareCode, JSON.stringify(shared.errors)).toBeTruthy();

    // Signed out, through the public endpoint the collection page itself
    // uses (ADR-070): what anybody holding the link receives.
    const stranger = await browser.newContext();
    const preview = (await (
      await stranger.request.post("/api/graphql/public", {
        headers: { "Content-Type": "application/json" },
        data: {
          query: `
            query P($shareCode: String!) {
              sharedCollection(shareCode: $shareCode) {
                members {
                  name
                }
                withheldCount
              }
            }
          `,
          variables: { shareCode },
        },
      })
    ).json()) as Gql<{
      sharedCollection: {
        members: { name: string }[];
        withheldCount: number;
      };
    }>;
    const names = preview.data?.sharedCollection.members.map((m) => m.name);
    expect(names, JSON.stringify(preview.errors)).toEqual(
      expect.arrayContaining([loreTitle, itemName]),
    );
    expect(names).toHaveLength(2);
    expect(JSON.stringify(preview)).not.toContain(CREATURE);
    await stranger.close();

    // ── Publish ────────────────────────────────────────────────────────────
    // Allowed: the authored item, published on its own.
    const published = await graphql<
      Gql<{ createItemShareLink: { shareCode: string } }>
    >(
      page,
      `
        mutation P($itemId: UUID!, $attestation: AttestationInput!) {
          createItemShareLink(itemId: $itemId, attestation: $attestation) {
            shareCode
          }
        }
      `,
      { itemId, attestation },
    );
    expect(
      published.data?.createItemShareLink.shareCode,
      JSON.stringify(published.errors),
    ).toBeTruthy();
    // Refused: the book's creature, named where an item is expected. The
    // publish route has no way to name uploaded content, so this mints
    // nothing — the absence of a code is the assertion.
    const smuggled = await graphql<
      Gql<{ createItemShareLink: { shareCode: string } }>
    >(
      page,
      `
        mutation P($itemId: UUID!, $attestation: AttestationInput!) {
          createItemShareLink(itemId: $itemId, attestation: $attestation) {
            shareCode
          }
        }
      `,
      { itemId: entryId, attestation },
    );
    expect(smuggled.data?.createItemShareLink ?? null).toBeNull();
    expect(smuggled.errors?.length ?? 0).toBeGreaterThan(0);

    // ── Export ─────────────────────────────────────────────────────────────
    // Allowed: the homebrew travels. Refused: the book, named as withheld with
    // the same reason as every other route, and none of its contents.
    const exported = await graphql<
      Gql<{
        exportMyData: {
          items: { name: string }[];
          loreEntries: { title: string }[];
          withheld: {
            kind: string;
            id: string;
            title: string;
            reason: string;
          }[];
        };
      }>
    >(
      page,
      `
        query X {
          exportMyData {
            items
            loreEntries
            withheld {
              kind
              id
              title
              reason
            }
          }
        }
      `,
      {},
    );
    expect(exported.errors, JSON.stringify(exported.errors)).toBeUndefined();
    const download = exported.data?.exportMyData;
    expect(download?.items.map((i) => i.name)).toContain(itemName);
    expect(download?.loreEntries.map((l) => l.title)).toContain(loreTitle);
    const withheld = download?.withheld.find((w) => w.id === compendiumId);
    expect(withheld, "the book must be named as withheld").toBeTruthy();
    expect(withheld?.kind).toBe("compendium");
    expect(withheld?.title).toBe(BOOK);
    expectOriginRefusal(withheld?.reason, "exporting a book");
    expect(JSON.stringify(download?.items)).not.toContain(CREATURE);
    expect(JSON.stringify(download?.loreEntries)).not.toContain(CREATURE);

    // ── Adoption into another world ───────────────────────────────────────
    // Allowed: the collection lands, homebrew and all. And nothing of the book
    // arrives with it — not in the copy's reply, not in the world's tables.
    const destination = await aWorld(page, `Somebody's Table ${suffix}`);
    const copied = await graphql<
      Gql<{
        copySharedCollectionToWorld: { created: { id: string }[] };
      }>
    >(
      page,
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
      { shareCode, destinationWorldId: destination },
    );
    expect(copied.errors, JSON.stringify(copied.errors)).toBeUndefined();
    expect(copied.data?.copySharedCollectionToWorld.created).toHaveLength(2);
    expect(copiesInWorld(destination, CREATURE)).toBe(0);
    expect(
      Number(
        sql(
          `SELECT count(*) FROM world_lore_entries
            WHERE world_id = '${uuid(destination)}' AND title = '${loreTitle.replace(/'/g, "''")}';`,
        ),
      ),
      "the authored lore arrived",
    ).toBe(1);
  });
});
