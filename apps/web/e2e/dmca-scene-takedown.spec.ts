import { execFileSync } from "node:child_process";
import { expect, test } from "./fixtures/test";
import {
  currentSharingTermsVersion,
  graphql,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";
import { fileSceneTakedown } from "./fixtures/playPause";

/**
 * specs/015-dmca-notice-takedown T042: a scene is a moderated entity type.
 *
 * Before T042 the takedown form could not name a scene at all, so a scene in
 * a shared collection stayed on the signed-out link whatever a claimant did.
 * ADR-098 held spec 050's sync-back on this. The whole statutory loop is
 * driven here the way people drive it:
 *
 *  - the notice is filed through `/legal/dmca`, logged out, choosing "Scene";
 *  - the signed-out collection link withholds the scene without naming it,
 *    and keeps the collection's other member;
 *  - the owner files the counter-notice from their standing page — a scene
 *    has no detail page to carry the moderation banner, so that is its only
 *    route back, and T042 is what put the form there for them;
 *  - the scene stays withheld while forwarded, and returns to the same link
 *    once the waiting period has passed, with nothing rebuilt.
 *
 * The waiting period is simulated exactly as `dmca-counter-notice.spec.ts`
 * does and for the reasons its header gives: one `UPDATE` moving one
 * already-recorded timestamp into the past. Nothing else is written to the
 * database.
 */

const CASE_ID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

function elapseWaitingPeriod(caseId: string): void {
  if (!CASE_ID_PATTERN.test(caseId)) {
    throw new Error(`Refusing to run SQL for a non-UUID case id: ${caseId}`);
  }
  const container =
    process.env.THUNDERFORGE_POSTGRES_CONTAINER ?? "thunderforge-postgres";
  const database = process.env.THUNDERFORGE_DB_NAME ?? "thunderforge";
  const dbUser = process.env.THUNDERFORGE_DB_USER ?? "postgres";

  const output = execFileSync(
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
    {
      input:
        "UPDATE content_moderation_actions " +
        "SET restoration_due_at = NOW() - INTERVAL '1 day' " +
        `WHERE case_id = '${caseId}' AND action_type = 'counter_notice_forwarded' ` +
        "RETURNING id;",
      encoding: "utf-8",
      stdio: ["pipe", "pipe", "inherit"],
    },
  );
  const updated = output
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => CASE_ID_PATTERN.test(line));
  if (updated.length !== 1) {
    throw new Error(
      `Expected exactly one forwarded counter-notice for case ${caseId}, ` +
        `psql reported: ${JSON.stringify(output)}`,
    );
  }
}

test.describe("spec 015 T042: a takedown reaches a scene in a shared collection", () => {
  test("the scene leaves the signed-out link on a notice and returns after a counter-notice", async ({
    page,
    browser,
  }) => {
    test.setTimeout(300_000);

    const suffix = uniqueSuffix();
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Scene Takedown ${suffix}`,
      "e2escenedmca",
    );

    const sceneName = `Contested Keep ${suffix}`;
    const itemName = `Ordinary Lantern ${suffix}`;

    const scene = await graphql<{
      data: { createScene: { sceneId: string } };
    }>(
      page,
      `
        mutation S($input: GraphQLCreateSceneInput!) {
          createScene(input: $input) {
            sceneId
          }
        }
      `,
      { input: { worldId, name: sceneName } },
    );
    const sceneId = scene.data.createScene.sceneId;

    const item = await graphql<{ data: { createItem: { id: string } } }>(
      page,
      `
        mutation I($input: CreateItemInput!) {
          createItem(input: $input) {
            id
          }
        }
      `,
      { input: { worldId, name: itemName } },
    );
    const itemId = item.data.createItem.id;

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
      { input: { worldId, name: `Scene Takedown Collection ${suffix}` } },
    );
    const collectionId = collection.data.createCollection.id;

    for (const [memberType, memberId] of [
      ["scene", sceneId],
      ["item", itemId],
    ] as const) {
      await graphql(
        page,
        `
          mutation A($input: AddCollectionMemberInput!) {
            addCollectionMember(input: $input) {
              id
            }
          }
        `,
        { input: { collectionId, memberType, memberId } },
      );
    }

    const shared = await graphql<{
      data: { createCollectionShareLink: { shareCode: string } };
    }>(
      page,
      `
        mutation L($collectionId: UUID!, $attestation: AttestationInput!) {
          createCollectionShareLink(
            collectionId: $collectionId
            attestation: $attestation
          ) {
            shareCode
          }
        }
      `,
      {
        collectionId,
        attestation: { termsVersionId: await currentSharingTermsVersion(page) },
      },
    );
    const sharePath = `/collection/${shared.data.createCollectionShareLink.shareCode}`;

    // Two distinct logged-out contexts: the claimant, and a stranger holding
    // the link. Neither is the owner's session.
    const claimantContext = await browser.newContext();
    const claimant = await claimantContext.newPage();
    const visitorContext = await browser.newContext();
    const visitor = await visitorContext.newPage();

    try {
      const members = visitor.getByTestId("collection-members");
      await visitor.goto(sharePath);
      await expect(members).toContainText(sceneName, { timeout: 20_000 });
      await expect(members).toContainText(itemName);

      const caseId = await fileSceneTakedown(claimant, sceneId, sceneName);

      // FR-021/FR-022/FR-023: withheld, unnamed, and the collection survives.
      await visitor.goto(sharePath);
      await expect(members).toContainText(itemName, { timeout: 20_000 });
      await expect(visitor.locator("body")).not.toContainText(sceneName);
      await expect(visitor.locator("body")).toContainText(
        /unavailable and will not be copied/i,
      );

      // The owner's route back. The strike names the kind, and the form is
      // offered in place because a scene has no page of its own.
      await page.goto("/settings/standing");
      const strike = page
        .getByTestId("standing-strike")
        .filter({ hasText: caseId });
      await expect(strike).toContainText("A scene", { timeout: 20_000 });
      await strike
        .getByRole("button", { name: "File a counter-notice" })
        .click();
      await strike
        .locator("#counter-notice-material")
        .fill(`My own scene "${sceneName}", drawn by hand.`);
      await strike
        .locator("#counter-notice-contact")
        .fill("owner@example.test");
      await strike.locator("#counter-notice-good-faith").click();
      await strike.locator("#counter-notice-jurisdiction").click();
      await strike.locator("#counter-notice-signature").fill("The Owner");
      await strike.getByTestId("counter-notice-submit").click();
      await expect(page.getByTestId("standing-counter-filed")).toContainText(
        caseId,
        { timeout: 15_000 },
      );

      // § 512(g): forwarded is not restored. Still withheld from the link.
      await visitor.goto(sharePath);
      await expect(members).toContainText(itemName, { timeout: 20_000 });
      await expect(visitor.locator("body")).not.toContainText(sceneName);

      elapseWaitingPeriod(caseId);

      // FR-025: back on the same link, nothing rebuilt.
      await visitor.goto(sharePath);
      await expect(members).toContainText(sceneName, { timeout: 20_000 });
      await expect(members).toContainText(itemName);
      await expect(visitor.locator("body")).not.toContainText(
        /unavailable and will not be copied/i,
      );
    } finally {
      await claimantContext.close();
      await visitorContext.close();
    }
  });
});
