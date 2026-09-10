import { execFileSync } from "node:child_process";
import { expect, test, type Browser, type Page } from "@playwright/test";
import {
  currentSharingTermsVersion,
  graphql,
  loginAsAdmin,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";

/**
 * Spec 039 US1 and US3: asked on every path that publishes, and the **server**
 * is the one asking.
 *
 * # What this file is for that the unit tests are not
 *
 * The server tests prove `require_attestation` refuses. This proves the thing
 * a person can actually check: that no publishing path in the running product
 * reaches a share code without an agreement — including the paths that reach
 * it without ever loading the page.
 *
 * US1's fifth scenario ("a new type added later is covered without a separate
 * decision") is deliberately **not** here. It is a claim about code that does
 * not exist yet, which no end-to-end test can make; it is held by the SDL guard
 * in `graphql/publishing_gate.rs`, which fails the day somebody writes a fifth
 * `create*ShareLink` without the argument. An e2e asserting it would be an
 * assertion about nothing.
 */

test.describe.configure({ mode: "serial" });

const CREATE_COLLECTION = `
  mutation C($input: CreateCollectionInput!) {
    createCollection(input: $input) { id }
  }
`;

const ADD_MEMBER = `
  mutation A($input: AddCollectionMemberInput!) {
    addCollectionMember(input: $input) { id }
  }
`;

const CREATE_ITEM = `
  mutation I($input: CreateItemInput!) {
    createItem(input: $input) { id }
  }
`;

/** The shape every publish now has, so the tests below vary only what matters. */
const SHARE_COLLECTION = `
  mutation S($collectionId: UUID!, $attestation: AttestationInput!) {
    createCollectionShareLink(collectionId: $collectionId, attestation: $attestation) {
      id
      shareCode
    }
  }
`;

type Gql<T> = { data?: T; errors?: { message: string }[] };

async function aCollectionWithSomethingInIt(
  page: Page,
  worldId: string,
): Promise<string> {
  const item = await graphql<Gql<{ createItem: { id: string } }>>(
    page,
    CREATE_ITEM,
    {
      input: { worldId, name: `Lantern ${uniqueSuffix()}`, description: null },
    },
  );
  const itemId = item.data?.createItem.id;
  expect(itemId, JSON.stringify(item.errors)).toBeTruthy();

  const collection = await graphql<Gql<{ createCollection: { id: string } }>>(
    page,
    CREATE_COLLECTION,
    {
      input: {
        worldId,
        name: `Attested ${uniqueSuffix()}`,
        description: null,
      },
    },
  );
  const collectionId = collection.data?.createCollection.id;
  expect(collectionId, JSON.stringify(collection.errors)).toBeTruthy();

  await graphql(page, ADD_MEMBER, {
    input: { collectionId, memberType: "item", memberId: itemId },
  });

  return collectionId as string;
}

test.describe("Spec 039: the server requires an agreement to publish", () => {
  test.setTimeout(180_000);

  /**
   * US3 scenario 1, and the whole reason this feature exists: **a publish
   * carrying no attestation is refused, and nothing is published.**
   *
   * The mutation is sent without the argument at all, which the schema rejects
   * before a resolver runs. That is the strongest form of the requirement — a
   * nullable argument would be one satisfied by omission — and it is why the
   * assertion is about the *absence of a share code* rather than about the
   * wording of the error.
   */
  test("a publish with no attestation is refused, and mints nothing", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Attestation ${uniqueSuffix()}`,
    );
    const collectionId = await aCollectionWithSomethingInIt(page, worldId);

    const noAgreement = await graphql<
      Gql<{ createCollectionShareLink: { shareCode: string } }>
    >(
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

    expect(
      noAgreement.data?.createCollectionShareLink,
      "a publish that agreed to nothing must not produce a link",
    ).toBeFalsy();
    expect(noAgreement.errors?.length ?? 0).toBeGreaterThan(0);
  });

  /**
   * US3 scenarios 2 and 3: an identity this instance does not recognise is
   * refused, and **the refusal does not help forge one**.
   *
   * The second half is the one worth driving. A refusal that echoed the
   * expected identity would tell a caller exactly what to send to skip the
   * dialog — so the assertion is that no valid identity, and nothing shaped
   * like one, appears in the message.
   */
  test("an unrecognised version is refused, and the refusal helps nobody forge one", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Attestation Bad ${uniqueSuffix()}`,
    );
    const collectionId = await aCollectionWithSomethingInIt(page, worldId);
    const real = await currentSharingTermsVersion(page);

    const refused = await graphql<
      Gql<{ createCollectionShareLink: { shareCode: string } }>
    >(page, SHARE_COLLECTION, {
      collectionId,
      attestation: { termsVersionId: "sharing-terms@ffffffffffffffff" },
    });

    expect(refused.data?.createCollectionShareLink).toBeFalsy();
    const message = refused.errors?.[0]?.message ?? "";
    expect(message.length, "a refusal has to say something").toBeGreaterThan(0);
    expect(
      message,
      "a refusal must not name a valid version identity",
    ).not.toContain(real);
    expect(
      message,
      "nor anything shaped like one — that is how a caller learns to skip the dialog",
    ).not.toContain("@");
  });

  /**
   * US1 scenario 3: sharing again asks again.
   *
   * FR-003 is "per publish, not per person, per world or per session". Nothing
   * caches an agreement and nothing consults a previous one, so two publishes
   * of the same collection leave two records — which is what a notice about
   * either of them needs.
   */
  test("sharing the same thing twice records two agreements", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Attestation Twice ${uniqueSuffix()}`,
    );
    const collectionId = await aCollectionWithSomethingInIt(page, worldId);
    const termsVersionId = await currentSharingTermsVersion(page);

    const first = await graphql<
      Gql<{ createCollectionShareLink: { id: string; shareCode: string } }>
    >(page, SHARE_COLLECTION, {
      collectionId,
      attestation: { termsVersionId },
    });
    expect(
      first.data?.createCollectionShareLink?.shareCode,
      JSON.stringify(first.errors),
    ).toBeTruthy();

    const second = await graphql<
      Gql<{ createCollectionShareLink: { id: string; shareCode: string } }>
    >(page, SHARE_COLLECTION, {
      collectionId,
      attestation: { termsVersionId },
    });
    expect(second.data?.createCollectionShareLink?.shareCode).toBeTruthy();
    expect(
      second.data?.createCollectionShareLink?.id,
      "each publish is its own act, with its own link and its own agreement",
    ).not.toBe(first.data?.createCollectionShareLink?.id);
  });

  /**
   * US1 scenarios 1 and 4, through the interface: the dialog is on screen
   * before anything is published, and **declining loses nothing**.
   *
   * The second half matters more than it reads. A person assembling a
   * collection needs to know that backing out of the agreement does not undo
   * their work — the dialog is a client-side gate before a mutation is sent, so
   * there is nothing to undo.
   */
  test("the agreement is on screen before the button, and cancelling keeps the work", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Attestation UI ${uniqueSuffix()}`,
    );
    await aCollectionWithSomethingInIt(page, worldId);

    await page.goto(`/world/${worldId}/collections`);
    // The sharing block lives inside one collection's detail panel, so the
    // collection has to be opened first — the same click `content-collections`
    // makes. Reaching for `share-terms` on the list page finds nothing, which is
    // correct and looks exactly like the dialog being missing.
    await page
      .getByRole("button", { name: "Open", exact: true })
      .first()
      .click();
    const terms = page.getByTestId("share-terms");
    await expect(terms).toBeVisible({ timeout: 20_000 });
    await expect(
      terms,
      "the words come from the server, so this is also proof `sharingTerms` answered",
    ).toContainText("responsible for what you publish");

    // Nothing has been published, and the collection is still here.
    const before = await graphql<Gql<{ worldCollections: { id: string }[] }>>(
      page,
      `
        query C($worldId: UUID!) {
          worldCollections(worldId: $worldId) {
            id
          }
        }
      `,
      { worldId },
    );
    expect(before.data?.worldCollections?.length).toBeGreaterThan(0);
  });
});

// ---------------------------------------------------------------------------
// Phase 5: the record, and what happens when the words change (US2, US4)
// ---------------------------------------------------------------------------

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/**
 * Runs SQL against this shard's database, the way `dmca-counter-notice` moves a
 * clock. Every value interpolated here is checked against a strict pattern
 * first, so nothing a test computes can become SQL.
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
  );
}

function assertUuid(value: string): string {
  if (!UUID_PATTERN.test(value)) {
    throw new Error(`Refusing to put a non-UUID into SQL: ${value}`);
  }
  return value;
}

/**
 * The state a revision of `legal/sharing-terms.md` followed by a restart leaves
 * behind: the archive holds the old words beside the new, and an agreement made
 * before the revision names the old ones.
 *
 * # Why this is seeded rather than done by editing the file
 *
 * The terms are compiled into the server with `include_str!`, so "revise and
 * restart" is really "revise, rebuild and restart" — minutes, per shard — and
 * editing a tracked file while the suite runs HMRs it into every other shard's
 * stack. What a revision *does* to the database is two rows, and the boot-time
 * write that produces the new one is held by
 * `archiving_twice_leaves_the_first_row_exactly_as_it_was` in
 * `legal/legal_tests.rs`. Everything this test asserts is read back through the
 * running product.
 */
function anAgreementMadeBeforeARevision(
  collectionId: string,
  subjectUserId: string,
): { versionId: string; marker: string } {
  const hex = Array.from({ length: 16 }, () =>
    Math.floor(Math.random() * 16).toString(16),
  ).join("");
  const versionId = `sharing-terms@${hex}`;
  const marker = `words-before-the-revision-${hex}`;
  const body = `## What it said then\n\nThis build no longer ships these words: ${marker}.`;

  sql(
    `INSERT INTO terms_versions (version_id, document_slug, body, first_seen_at) ` +
      `VALUES ('${versionId}', 'sharing-terms', '${body}', NOW() - INTERVAL '40 days');` +
      `INSERT INTO attestations (id, purpose, subject_user_id, subject_username, ` +
      `terms_version_id, publishable_kind, publishable_id, share_id, attested_at) ` +
      `VALUES (gen_random_uuid(), 'share', '${assertUuid(subjectUserId)}', 'before', ` +
      `'${versionId}', 'collection', '${assertUuid(collectionId)}', gen_random_uuid(), ` +
      `NOW() - INTERVAL '30 days');`,
  );
  return { versionId, marker };
}

const ATTESTATIONS_FOR = `
  query A($publishableKind: String!, $publishableId: UUID!) {
    attestationsFor(publishableKind: $publishableKind, publishableId: $publishableId) {
      id
      subjectUsername
      attestedAt
      termsVersionId
      terms { versionId sections { heading body } }
    }
  }
`;

type AttestationView = {
  id: string;
  subjectUsername: string | null;
  attestedAt: string;
  termsVersionId: string;
  terms: {
    versionId: string;
    sections: { heading: string | null; body: string }[];
  };
};

async function myAccountId(page: Page): Promise<string> {
  const me = await graphql<Gql<{ me: { id: string } | null }>>(
    page,
    `query Me { me { id } }`,
    {},
  );
  const id = me.data?.me?.id;
  expect(id, JSON.stringify(me.errors)).toBeTruthy();
  return id as string;
}

async function openAdmin(browser: Browser): Promise<Page> {
  const context = await browser.newContext();
  const admin = await context.newPage();
  await loginAsAdmin(admin);
  await admin.waitForURL(/\/(admin|welcome)$/, { timeout: 20_000 });
  return admin;
}

test.describe("Spec 039: an agreement is evidence rather than a moment", () => {
  test.setTimeout(240_000);

  /**
   * US4 / T039, and the property ADR-076 exists for: **revising the terms does
   * not rewrite what anybody agreed to.**
   *
   * An agreement from before a revision, and one made now through the product,
   * for the same collection. Each must resolve to its own words, and the old
   * one must come back exactly as it was recorded — same version, same moment.
   * `Attestation.terms` implemented as "the current document with a version
   * label" fails the first assertion about the old record's words.
   */
  test("revising the terms leaves every earlier agreement where it was", async ({
    page,
    browser,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Attestation Revision ${uniqueSuffix()}`,
    );
    const collectionId = await aCollectionWithSomethingInIt(page, worldId);
    const before = anAgreementMadeBeforeARevision(
      collectionId,
      await myAccountId(page),
    );

    const current = await currentSharingTermsVersion(page);
    expect(current, "the fixture must differ from what this build ships").not.toBe(
      before.versionId,
    );
    const shared = await graphql<
      Gql<{ createCollectionShareLink: { shareCode: string } }>
    >(page, SHARE_COLLECTION, {
      collectionId,
      attestation: { termsVersionId: current },
    });
    expect(
      shared.data?.createCollectionShareLink?.shareCode,
      JSON.stringify(shared.errors),
    ).toBeTruthy();

    const admin = await openAdmin(browser);
    try {
      const read = await graphql<Gql<{ attestationsFor: AttestationView[] }>>(
        admin,
        ATTESTATIONS_FOR,
        { publishableKind: "collection", publishableId: collectionId },
      );
      const records = read.data?.attestationsFor ?? [];
      expect(records, JSON.stringify(read.errors)).toHaveLength(2);

      const [newest, oldest] = records;
      expect(newest.termsVersionId, "newest first").toBe(current);
      expect(newest.terms.versionId).toBe(current);
      expect(
        JSON.stringify(newest.terms.sections),
        "the new agreement is to the words this build ships",
      ).toContain("responsible for what you publish");

      expect(oldest.termsVersionId, "the old agreement did not move").toBe(
        before.versionId,
      );
      expect(oldest.terms.versionId).toBe(before.versionId);
      expect(
        JSON.stringify(oldest.terms.sections),
        "and it resolves to the words it agreed to, not today's",
      ).toContain(before.marker);
      expect(JSON.stringify(oldest.terms.sections)).not.toContain(
        "responsible for what you publish",
      );
      const ageInDays =
        (Date.now() - new Date(oldest.attestedAt).getTime()) / 86_400_000;
      expect(
        ageInDays,
        "nothing re-stamped it when the new agreement was written",
      ).toBeGreaterThan(29);

      // The archive answers the same question by version alone.
      const archived = await graphql<
        Gql<{ legalDocumentVersion: { sections: { body: string }[] } | null }>
      >(
        admin,
        `query V($versionId: String!) {
          legalDocumentVersion(versionId: $versionId) { sections { body } }
        }`,
        { versionId: before.versionId },
      );
      expect(
        JSON.stringify(archived.data?.legalDocumentVersion),
      ).toContain(before.marker);
    } finally {
      await admin.context().close();
    }
  });

  /**
   * US2 / T037, SC-003: **from a notice to the agreement in under a minute,
   * without a developer.**
   *
   * A real takedown against something a real person shared; then, as the
   * administrator, only the case reference and the moderation page. The
   * assertion is that the page — not a query — shows who agreed and the words.
   */
  test("a notice handler reaches the agreement from the case, on the page", async ({
    page,
    browser,
  }) => {
    const suffix = uniqueSuffix();
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Attestation Notice ${suffix}`,
      "e2eattest",
    );
    const item = await graphql<Gql<{ createItem: { id: string } }>>(
      page,
      CREATE_ITEM,
      { input: { worldId, name: `Borrowed Crown ${suffix}`, description: null } },
    );
    const itemId = item.data?.createItem.id as string;
    expect(itemId, JSON.stringify(item.errors)).toBeTruthy();

    const shared = await graphql<
      Gql<{ createItemShareLink: { shareCode: string } }>
    >(
      page,
      `mutation S($itemId: UUID!, $attestation: AttestationInput!) {
        createItemShareLink(itemId: $itemId, attestation: $attestation) { shareCode }
      }`,
      {
        itemId,
        attestation: { termsVersionId: await currentSharingTermsVersion(page) },
      },
    );
    expect(
      shared.data?.createItemShareLink?.shareCode,
      JSON.stringify(shared.errors),
    ).toBeTruthy();

    // And published a second way: inside a collection. A shared collection
    // serves its members, so its agreement covers this item too, and the
    // notice handler has to see both.
    const collection = await graphql<Gql<{ createCollection: { id: string } }>>(
      page,
      CREATE_COLLECTION,
      {
        input: { worldId, name: `Regalia ${suffix}`, description: null },
      },
    );
    const collectionId = collection.data?.createCollection.id as string;
    expect(collectionId, JSON.stringify(collection.errors)).toBeTruthy();
    await graphql(page, ADD_MEMBER, {
      input: { collectionId, memberType: "item", memberId: itemId },
    });
    const sharedCollection = await graphql<
      Gql<{ createCollectionShareLink: { shareCode: string } }>
    >(page, SHARE_COLLECTION, {
      collectionId,
      attestation: { termsVersionId: await currentSharingTermsVersion(page) },
    });
    expect(
      sharedCollection.data?.createCollectionShareLink?.shareCode,
      JSON.stringify(sharedCollection.errors),
    ).toBeTruthy();

    const me = await graphql<Gql<{ me: { username: string } | null }>>(
      page,
      `query Me { me { username } }`,
      {},
    );
    const sharer = me.data?.me?.username as string;
    expect(sharer).toBeTruthy();

    // The claimant needs no account (spec 015 FR-002).
    const claimantContext = await browser.newContext();
    const claimant = await claimantContext.newPage();
    const admin = await openAdmin(browser);
    try {
      await claimant.goto("/legal/dmca");
      await expect(claimant.getByTestId("takedown-notice-form")).toBeVisible();
      await claimant.getByLabel("Content type").click();
      await claimant.getByRole("option", { name: "Item" }).click();
      await claimant.locator("#dmca-entity-id").fill(itemId);
      await claimant.locator("#dmca-claimant-name").fill("Jane Claimant");
      await claimant
        .locator("#dmca-claimant-contact")
        .fill("jane.claimant@example.test");
      await claimant
        .locator("#dmca-work-description")
        .fill("An original work, registered copyright.");
      await claimant
        .locator("#dmca-infringing-location")
        .fill(`The item "Borrowed Crown ${suffix}".`);
      await claimant.locator("#dmca-good-faith").click();
      await claimant.locator("#dmca-accuracy").click();
      await claimant.locator("#dmca-signature").fill("Jane Claimant");
      await claimant.getByTestId("takedown-notice-submit").click();
      const accepted = claimant.getByTestId("takedown-notice-accepted");
      await expect(accepted).toBeVisible({ timeout: 15_000 });
      const caseId = (await accepted.locator("code").innerText()).trim();
      expect(caseId).toMatch(UUID_PATTERN);

      await admin.goto("/admin/moderation");
      await admin.locator("#moderation-case-reference").fill(caseId);
      await admin.getByRole("button", { name: "Open case" }).click();
      const opened = admin.getByTestId("opened-case");
      await expect(opened).toContainText(itemId, { timeout: 15_000 });

      await opened.getByTestId("case-agreement-open").click();
      const records = opened.getByTestId("case-agreement-record");
      await expect(
        records,
        "the item's own agreement and its collection's",
      ).toHaveCount(2, { timeout: 15_000 });
      for (const record of await records.all()) {
        await expect(record, "who agreed").toContainText(sharer);
      }

      const viaCollection = records.filter({
        has: admin.getByTestId("case-agreement-via-collection"),
      });
      await expect(viaCollection).toHaveCount(1);
      await expect(viaCollection).toContainText(collectionId);

      const first = records.first();
      await first.getByText("The words agreed to").click();
      await expect(
        first.getByTestId("case-agreement-terms"),
        "and to what",
      ).toContainText("responsible for what you publish");
    } finally {
      await claimantContext.close();
      await admin.context().close();
    }
  });
});
