import { expect, test, type Page } from "@playwright/test";
import { freshCredentials, graphql, register } from "./fixtures/helpers";

/**
 * Spec 049 US2: sending a reviewed book, and what is left behind when it does
 * not arrive (FR-030 to FR-036, SC-007).
 *
 * `book-import-review.spec.ts` proves the half before submit — the book is
 * read on the Game Master's own machine and none of it crosses the wire. This
 * is the half after: what the server does with what arrives, and what the
 * account holds when the import fails partway.
 *
 * # Why the failure is induced at the database and not mocked
 *
 * SC-007 is a claim about a **partial** write, so the failure has to happen
 * after some rows are in. `compendium_entries` carries a CHECK constraint
 * saying a prose entry has no mechanical fields, and the store writes entries
 * in chunks of 500 inside one transaction — so an entry that breaks that
 * constraint, placed past the first chunk, fails with hundreds of rows
 * already written and nothing else touched. A mocked failure would prove the
 * mock rolls back. This proves Postgres does.
 *
 * # What "byte-identical" is checked against
 *
 * A book the account already holds, read back field by field afterwards —
 * including `updatedAt`, which is what a write that half-happened would
 * disturb. Plus the absence of the book that failed, asked by hash against
 * the account's own library.
 */

/** The system the books below are read as. Its pack declares the kinds. */
const SYSTEM = "dnd5e";

type Entry = Record<string, unknown>;

function creature(name: string): Entry {
  return {
    kind: "creature",
    name,
    nameUncertain: false,
    page: 12,
    values: {
      armorClass: { state: "clear", value: "15" },
      hitPoints: { state: "clear", value: "27" },
      // FR-002 over the wire: looked for, not found, and carrying no value.
      challenge: { state: "unread" },
    },
    text: null,
    suspect: false,
    extras: null,
  };
}

/**
 * An entry no reader produces: prose *and* mechanical fields at once.
 *
 * FR-001b says a prose entry has nowhere to put a mechanical value, and the
 * table enforces it. This is the induced failure.
 */
function impossibleEntry(name: string): Entry {
  return {
    ...creature(name),
    kind: "magicItem",
    text: "A cloak that is also, impossibly, a creature.",
  };
}

function aHash(seed: string): string {
  return seed.padStart(64, "0").slice(0, 64);
}

function payload(hash: string, entries: Entry[], title = "A Read Book") {
  return {
    bookTitle: title,
    sourceHash: hash,
    systemId: SYSTEM,
    parserVersion: "e2e",
    pageCount: 300,
    silentPageCount: 4,
    replacesCompendiumId: null,
    entries,
  };
}

const COMPENDIUM_FIELDS = `
  id bookTitle sourceHash systemId origin parserVersion
  pageCount silentPageCount entryTotal importedAt updatedAt
  entryCounts { kind count }
`;

type Compendium = {
  id: string;
  bookTitle: string;
  entryTotal: number;
  updatedAt: string;
  origin: string;
  entryCounts: { kind: string; count: number }[];
};

async function submit(
  page: Page,
  input: ReturnType<typeof payload>,
): Promise<{
  data?: { createCompendiumFromImport: Compendium | null };
  errors?: { message: string }[];
}> {
  return graphql(
    page,
    `mutation Commit($input: CreateCompendiumFromImportInput!) {
       createCompendiumFromImport(input: $input) { ${COMPENDIUM_FIELDS} }
     }`,
    { input },
  );
}

async function bookByHash(page: Page, hash: string): Promise<Compendium | null> {
  const reply = await graphql<{
    data: { compendiumForFileHash: Compendium | null };
  }>(
    page,
    `query Held($sourceHash: String!) {
       compendiumForFileHash(sourceHash: $sourceHash) { ${COMPENDIUM_FIELDS} }
     }`,
    { sourceHash: hash },
  );
  return reply.data.compendiumForFileHash;
}

test.describe("Committing a book (spec 049 US2)", () => {
  test("an import that fails partway leaves the account exactly as it was", async ({
    page,
  }) => {
    test.setTimeout(180_000);
    await register(page, freshCredentials("import"));

    // A book the account already holds, so "unchanged" has something to be
    // measured against rather than being the absence of everything.
    const untouchedHash = aHash("a11");
    const before = await submit(
      page,
      payload(untouchedHash, [creature("Goblin")], "A Book Already Here"),
    );
    const untouched = before.data?.createCompendiumFromImport;
    expect(untouched, JSON.stringify(before.errors)).toBeTruthy();

    // Past the first chunk of 500, so rows are genuinely written before the
    // constraint refuses one.
    const entries = Array.from({ length: 700 }, (_, index) =>
      creature(`Goblin ${index}`),
    );
    entries[600] = impossibleEntry("Cloak of Contradiction");

    const failedHash = aHash("b22");
    const failed = await submit(page, payload(failedHash, entries));

    expect(
      failed.errors?.length ?? 0,
      "an import that cannot be applied must fail, and say so (FR-033)",
    ).toBeGreaterThan(0);
    expect(failed.data?.createCompendiumFromImport ?? null).toBeNull();

    // FR-032, SC-007: not the compendium, not one of the 600 entries that had
    // already been written when the 601st was refused.
    expect(
      await bookByHash(page, failedHash),
      "a failed import must leave no book behind at all",
    ).toBeNull();

    // And the book that was already there is untouched, field for field.
    const after = await bookByHash(page, untouchedHash);
    expect(after).toEqual(untouched);

    // The final proof that nothing partial survived: the same file imports
    // cleanly afterwards. A leftover compendium row would refuse this, since
    // an account holds one book per file.
    const retried = await submit(
      page,
      payload(failedHash, entries.slice(0, 600)),
    );
    expect(
      retried.data?.createCompendiumFromImport?.entryTotal,
      JSON.stringify(retried.errors),
    ).toBe(600);
  });

  test("progress reflects bytes actually sent, and says what is being sent", async ({
    page,
  }) => {
    test.setTimeout(180_000);
    await register(page, freshCredentials("import"));
    await page.goto("/");

    const hash = aHash("c33");
    const observed = await page.evaluate(
      async ([sourceHash]) => {
        const api = (await import(
          /* @vite-ignore */ "/src/api/compendium.ts"
        )) as typeof import("../src/api/compendium");

        const entries = Array.from({ length: 800 }, (_, index) => ({
          kind: "creature",
          name: `Goblin ${index}`,
          nameState: "clear" as const,
          page: 12,
          values: {
            armorClass: { state: "clear" as const, value: "15" },
            challenge: { state: "unread" as const },
          },
          text: null,
          suspect: false,
        }));

        const progress: unknown[] = [];
        const book = await api.submitImport(
          {
            title: "Monster Manual",
            sha256: sourceHash as string,
            pages: 300,
            silentPages: 4,
            systemId: "dnd5e",
            parserVersion: "e2e",
            entries,
          },
          (event) => progress.push(event),
        );
        return { progress, entryTotal: book.entryTotal };
      },
      [hash],
    );

    const progress = observed.progress as (
      | { phase: "sending"; sentBytes: number; totalBytes: number; what: string }
      | { phase: "applying"; totalBytes: number; what: string }
    )[];

    expect(progress.length).toBeGreaterThan(0);

    // FR-030: the numbers are bytes the browser actually handed to the
    // network, so they are bounded by the body it measured and never go
    // backwards.
    const sent = progress.filter((event) => event.phase === "sending");
    for (const event of sent) {
      expect(event.totalBytes).toBeGreaterThan(0);
      expect(event.sentBytes).toBeLessThanOrEqual(event.totalBytes);
    }
    expect(sent.at(-1)?.sentBytes).toBe(sent.at(-1)?.totalBytes);

    // FR-031: it says *what* is going, not only how much.
    expect(progress[0].what).toContain("800 creatures");
    expect(progress[0].what).toContain("Monster Manual");

    // The honest end of the bar: everything is sent and the server is inside
    // one transaction. "Applied" is what the resolved promise says, and it
    // says it after this.
    expect(progress.at(-1)?.phase).toBe("applying");
    expect(observed.entryTotal).toBe(800);
    expect((await bookByHash(page, hash))?.entryTotal).toBe(800);
  });

  test("abandoning an import in flight leaves nothing behind", async ({
    page,
  }) => {
    test.setTimeout(180_000);
    await register(page, freshCredentials("import"));
    await page.goto("/");

    const hash = aHash("d44");

    // The connection is held open by the test, so the abandonment happens
    // with the request genuinely outstanding — which is what a slow upload
    // looks like and what FR-034 is about. Over loopback a payload of any
    // plausible size is gone in one write, so without this the test would be
    // abandoning something that had already arrived, and proving nothing.
    // Only this book's submission is held; every other call goes through.
    await page.route("**/api/graphql", async (route) => {
      if (route.request().postData()?.includes("A Book Nobody Waits For")) {
        await new Promise((resume) => setTimeout(resume, 60_000));
      }
      await route.continue();
    });

    const outcome = await page.evaluate(
      async ([sourceHash]) => {
        const api = (await import(
          /* @vite-ignore */ "/src/api/compendium.ts"
        )) as typeof import("../src/api/compendium");

        const entries = Array.from({ length: 400 }, (_, index) => ({
          kind: "creature",
          name: `Goblin ${index}`,
          nameState: "clear" as const,
          page: 12,
          values: { armorClass: { state: "clear" as const, value: "15" } },
          text: null,
          suspect: false,
        }));

        const abandon = new AbortController();
        const attempt = api.submitImport(
          {
            title: "A Book Nobody Waits For",
            sha256: sourceHash as string,
            pages: 900,
            silentPages: 0,
            systemId: "dnd5e",
            parserVersion: "e2e",
            entries,
          },
          undefined,
          abandon.signal,
        );

        // Long enough that the request is unambiguously outstanding, short
        // enough to be nowhere near the hold above.
        await new Promise((resume) => setTimeout(resume, 500));
        abandon.abort();

        try {
          await attempt;
          return "it completed";
        } catch (error) {
          return error instanceof Error ? error.name : String(error);
        }
      },
      [hash],
    );

    expect(outcome, "an abandoned import stops").toBe("ImportAbandoned");
    await page.unroute("**/api/graphql");

    // FR-034: nothing to clean up, because nothing partial was ever
    // committed — the server never saw a whole payload to commit.
    expect(await bookByHash(page, hash)).toBeNull();
  });

  test("removal reports before it removes, and then removes only that book", async ({
    page,
  }) => {
    test.setTimeout(180_000);
    await register(page, freshCredentials("import"));

    const goingHash = aHash("e55");
    const stayingHash = aHash("f66");
    const going = (
      await submit(
        page,
        payload(goingHash, [creature("Goblin"), creature("Ogre")], "Going"),
      )
    ).data?.createCompendiumFromImport;
    const staying = (
      await submit(page, payload(stayingHash, [creature("Dragon")], "Staying"))
    ).data?.createCompendiumFromImport;
    expect(going && staying).toBeTruthy();

    const remove = async (id: string, confirm: boolean) =>
      graphql<{
        data: {
          removeCompendium: {
            entryCount: number;
            inUse: unknown[];
            handEdited: string[];
            removed: boolean;
          };
        };
      }>(
        page,
        `mutation Remove($id: UUID!, $confirm: Boolean!) {
           removeCompendium(id: $id, confirm: $confirm) {
             entryCount inUse { worldId } handEdited removed
           }
         }`,
        { id, confirm },
      );

    // FR-045: it names what would go, and changes nothing.
    const report = (await remove(going!.id, false)).data.removeCompendium;
    expect(report.entryCount).toBe(2);
    expect(report.removed).toBe(false);
    expect(await bookByHash(page, goingHash)).not.toBeNull();

    // FR-044: confirming takes that import's contribution, and nothing else.
    const confirmed = (await remove(going!.id, true)).data.removeCompendium;
    expect(confirmed.removed).toBe(true);
    expect(await bookByHash(page, goingHash)).toBeNull();
    expect((await bookByHash(page, stayingHash))?.entryTotal).toBe(1);
  });
});
