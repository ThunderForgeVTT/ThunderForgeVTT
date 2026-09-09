import { expect, test, type Page } from "@playwright/test";
import {
  clickPlay,
  freshCredentials,
  graphql,
  inviteAndJoinAsPlayer,
  openDockTab,
  register,
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";
import { openAnotherClient } from "./fixtures/clients";

/**
 * Spec 031's shared initiative tracker, driven through the Play dock's
 * Combat section (`PlayDock/CombatPanel.tsx`) against the real server
 * (`graphql/mutations_combat.rs`).
 *
 * # What this covers, and why it is e2e rather than unit
 *
 * The turn structure had fourteen Rust tests and a full set of `data-testid`s
 * and not one thing in this suite that pressed a button. The parts that were
 * proven were `sort_combatants` and `next_turn_index` in isolation; what was
 * not proven is the claim the tracker actually makes — that the Game Master
 * and every player at the table are looking at the *same* round and the *same*
 * active combatant, at the same moment, without anybody reloading.
 *
 * That claim cannot be made by a unit test, because it spans two browser
 * contexts, a mutation, a `world_events` broadcast (code 18) and a refetch. So
 * it is made here:
 *
 * 1. A Game Master starts an encounter, files combatants into it, takes one
 *    out, and ends it.
 * 2. The turn walks the order the server sorted, and passing the last
 *    combatant opens a new round.
 * 3. A second person, in their own session, watches the round and the active
 *    combatant change on their screen — and is refused by the server when they
 *    try to advance the turn themselves.
 *
 * # What it deliberately does not cover
 *
 * `combat-selection-offer` / `combat-selection-list` / `combat-selection-row`
 * / `start-combat-with-selection-button` / `combat-add-selected-button` — the
 * "offer me the tokens I have selected" path (FR-030). Every one of those
 * needs a real map selection made through the engine, which is a different
 * fixture and a different kind of test; the roster arithmetic behind them is
 * already unit-tested in `combatRoster.ts`'s suite. Flagged rather than
 * skipped silently, so the gap is a known one.
 */

interface CreatedActor {
  id: string;
  label: string;
}

/** A GraphQL call that must succeed, in the style of `interactive-secrets`. */
async function gql<T>(
  page: Page,
  query: string,
  variables: Record<string, unknown>,
): Promise<T> {
  const res = await graphql<{ data?: T; errors?: { message: string }[] }>(
    page,
    query,
    variables,
  );
  if (res.errors?.length || !res.data) {
    throw new Error(`GraphQL failed: ${JSON.stringify(res.errors ?? res)}`);
  }
  return res.data;
}

/**
 * An NPC for the roster, made over the API rather than through the compendium.
 *
 * These specs need *someone to put in the initiative order*; how a Game Master
 * authors an NPC is `content-collections`' subject, not this file's. NPCs
 * rather than player characters on purpose: an actor a player could claim
 * would send the second account to Actor Selection instead of the map.
 */
async function createNpc(
  page: Page,
  worldId: string,
  name: string,
): Promise<CreatedActor> {
  const label = `${name} ${uniqueSuffix()}`;
  const created = await gql<{ createActor: { id: string } }>(
    page,
    `mutation ($input: CreateActorInput!) {
      createActor(input: $input) { id }
    }`,
    { input: { worldId, label, isNpc: true, gameSystemId: "genie" } },
  );
  return { id: created.createActor.id, label };
}

/** Open the dock's Combat section on a page already in the Play view. */
async function openCombatSection(page: Page): Promise<void> {
  // Before the dock is opened, not after: `waitForEngineReady` clicks the
  // bottom-right of the canvas to settle the engine, and an open dock panel
  // is sitting exactly there.
  await waitForEngineReady(page);
  await openDockTab(page, "combat");
  await expect(page.getByTestId("combat-panel")).toBeVisible({
    timeout: 20_000,
  });
}

/** The Game Master's way in: the staging page's Play button. */
async function gmOpensCombatPanel(page: Page, worldId: string): Promise<void> {
  await page.goto(`/world/${worldId}/staging`);
  await clickPlay(page);
  await openCombatSection(page);
}

/**
 * A player's way in: straight to the Play view.
 *
 * Not through staging — that is the Game Master's prep screen, and a player
 * has no Play button to press on it.
 */
async function playerOpensCombatPanel(
  page: Page,
  worldId: string,
): Promise<void> {
  await page.goto(`/world/${worldId}/play`);
  await openCombatSection(page);
}

/** Put `actor` into the running combat through the panel's actor picker. */
async function addCombatantByActor(
  page: Page,
  actor: CreatedActor,
  expectedRows: number,
): Promise<void> {
  const select = page.getByTestId("combat-add-actor-select");
  await select.selectOption({ label: actor.label });
  await page.getByTestId("combat-add-button").click();
  await expect(page.getByTestId("combatant-row")).toHaveCount(expectedRows, {
    timeout: 20_000,
  });
}

/**
 * Set one combatant's initiative through the row's own number field.
 *
 * The panel renders combatants in exactly the order the server sent, so the
 * only way to assert the order from outside is to give the server something to
 * sort and then read the list back.
 */
async function setInitiative(
  page: Page,
  label: string,
  initiative: number,
): Promise<void> {
  const field = page.getByLabel(`Initiative for ${label}`);
  await field.fill(String(initiative));
  await expect(field).toHaveValue(String(initiative), { timeout: 20_000 });
}

/** The combatant labels currently on screen, top of the order first. */
async function expectOrder(page: Page, labels: string[]): Promise<void> {
  const rows = page.getByTestId("combatant-row");
  await expect(rows).toHaveCount(labels.length, { timeout: 20_000 });
  for (const [index, label] of labels.entries()) {
    await expect(rows.nth(index)).toContainText(label, { timeout: 20_000 });
  }
}

test.describe("Spec 031: the shared initiative tracker", () => {
  test("a Game Master starts an encounter, fills the roster, removes a combatant, and ends it", async ({
    page,
  }) => {
    test.setTimeout(180_000);

    const worldId = await registerAndCreateWorld(
      page,
      `E2E Combat Roster ${uniqueSuffix()}`,
      "e2ecombatgm",
    );
    // Before the panel mounts: it fetches the world's actors once, on mount,
    // and the dock only mounts the section that is open.
    const sentry = await createNpc(page, worldId, "Sentry");
    const marauder = await createNpc(page, worldId, "Marauder");

    await gmOpensCombatPanel(page, worldId);

    await test.step("no combat in progress offers a start and nothing else", async () => {
      await expect(page.getByTestId("start-combat-button")).toBeVisible({
        timeout: 20_000,
      });
      await expect(page.getByTestId("combat-round-counter")).toHaveCount(0);
      await expect(page.getByTestId("combatant-list")).toHaveCount(0);
      await expect(page.getByTestId("advance-turn-button")).toHaveCount(0);
    });

    await test.step("starting combat opens round 1 with an empty order", async () => {
      await page.getByTestId("start-combat-button").click();
      // Genie declares `turnStructure.rounds` with the label "Round"
      // (packs/systems/genie/system.json), so the counter is shown and says so.
      await expect(page.getByTestId("combat-round-counter")).toHaveText(
        "Round 1",
        { timeout: 20_000 },
      );
      await expect(page.getByTestId("start-combat-button")).toHaveCount(0);
      await expect(page.getByTestId("combatant-row")).toHaveCount(0);
      // Nothing to advance to: the panel refuses the press rather than
      // sending a mutation the server would answer with "No active
      // combatants to advance to".
      await expect(page.getByTestId("advance-turn-button")).toBeDisabled();
    });

    await test.step("actors are filed into the order and leave the picker", async () => {
      await addCombatantByActor(page, sentry, 1);
      // The picker clears itself after an add, so a second press cannot file
      // the same actor twice by inertia.
      await expect(page.getByTestId("combat-add-actor-select")).toHaveValue("");
      await addCombatantByActor(page, marauder, 2);

      await expectOrder(page, [sentry.label, marauder.label]);
      // Already in the fight, so no longer offered: `addableActors`.
      await expect(
        page
          .getByTestId("combat-add-actor-select")
          .locator("option", { hasText: sentry.label }),
      ).toHaveCount(0);
      await expect(page.getByTestId("advance-turn-button")).toBeEnabled();
    });

    await test.step("removing a combatant takes them out of the order", async () => {
      await page
        .getByRole("button", { name: `Remove ${sentry.label}` })
        .click();
      await expectOrder(page, [marauder.label]);
      // And they are offered again — a combatant removed by mistake has to be
      // re-addable, or the mistake is permanent.
      await expect(
        page
          .getByTestId("combat-add-actor-select")
          .locator("option", { hasText: sentry.label }),
      ).toHaveCount(1);
    });

    await test.step("ending combat returns the panel to its resting state", async () => {
      await page.getByTestId("end-combat-button").click();
      await expect(page.getByTestId("start-combat-button")).toBeVisible({
        timeout: 20_000,
      });
      await expect(page.getByTestId("combat-round-counter")).toHaveCount(0);
      await expect(page.getByTestId("combatant-list")).toHaveCount(0);
    });
  });

  /**
   * The turn walks the order the server sorted, and a full pass is a round.
   *
   * # Why the roster is built in the wrong order on purpose
   *
   * Combatants are added lowest-initiative-first, so that the list as first
   * rendered is the *reverse* of the order the fight should run in. Setting
   * the initiatives afterwards is then a real claim about `sort_combatants`
   * being the single authority on turn order — if the panel were quietly
   * keeping its own list, the rows would never move.
   */
  test("the turn advances down the initiative order and wraps into a new round", async ({
    page,
  }) => {
    test.setTimeout(180_000);

    const worldId = await registerAndCreateWorld(
      page,
      `E2E Combat Turns ${uniqueSuffix()}`,
      "e2ecombatturn",
    );
    const vanguard = await createNpc(page, worldId, "Vanguard");
    const rival = await createNpc(page, worldId, "Rival");
    const bystander = await createNpc(page, worldId, "Bystander");

    await gmOpensCombatPanel(page, worldId);
    await page.getByTestId("start-combat-button").click();
    await expect(page.getByTestId("combat-round-counter")).toHaveText(
      "Round 1",
      { timeout: 20_000 },
    );

    await addCombatantByActor(page, bystander, 1);
    await addCombatantByActor(page, rival, 2);
    await addCombatantByActor(page, vanguard, 3);
    await expectOrder(page, [bystander.label, rival.label, vanguard.label]);

    await setInitiative(page, vanguard.label, 20);
    await setInitiative(page, rival.label, 10);
    await setInitiative(page, bystander.label, 5);
    await expectOrder(page, [vanguard.label, rival.label, bystander.label]);

    const rows = page.getByTestId("combatant-row");
    const round = page.getByTestId("combat-round-counter");

    // Nobody has acted yet, so nobody is highlighted.
    for (let index = 0; index < 3; index += 1) {
      await expect(rows.nth(index)).toHaveAttribute(
        "data-active-turn",
        "false",
      );
    }

    const advance = page.getByTestId("advance-turn-button");

    // The first press enters the order. Entering it is not a new round, and
    // the person who rolled highest acts first — a combatant at the top of
    // the initiative order who never gets a turn in round one is the whole
    // reason initiative was rolled.
    await advance.click();
    await expect(rows.nth(0)).toHaveAttribute("data-active-turn", "true", {
      timeout: 20_000,
    });
    await expect(round).toHaveText("Round 1");

    await advance.click();
    await expect(rows.nth(1)).toHaveAttribute("data-active-turn", "true", {
      timeout: 20_000,
    });
    await expect(rows.nth(0)).toHaveAttribute("data-active-turn", "false");
    await expect(round).toHaveText("Round 1");

    await advance.click();
    await expect(rows.nth(2)).toHaveAttribute("data-active-turn", "true", {
      timeout: 20_000,
    });
    await expect(round).toHaveText("Round 1");

    // Past the last combatant is the top of the order again, in a new round.
    await advance.click();
    await expect(round).toHaveText("Round 2", { timeout: 20_000 });
    await expect(rows.nth(0)).toHaveAttribute("data-active-turn", "true");
    await expect(rows.nth(2)).toHaveAttribute("data-active-turn", "false");
  });

  /**
   * The point of the whole feature: two people, two sessions, one turn order.
   *
   * Nothing here reloads. A tracker that only agrees after a refresh is not a
   * shared tracker, it is two trackers that happen to read the same table.
   */
  test("a player watches the round and the active combatant change, and may not advance the turn", async ({
    page,
    browser,
  }) => {
    test.setTimeout(300_000);

    const worldId = await registerAndCreateWorld(
      page,
      `E2E Combat Shared ${uniqueSuffix()}`,
      "e2ecombatshare",
    );
    const sentinel = await createNpc(page, worldId, "Lone Sentinel");

    await gmOpensCombatPanel(page, worldId);
    await page.getByTestId("start-combat-button").click();
    await expect(page.getByTestId("combat-round-counter")).toHaveText(
      "Round 1",
      { timeout: 20_000 },
    );
    await addCombatantByActor(page, sentinel, 1);

    const playerPage = await inviteAndJoinAsPlayer(
      browser,
      page,
      worldId,
      "e2ecombatplyr",
    );

    try {
      const playerRound = playerPage.getByTestId("combat-round-counter");
      const playerRows = playerPage.getByTestId("combatant-row");

      await test.step("the player is shown the tracker they are in, read-only", async () => {
        await playerOpensCombatPanel(playerPage, worldId);
        await expect(playerRound).toHaveText("Round 1", { timeout: 30_000 });
        await expect(playerRows).toHaveCount(1, { timeout: 30_000 });
        await expect(playerRows.nth(0)).toContainText(sentinel.label);
        await expect(playerRows.nth(0)).toHaveAttribute(
          "data-active-turn",
          "false",
        );

        // Every control is the Game Master's. A greyed-out button would still
        // tell a player the tracker is theirs to drive; these are simply not
        // rendered for them.
        await expect(playerPage.getByTestId("advance-turn-button")).toHaveCount(
          0,
        );
        await expect(playerPage.getByTestId("end-combat-button")).toHaveCount(
          0,
        );
        await expect(
          playerPage.getByTestId("combat-add-actor-select"),
        ).toHaveCount(0);
        await expect(playerPage.getByTestId("start-combat-button")).toHaveCount(
          0,
        );
      });

      await test.step("the server refuses a player's advanceTurn even without the button", async () => {
        // Reading is open to any world member (`require_world_member`) — the
        // player has to be able to see the tracker they are in, and that is
        // also how this test learns the combat's id.
        const seen = await gql<{
          activeCombat: { id: string; round: number };
        }>(
          playerPage,
          `query ($worldId: UUID!) {
            activeCombat(worldId: $worldId) { id round }
          }`,
          { worldId },
        );
        expect(seen.activeCombat.round).toBe(1);

        const refused = await graphql<{
          data?: { advanceTurn?: { round: number } | null };
          errors?: { message: string }[];
        }>(
          playerPage,
          `
            mutation ($combatId: UUID!) {
              advanceTurn(combatId: $combatId) {
                id
                round
              }
            }
          `,
          { combatId: seen.activeCombat.id },
        );
        expect(
          refused.errors?.map((e) => e.message).join(" | "),
          "advance_turn_impl is GM-only, whatever the client sends",
        ).toMatch(/only the gm may advance the turn/i);
        expect(refused.data?.advanceTurn ?? null).toBeNull();

        // And the refusal changed nothing.
        const after = await gql<{ activeCombat: { round: number } }>(
          playerPage,
          `query ($worldId: UUID!) {
            activeCombat(worldId: $worldId) { round }
          }`,
          { worldId },
        );
        expect(after.activeCombat.round).toBe(1);
      });

      await test.step("the GM takes the turn and the player's screen follows", async () => {
        await page.getByTestId("advance-turn-button").click();
        await expect(page.getByTestId("combatant-row").nth(0)).toHaveAttribute(
          "data-active-turn",
          "true",
          { timeout: 20_000 },
        );

        // No reload: the `world_events` broadcast (code 18) is what carries
        // this, and if it did not the player would sit on a stale order.
        await expect(playerRows.nth(0)).toHaveAttribute(
          "data-active-turn",
          "true",
          { timeout: 30_000 },
        );
        await expect(playerRound).toHaveText("Round 1");
      });

      await test.step("a full pass of the order opens a new round for both", async () => {
        // One combatant, so the pass is a single press: the sole active
        // combatant keeps acting and every pass counts as a round.
        await page.getByTestId("advance-turn-button").click();
        await expect(page.getByTestId("combat-round-counter")).toHaveText(
          "Round 2",
          { timeout: 20_000 },
        );
        await expect(playerRound).toHaveText("Round 2", { timeout: 30_000 });
        await expect(playerRows.nth(0)).toHaveAttribute(
          "data-active-turn",
          "true",
        );
      });

      await test.step("ending the encounter clears it from the player's screen too", async () => {
        await page.getByTestId("end-combat-button").click();
        await expect(page.getByTestId("start-combat-button")).toBeVisible({
          timeout: 20_000,
        });
        await expect(playerRound).toHaveCount(0, { timeout: 30_000 });
        await expect(playerRows).toHaveCount(0);
        await expect(
          playerPage
            .getByTestId("combat-panel")
            .getByText("No combat in progress."),
        ).toBeVisible({ timeout: 30_000 });
      });
    } finally {
      await playerPage.context().close();
    }
  });

  /**
   * Spec 036 T060, FR-011: **one person, two screens.**
   *
   * The test above is two *people* — a Game Master and a player, who see
   * different things because the server decides they may. This is the case
   * that could not be written at all until ADR-073: the same account, signed
   * in twice, with the table on one screen and the tracker on another. Before
   * the eviction was removed, opening the second window signed the first one
   * out, so the shape this asserts was unreachable.
   *
   * What it adds beyond the player case is that both windows are the **Game
   * Master's**. A follower that only ever renders read-only would pass the
   * player test and fail this one, because here the second window must carry
   * the controls too — and pressing them in *either* window must move both.
   */
  test("one Game Master, two windows: both follow the round, and either can drive it", async ({
    page,
    browser,
  }) => {
    test.setTimeout(300_000);

    // Registered by hand rather than through `registerAndCreateWorld`,
    // because the second window has to sign in as the same person and that
    // helper keeps its credentials to itself.
    const creds = freshCredentials("e2ecombat2win");
    await register(page, creds);
    await page.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
    await page
      .locator("#world-name")
      .fill(`E2E Combat Two Windows ${uniqueSuffix()}`);
    await page.getByRole("button", { name: /create world/i }).click();
    await page.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });
    const worldId = /\/world\/([^/]+)\/staging$/.exec(
      new URL(page.url()).pathname,
    )![1];

    const vanguard = await createNpc(page, worldId, "Vanguard");
    const rival = await createNpc(page, worldId, "Rival");

    await gmOpensCombatPanel(page, worldId);
    await page.getByTestId("start-combat-button").click();
    await expect(page.getByTestId("combat-round-counter")).toHaveText(
      "Round 1",
      { timeout: 20_000 },
    );
    await addCombatantByActor(page, vanguard, 1);
    await addCombatantByActor(page, rival, 2);
    await setInitiative(page, vanguard.label, 20);
    await setInitiative(page, rival.label, 10);

    const second = await openAnotherClient(browser, creds, "context");

    try {
      await test.step("the second window shows the same encounter, with the same controls", async () => {
        await playerOpensCombatPanel(second, worldId);
        await expect(second.getByTestId("combat-round-counter")).toHaveText(
          "Round 1",
          { timeout: 30_000 },
        );
        await expect(second.getByTestId("combatant-row")).toHaveCount(2, {
          timeout: 30_000,
        });
        // The same account, so the same authority. This is the assertion a
        // read-only follower would fail.
        await expect(second.getByTestId("advance-turn-button")).toBeVisible();
        await expect(second.getByTestId("end-combat-button")).toBeVisible();
      });

      await test.step("the first window takes a turn and the second follows without a reload", async () => {
        await page.getByTestId("advance-turn-button").click();
        await expect(page.getByTestId("combatant-row").nth(0)).toHaveAttribute(
          "data-active-turn",
          "true",
          { timeout: 20_000 },
        );
        await expect(
          second.getByTestId("combatant-row").nth(0),
        ).toHaveAttribute("data-active-turn", "true", { timeout: 30_000 });
      });

      await test.step("the second window drives it, and the first follows", async () => {
        // The direction that matters: two windows of one account are two
        // clients, not one client and a mirror.
        await second.getByTestId("advance-turn-button").click();
        await expect(
          second.getByTestId("combatant-row").nth(1),
        ).toHaveAttribute("data-active-turn", "true", { timeout: 30_000 });
        await expect(page.getByTestId("combatant-row").nth(1)).toHaveAttribute(
          "data-active-turn",
          "true",
          { timeout: 30_000 },
        );

        // And a full pass from the second window opens a new round on both.
        await second.getByTestId("advance-turn-button").click();
        await expect(second.getByTestId("combat-round-counter")).toHaveText(
          "Round 2",
          { timeout: 30_000 },
        );
        await expect(page.getByTestId("combat-round-counter")).toHaveText(
          "Round 2",
          { timeout: 30_000 },
        );
      });

      await test.step("ending it in one window ends it in the other", async () => {
        await second.getByTestId("end-combat-button").click();
        await expect(second.getByTestId("start-combat-button")).toBeVisible({
          timeout: 30_000,
        });
        await expect(page.getByTestId("combat-round-counter")).toHaveCount(0, {
          timeout: 30_000,
        });
      });
    } finally {
      await second.context().close();
    }
  });
});
