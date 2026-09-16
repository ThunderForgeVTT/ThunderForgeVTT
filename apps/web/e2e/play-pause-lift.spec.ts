import { expect, test, type Locator, type Page } from "./fixtures/test";
import { openAdminPage } from "./fixtures/admin";
import { expectNoAxeViolations } from "./fixtures/axe";
import {
  graphql,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";
import {
  elapseWaitingPeriod,
  expectPausedNotice,
  fileCounterNoticeViaApi,
  fileSceneTakedown,
  markDocument,
  pauseWorldAsOperator,
  type GqlAnswer,
} from "./fixtures/playPause";

/**
 * Spec 051 User Story 4 (T050, quickstart scenario 4): an operator lifts a
 * pause, and play comes back with nothing else changed.
 *
 * Proven here, in a real browser against the real stack, on one world:
 *
 *  1. a scene of the world is taken down through `/legal/dmca`, the world is
 *     paused, and the world's own Owner cannot lift it (FR-040);
 *  2. an operator lifts it from `/admin/play-pauses`, through a dialog that
 *     will not take blank grounds;
 *  3. the table, still on the notice, is offered "Return to the world" within
 *     30 s without reloading, and play starts again (FR-041);
 *  4. the taken-down scene is still withheld: lifting a pause restores play,
 *     not content (FR-042);
 *  5. the operator's *Record* shows the pause with who lifted it, when, and
 *     on what grounds (FR-051);
 *  6. paused again, the takedown is reversed by counter-notice and the
 *     waiting period runs out: the content comes back, and the pause stays in
 *     force with the table still on the notice (FR-042).
 *
 * The pause itself is made through GraphQL (`play-pause.spec.ts` is the
 * pause's portal proof); the lift, which this file is about, is made in the
 * portal. GraphQL is otherwise only used to set up and to **check**.
 */

async function createScene(
  page: Page,
  worldId: string,
  name: string,
): Promise<string> {
  const created = await graphql<
    GqlAnswer<{ createScene: { sceneId: string } }>
  >(
    page,
    `
      mutation CreateScene($input: GraphQLCreateSceneInput!) {
        createScene(input: $input) {
          sceneId
        }
      }
    `,
    { input: { worldId, name } },
  );
  const sceneId = created.data?.createScene?.sceneId;
  if (!sceneId) {
    throw new Error(`createScene failed: ${JSON.stringify(created.errors)}`);
  }
  return sceneId;
}

/** The scene ids a member of the world is shown. */
async function sceneIdsShownTo(
  member: Page,
  worldId: string,
): Promise<string[]> {
  const answer = await graphql<GqlAnswer<{ scenes: { sceneId: string }[] }>>(
    member,
    `
      query Scenes($worldId: UUID!) {
        scenes(worldId: $worldId) {
          sceneId
        }
      }
    `,
    { worldId },
  );
  if (!answer.data?.scenes) {
    throw new Error(`scenes did not answer: ${JSON.stringify(answer.errors)}`);
  }
  return answer.data.scenes.map((scene) => scene.sceneId);
}

/**
 * Spec 015's own enforcement primitive for a scene. Asking it is also what
 * materialises a restoration whose waiting period has run out.
 */
async function sceneModerationStatus(
  page: Page,
  sceneId: string,
): Promise<string | null> {
  const answer = await graphql<GqlAnswer<{ moderationStatus: string | null }>>(
    page,
    `
      query Status($entityId: UUID!) {
        moderationStatus(entityType: SCENE, entityId: $entityId)
      }
    `,
    { entityId: sceneId },
  );
  if (!answer.data) {
    throw new Error(
      `moderationStatus did not answer: ${JSON.stringify(answer.errors)}`,
    );
  }
  return answer.data.moderationStatus;
}

async function activePauseIds(
  adminPage: Page,
  worldId: string,
): Promise<string[]> {
  const answer = await graphql<
    GqlAnswer<{ playPauses: { nodes: { id: string }[] } }>
  >(
    adminPage,
    `
      query Active($worldId: UUID) {
        playPauses(active: true, worldId: $worldId, first: 10) {
          nodes {
            id
          }
        }
      }
    `,
    { worldId },
  );
  if (!answer.data?.playPauses) {
    throw new Error(`playPauses did not answer: ${JSON.stringify(answer)}`);
  }
  return answer.data.playPauses.nodes.map((pause) => pause.id);
}

async function worldPlayPaused(
  member: Page,
  worldId: string,
): Promise<boolean> {
  const answer = await graphql<
    GqlAnswer<{ worldPlayState: { paused: boolean } }>
  >(
    member,
    `
      query State($worldId: UUID!) {
        worldPlayState(worldId: $worldId) {
          paused
        }
      }
    `,
    { worldId },
  );
  if (!answer.data?.worldPlayState) {
    throw new Error(`worldPlayState did not answer: ${JSON.stringify(answer)}`);
  }
  return answer.data.worldPlayState.paused;
}

async function enterPlay(page: Page, worldId: string): Promise<void> {
  await page.goto(`/world/${worldId}/play`);
  await expect(page.locator("canvas")).toBeVisible({ timeout: 60_000 });
}

function activeRow(adminPage: Page, worldId: string): Locator {
  return adminPage.locator(
    `[data-testid="play-pause-active"][data-world-id="${worldId}"]`,
  );
}

test.describe("spec 051 US4: lifting a pause", () => {
  test("an operator lifts a pause in the portal: play returns, the taken-down scene stays withheld, and a restoration never lifts a pause", async ({
    browser,
  }) => {
    test.setTimeout(420_000);
    const gmContext = await browser.newContext();
    const gmPage = await gmContext.newPage();
    const claimantContext = await browser.newContext();
    const claimant = await claimantContext.newPage();
    const adminPage = await openAdminPage(browser);

    try {
      const worldName = `E2E Lifted Table ${uniqueSuffix()}`;
      const worldId = await registerAndCreateWorld(
        gmPage,
        worldName,
        "e2elift",
      );
      const sceneName = `Withheld Vault ${uniqueSuffix()}`;
      const sceneId = await createScene(gmPage, worldId, sceneName);

      // 1. The scene is taken down before anyone plays, so the notice raises
      //    no request and the pause below is the only one in the story.
      const caseId = await fileSceneTakedown(claimant, sceneId, sceneName);
      expect(await sceneModerationStatus(gmPage, sceneId)).toBe(
        "CONTENT_DISABLED",
      );
      expect(await sceneIdsShownTo(gmPage, worldId)).not.toContain(sceneId);

      await enterPlay(gmPage, worldId);
      await markDocument(gmPage);
      const grounds = `Lift e2e ${uniqueSuffix()}: holding while we look`;
      const { pause } = await pauseWorldAsOperator(adminPage, worldId, grounds);
      await gmPage.waitForURL(/\/world\/[^/]+\/paused$/, { timeout: 15_000 });
      await expectPausedNotice(gmPage, worldName, { grounds, marked: true });

      // FR-040: the world's own Owner may not lift it.
      const byOwner = await graphql<
        GqlAnswer<{ liftWorldPlayPause: { id: string } }>
      >(
        gmPage,
        `
          mutation Lift($pauseId: UUID!, $grounds: String!) {
            liftWorldPlayPause(pauseId: $pauseId, grounds: $grounds) {
              id
            }
          }
        `,
        { pauseId: pause.id, grounds: "It is my world." },
      );
      expect(byOwner.data?.liftWorldPlayPause ?? null).toBeNull();
      expect(byOwner.errors?.[0]?.message).toBe("Admin privileges required");
      expect(await activePauseIds(adminPage, worldId)).toEqual([pause.id]);

      // 2. The operator lifts it in the portal.
      await adminPage.goto("/admin/play-pauses");
      await expect(
        adminPage.getByRole("heading", { name: "Play pauses", level: 1 }),
      ).toBeVisible({ timeout: 20_000 });
      const row = activeRow(adminPage, worldId);
      await expect(row).toContainText(grounds, { timeout: 20_000 });
      const open = row.getByRole("button", {
        name: `Lift the pause on ${worldName}`,
      });
      await open.click();
      const dialog = adminPage.getByTestId("play-pause-lift");
      await expect(dialog).toBeVisible();
      await expect(dialog.getByRole("heading")).toHaveText(
        `Lift the pause on ${worldName}?`,
      );
      const submit = dialog.getByTestId("play-pause-lift-submit");
      await expect(submit).toBeDisabled();
      await dialog.getByLabel("Grounds").fill("   ");
      await expect(submit).toBeDisabled();
      await expectNoAxeViolations(adminPage, '[data-testid="play-pause-lift"]');

      // Cancelling changes nothing and gives focus back to the button.
      await adminPage.keyboard.press("Escape");
      await expect(dialog).toBeHidden();
      await expect(open).toBeFocused();
      expect(await activePauseIds(adminPage, worldId)).toEqual([pause.id]);

      await open.click();
      await expect(dialog).toBeVisible();
      const liftGrounds = `Lifted ${uniqueSuffix()}: the question is settled`;
      await dialog.getByLabel("Grounds").fill(liftGrounds);
      await expect(submit).toBeEnabled();
      await submit.click();
      await expect(dialog).toBeHidden({ timeout: 15_000 });
      const liftedAtMs = Date.now();
      const outcome = adminPage.getByTestId("play-pause-outcome");
      await expect(outcome).toHaveText(
        `Play in ${worldName} is no longer paused.`,
      );
      await expect(outcome).toBeFocused();
      await expect(activeRow(adminPage, worldId)).toHaveCount(0);
      expect(await activePauseIds(adminPage, worldId)).toEqual([]);

      // 5. The record: the pause, its grounds, and its lift.
      const recorded = adminPage.locator(
        `[data-testid="play-pause-record-pause"][data-pause-id="${pause.id}"]`,
      );
      await expect(recorded).toHaveAttribute("data-lifted", "true", {
        timeout: 15_000,
      });
      await expect(recorded).toContainText(worldName);
      await expect(
        recorded.getByTestId("play-pause-record-grounds"),
      ).toHaveText(grounds);
      const lift = recorded.getByTestId("play-pause-record-lift");
      await expect(lift).toContainText(/^Lifted .+ by \S/);
      await expect(lift).toContainText(liftGrounds);
      await expect(recorded.getByRole("button", { name: /Lift/ })).toHaveCount(
        0,
      );
      await expectNoAxeViolations(adminPage);

      // 3. The notice notices on its own, without a reload, within 30 s.
      const back = gmPage.getByRole("link", { name: "Return to the world" });
      await expect(back).toBeVisible({
        timeout: Math.max(1_000, 30_000 - (Date.now() - liftedAtMs)),
      });
      const sameDocument = await gmPage.evaluate(
        () =>
          (window as unknown as { __e2eSameDocument?: boolean })
            .__e2eSameDocument === true,
      );
      expect(sameDocument, "the notice must not have reloaded").toBe(true);
      await back.click();
      await expect(gmPage).toHaveURL(new RegExp(`/world/${worldId}/play$`));
      await expect(gmPage.locator("canvas")).toBeVisible({ timeout: 60_000 });
      await expect(gmPage.getByTestId("play-paused-notice")).toHaveCount(0);
      // Play has really started: a few seconds on, still on the playfield.
      await gmPage.waitForTimeout(6_000);
      await expect(gmPage).toHaveURL(new RegExp(`/world/${worldId}/play$`));

      // 4. Lifting restored play, not the scene.
      expect(await sceneModerationStatus(gmPage, sceneId)).toBe(
        "CONTENT_DISABLED",
      );
      expect(await sceneIdsShownTo(gmPage, worldId)).not.toContain(sceneId);

      // 6. Paused again, then the takedown is reversed.
      await markDocument(gmPage);
      const again = `Paused again ${uniqueSuffix()}`;
      const { pause: second } = await pauseWorldAsOperator(
        adminPage,
        worldId,
        again,
      );
      await gmPage.waitForURL(/\/world\/[^/]+\/paused$/, { timeout: 15_000 });
      await expectPausedNotice(gmPage, worldName, {
        grounds: again,
        marked: true,
      });

      const counter = await fileCounterNoticeViaApi(gmPage, caseId);
      expect(counter.data?.submitCounterNotice.currentStatus).toBe(
        "COUNTER_NOTICE_FORWARDED",
      );
      elapseWaitingPeriod(caseId);
      // Asking is what restores it (spec 015's lazy restoration).
      expect(await sceneModerationStatus(gmPage, sceneId)).toBeNull();
      expect(await sceneIdsShownTo(adminPage, worldId)).toContain(sceneId);

      // The content is back; the pause is not lifted by it.
      expect(await activePauseIds(adminPage, worldId)).toEqual([second.id]);
      expect(await worldPlayPaused(gmPage, worldId)).toBe(true);
      // Longer than the notice's own poll: it asked again, and stayed.
      await gmPage.waitForTimeout(32_000);
      await expectPausedNotice(gmPage, worldName, {
        grounds: again,
        marked: true,
      });
      await expect(
        gmPage.getByRole("link", { name: "Return to the world" }),
      ).toHaveCount(0);

      // 7. The history (T056): both pauses, newest first, times only — the
      // lifted one with when it was paused and when it was lifted, the
      // current one still paused.
      await gmPage.goto(`/world/${worldId}/settings/system`);
      const history = gmPage.getByTestId("play-pause-history-card");
      await expect(history).toBeVisible({ timeout: 20_000 });
      const rows = history.getByTestId("play-pause-history-row");
      await expect(rows).toHaveCount(2);
      const [current, lifted] = [rows.nth(0), rows.nth(1)];
      await expect(current).toContainText("Still paused");
      await expect(current.locator("time")).toHaveCount(1);
      expect(
        Date.parse(
          (await current.locator("time").getAttribute("datetime")) ?? "",
        ),
      ).toBe(Date.parse(second.pausedAt));
      const liftedTimes = lifted.locator("time");
      await expect(liftedTimes).toHaveCount(2);
      const [pausedAt, liftedAt] = (
        await liftedTimes.evaluateAll((times) =>
          times.map((time) => time.getAttribute("datetime") ?? ""),
        )
      ).map((value) => Date.parse(value));
      expect(pausedAt).toBe(Date.parse(pause.pausedAt));
      expect(liftedAt).toBeGreaterThanOrEqual(pausedAt);
      expect(liftedAt).toBeLessThanOrEqual(Date.parse(second.pausedAt));
      await expect(history).not.toContainText(grounds);
      await expect(history).not.toContainText(liftGrounds);
    } finally {
      await adminPage.context().close();
      await claimantContext.close();
      await gmContext.close();
    }
  });
});
