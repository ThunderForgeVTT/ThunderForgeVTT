import { expect, test, type Page } from "@playwright/test";
import {
  graphql,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";

/**
 * specs/026-content-collections, T043 (User Story 3): a takedown reaches one
 * member without disabling the collection.
 *
 * The notice is filed the way a real one is — through `/legal/dmca`, from a
 * logged-out context, which is the intake channel FR-002 of spec 015 requires
 * to be reachable without an account. Going around it through the database
 * would test the collection's reaction to a row rather than to a takedown.
 */

async function fileTakedown(
  claimant: Page,
  contentType: string,
  entityId: string,
  what: string,
): Promise<void> {
  await claimant.goto("/legal/dmca");
  await expect(claimant.getByTestId("takedown-notice-form")).toBeVisible();

  await claimant.getByLabel("Content type").click();
  await claimant.getByRole("option", { name: contentType }).click();
  await claimant.locator("#dmca-entity-id").fill(entityId);
  await claimant.locator("#dmca-claimant-name").fill("Jane Claimant");
  await claimant
    .locator("#dmca-claimant-contact")
    .fill("jane.claimant@example.test");
  await claimant
    .locator("#dmca-work-description")
    .fill("An original work, registered copyright.");
  await claimant.locator("#dmca-infringing-location").fill(what);
  await claimant.locator("#dmca-good-faith").click();
  await claimant.locator("#dmca-accuracy").click();
  await claimant.locator("#dmca-signature").fill("Jane Claimant");

  await claimant.getByTestId("takedown-notice-submit").click();
  await expect(claimant.getByTestId("takedown-notice-accepted")).toBeVisible({
    timeout: 15_000,
  });
}

test.describe("spec 026: a takedown reaches one member, not the collection", () => {
  test("the collection survives, the withheld member is unnamed, and the rest still copy", async ({
    page,
    browser,
  }) => {
    test.setTimeout(420_000);

    const suffix = uniqueSuffix();
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Moderation Source ${suffix}`,
      "e2emodsrc",
    );

    // Two items, so one can be taken down and the other must survive.
    const makeItem = async (name: string): Promise<string> => {
      const created = await graphql<{
        data: { createItem: { id: string } };
      }>(
        page,
        `
          mutation C($input: CreateItemInput!) {
            createItem(input: $input) {
              id
            }
          }
        `,
        { input: { worldId, name } },
      );
      return created.data.createItem.id;
    };

    const doomedName = `Contested Relic ${suffix}`;
    const survivorName = `Ordinary Rope ${suffix}`;
    const doomedId = await makeItem(doomedName);
    const survivorId = await makeItem(survivorName);

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
      { input: { worldId, name: `Moderated Collection ${suffix}` } },
    );
    const collectionId = collection.data.createCollection.id;

    for (const memberId of [doomedId, survivorId]) {
      await graphql(
        page,
        `
          mutation A($input: AddCollectionMemberInput!) {
            addCollectionMember(input: $input) {
              id
            }
          }
        `,
        { input: { collectionId, memberType: "item", memberId } },
      );
    }

    const shared = await graphql<{
      data: { createCollectionShareLink: { shareCode: string } };
    }>(
      page,
      `
        mutation S($collectionId: UUID!) {
          createCollectionShareLink(collectionId: $collectionId) {
            shareCode
          }
        }
      `,
      { collectionId },
    );
    const sharePath = `/collection/${shared.data.createCollectionShareLink.shareCode}`;

    const claimantContext = await browser.newContext();
    const claimant = await claimantContext.newPage();
    const recipientContext = await browser.newContext();
    const recipient = await recipientContext.newPage();

    try {
      // Both members are visible before anything is filed.
      await recipient.goto(sharePath);
      await expect(recipient.getByTestId("collection-members")).toContainText(
        doomedName,
        { timeout: 20_000 },
      );
      await expect(recipient.getByTestId("collection-members")).toContainText(
        survivorName,
      );

      await fileTakedown(
        claimant,
        "Item",
        doomedId,
        `Item "${doomedName}" in a shared ThunderForge collection.`,
      );

      // FR-023: the collection survives. FR-022: the absence is visible and
      // the withheld artifact is never named.
      await recipient.goto(sharePath);
      const body = recipient.locator("body");
      await expect(recipient.getByTestId("collection-members")).toContainText(
        survivorName,
        { timeout: 20_000 },
      );
      await expect(body).not.toContainText(doomedName);
      await expect(body).toContainText(/unavailable and will not be copied/i);

      // FR-021: a copy taken afterwards does not create the withheld member.
      const destinationWorldName = `E2E Moderation Destination ${suffix}`;
      await recipient.getByRole("button", { name: "Copy to a world" }).click();
      await recipient.waitForURL(/\/login\?returnTo=/, { timeout: 15_000 });
      await registerAndCreateWorld(
        recipient,
        destinationWorldName,
        "e2emoddst",
      );

      await recipient.goto(sharePath);
      await recipient.getByRole("button", { name: "Copy to a world" }).click();
      await recipient
        .getByLabel("Destination world")
        .selectOption({ label: destinationWorldName });
      await recipient.getByRole("button", { name: "Confirm copy" }).click();
      await expect(
        recipient.getByRole("heading", { name: "Copied" }),
      ).toBeVisible({ timeout: 60_000 });

      const receipt = recipient.getByTestId("copy-receipt-created");
      await expect(receipt.locator("li")).toHaveCount(1);
      await expect(receipt).toContainText(survivorName);
      await expect(receipt).not.toContainText(doomedName);
      await expect(recipient.getByTestId("copy-receipt-notes")).toContainText(
        /unavailable/i,
      );
      await expect(
        recipient.getByTestId("copy-receipt-notes"),
      ).not.toContainText(doomedName);

      // FR-024: with every member withheld, the collection reports that
      // nothing is available rather than presenting an empty set as complete.
      await fileTakedown(
        claimant,
        "Item",
        survivorId,
        `Item "${survivorName}" in a shared ThunderForge collection.`,
      );
      await recipient.goto(sharePath);
      await expect(recipient.getByTestId("collection-unavailable")).toBeVisible(
        { timeout: 20_000 },
      );
    } finally {
      await claimantContext.close();
      await recipientContext.close();
    }
  });
});
