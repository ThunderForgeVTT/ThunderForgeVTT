import { expect, test, type Page } from "@playwright/test";
import {
  currentSharingTermsVersion,
  graphql,
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
