import { expect, test, type Page } from "./fixtures/test";
import { expectNoAxeViolations } from "./fixtures/axe";
import { graphql, openDockTab } from "./fixtures/helpers";
import {
  closeTable,
  must,
  openTable,
  placeCast,
  sitDown,
} from "../playtest/table";
import {
  addCombatant,
  openCombatPanel,
  rosterOn,
  startCombat,
} from "../playtest/combat";

/**
 * Owner decision 2026-09-15: whether players see an NPC is the Game Master's
 * choice, per NPC, and an NPC is hidden until they choose.
 *
 * Token-name hiding already kept a hidden name off the board and the combat
 * tracker, but a player could read every NPC in the character list. "Hidden"
 * here means what it means there: the server never sends it. So this watches
 * what reaches a player's browser (every GraphQL response body and every
 * subscription frame, the way `combat-attack.spec.ts` watches for the hidden
 * ogre), not only what is drawn.
 */

/** Distinctive enough that finding either in traffic means it leaked. */
const HIDDEN_NAME = "Vorlaine the Unseen";
const SHOWN_NAME = "Mirella of the Lantern Inn";

interface Traffic {
  responses: string[];
  frames: string[];
}

function record(page: Page): Traffic {
  const traffic: Traffic = { responses: [], frames: [] };
  page.on("response", (response) => {
    if (!response.url().includes("graphql")) return;
    void response
      .text()
      .then((body) => traffic.responses.push(body))
      .catch(() => undefined);
  });
  page.on("websocket", (socket) => {
    socket.on("framereceived", (frame) => {
      traffic.frames.push(String(frame.payload));
    });
  });
  return traffic;
}

async function createNpc(page: Page, worldId: string, label: string) {
  const { createActor } = await must<{
    createActor: { id: string; visibleToPlayers: boolean };
  }>(
    page,
    `mutation ($input: CreateActorInput!) {
      createActor(input: $input) { id visibleToPlayers }
    }`,
    { input: { worldId, label, isNpc: true, gameSystemId: "dnd5e" } },
  );
  return createActor;
}

test("a Game Master chooses which NPCs players see, and a hidden one never reaches a player", async ({
  page,
  browser,
}, testInfo) => {
  test.setTimeout(6 * 60_000);

  const table = await openTable({
    browser,
    gm: page,
    testInfo,
    system: "dnd5e",
    players: ["Aria"],
    sceneName: "The Lantern Inn",
  });
  const [aria] = table.players;
  const traffic = record(aria.page);

  try {
    await placeCast(table, {
      label: "Aria",
      at: { x: 0, y: 0 },
      seat: aria,
    });
    const hidden = await createNpc(table.gm, table.worldId, HIDDEN_NAME);
    const shown = await createNpc(table.gm, table.worldId, SHOWN_NAME);
    expect(hidden.visibleToPlayers, "a new NPC starts hidden").toBe(false);
    expect(shown.visibleToPlayers, "a new NPC starts hidden").toBe(false);

    await test.step("the Game Master sees both, and shows one from the list with the keyboard", async () => {
      await sitDown(table, table.gm);
      await openDockTab(table.gm, "actors");
      const panel = table.gm.getByTestId("actors-panel");
      await expect(panel).toContainText(HIDDEN_NAME);
      await expect(panel).toContainText(SHOWN_NAME);

      const showToggle = table.gm.getByRole("button", {
        name: `Visible to players: ${SHOWN_NAME}`,
      });
      const hiddenToggle = table.gm.getByRole("button", {
        name: `Visible to players: ${HIDDEN_NAME}`,
      });
      await expect(showToggle).toHaveAttribute("aria-pressed", "false");
      await expect(hiddenToggle).toHaveAttribute("aria-pressed", "false");
      await expectNoAxeViolations(table.gm, '[data-testid="actors-panel"]');

      await showToggle.focus();
      await table.gm.keyboard.press("Enter");
      await expect(showToggle).toHaveAttribute("aria-pressed", "true");
      await expect(showToggle).toHaveText("Shown");
      await expect(hiddenToggle).toHaveAttribute("aria-pressed", "false");
    });

    await test.step("a player's list, search, sheet and pages carry nothing of the hidden NPC", async () => {
      await sitDown(table, aria.page);
      await openDockTab(aria.page, "actors");
      const panel = aria.page.getByTestId("actors-panel");
      await expect(panel).toContainText(SHOWN_NAME, { timeout: 10_000 });
      await expect(panel).toContainText("Aria");
      await expect(panel).not.toContainText(HIDDEN_NAME);
      await expect(
        aria.page.getByTestId(`actor-visible-${shown.id}`),
        "a player has no switch to press",
      ).toHaveCount(0);

      // The same player, straight at the API: search, and every by-id read.
      const search = await must<{ searchActors: { id: string }[] }>(
        aria.page,
        `query ($worldId: UUID!) { searchActors(worldId: $worldId, query: "") { id } }`,
        { worldId: table.worldId },
      );
      expect(search.searchActors.map((a) => a.id)).toContain(shown.id);
      expect(search.searchActors.map((a) => a.id)).not.toContain(hidden.id);
      for (const read of [
        "actorSheet(actorId: $actorId) { all { id } }",
        "actorSystemData(actorId: $actorId) { id }",
        "actorInventory(actorId: $actorId) { id }",
        "actorAbilities(actorId: $actorId) { abilityId }",
      ]) {
        const answer = await graphql<{
          data?: unknown;
          errors?: { message: string }[];
        }>(aria.page, `query ($actorId: UUID!) { ${read} }`, {
          actorId: hidden.id,
        });
        expect(
          answer.errors?.[0]?.message,
          `${read.split("(")[0]} refuses the hidden NPC as if it did not exist`,
        ).toBe("Actor not found");
        expect(JSON.stringify(answer)).not.toContain(HIDDEN_NAME);
      }

      // Its page, opened by a player who has its id, says it is not there.
      await aria.page.goto(`/world/${table.worldId}/actor/${hidden.id}/view`);
      await expect(
        aria.page.getByRole("heading", { name: "Actor not found" }),
      ).toBeVisible({ timeout: 10_000 });
      // And the world's compendium lists only what a player may see.
      await aria.page.goto(`/world/${table.worldId}/compendium`);
      await aria.page.waitForLoadState("networkidle");
      await expect(aria.page.locator("body")).not.toContainText(HIDDEN_NAME);

      const everything = [...traffic.responses, ...traffic.frames];
      expect(
        traffic.responses.some(
          (body) => body.includes('"worldActors"') && body.includes(SHOWN_NAME),
        ),
        "the player's client did read the roster (the check below is not vacuous)",
      ).toBe(true);
      expect(
        traffic.frames.length,
        "the player's subscription carried frames (the frame check is not vacuous)",
      ).toBeGreaterThan(0);
      const carryingName = everything.filter((b) => b.includes(HIDDEN_NAME));
      const carryingId = everything.filter((b) => b.includes(hidden.id));
      testInfo.annotations.push({
        type: "Aria's traffic",
        description:
          `${traffic.responses.length} responses and ${traffic.frames.length} frames checked; ` +
          `${carryingName.length} carry the hidden NPC's name, ${carryingId.length} its id`,
      });
      expect(
        carryingName,
        "no response or frame carries the hidden NPC's name",
      ).toEqual([]);
      expect(
        carryingId,
        "no response or frame carries the hidden NPC's id",
      ).toEqual([]);
    });

    await test.step("shown from its own page, it reaches the player", async () => {
      await table.gm.goto(`/world/${table.worldId}/actor/${hidden.id}/view`);
      const block = table.gm.getByTestId("actor-visibility-block");
      await expect(block).toBeVisible({ timeout: 10_000 });
      const toggle = block.getByRole("checkbox", {
        name: "Visible to players",
      });
      await expect(toggle).not.toBeChecked();
      await expectNoAxeViolations(
        table.gm,
        '[data-testid="actor-visibility-block"]',
      );
      await toggle.focus();
      await table.gm.keyboard.press("Space");
      await expect(toggle).toBeChecked();

      await sitDown(table, aria.page);
      await openDockTab(aria.page, "actors");
      await expect(aria.page.getByTestId("actors-panel")).toContainText(
        HIDDEN_NAME,
        { timeout: 10_000 },
      );
      await must(
        aria.page,
        `query ($actorId: UUID!) { actorSheet(actorId: $actorId) { all { id } } }`,
        { actorId: hidden.id },
      );
    });
  } finally {
    await closeTable(table);
  }
});

/**
 * Owner decision 2026-09-15, second half: one switch. Hiding an NPC hides its
 * tokens' names too — on the board and in the combat tracker — and showing it
 * hands them back without a reload.
 *
 * Before this, a hidden NPC's token still drew the creature's name above it on
 * every player's board, because a token's name fell back to its actor's label
 * with no visibility check: the whole point of hiding the creature, undone by
 * dropping a token of it.
 */
const LURKER_NAME = "Thessaly Nightglass";

type Nameplate = { tokenId: string; text: string; dimmed: boolean };

async function plateFor(page: Page, tokenId: string): Promise<Nameplate | null> {
  const plates = await page.evaluate(
    () =>
      (
        window as unknown as {
          __engineProbe?: { nameplates?: () => Nameplate[] };
        }
      ).__engineProbe?.nameplates?.() ?? [],
  );
  return plates.find((plate) => plate.tokenId === tokenId) ?? null;
}

test("a hidden NPC's token is nameless on a player's board and tracker, and showing it names them at once", async ({
  page,
  browser,
}, testInfo) => {
  test.setTimeout(6 * 60_000);

  const table = await openTable({
    browser,
    gm: page,
    testInfo,
    system: "dnd5e",
    players: ["Aria"],
    sceneName: "The Cellar",
  });
  const [aria] = table.players;
  const traffic = record(aria.page);

  try {
    await placeCast(table, { label: "Aria", at: { x: 0, y: 0 }, seat: aria });
    // Hidden, which is how every NPC starts. Its token's own name switch is
    // left alone — on, the default — so the creature's state is the only
    // thing deciding what a player reads.
    const lurker = await placeCast(table, {
      label: LURKER_NAME,
      at: { x: 96, y: 0 },
      tokenType: "npc",
      visibleToPlayers: false,
    });

    const combat = await startCombat(table);
    await addCombatant(table, combat.id, {
      label: "Aria",
      tokenId: (await must<{ tokens: { tokenId: string; name: string }[] }>(
        table.gm,
        `query ($sceneId: UUID!) { tokens(sceneId: $sceneId) { tokenId name } }`,
        { sceneId: table.sceneId },
      ).then(
        (answer) =>
          answer.tokens.find((token) => token.name === "Aria")!.tokenId,
      )),
      initiative: 20,
    });
    await addCombatant(table, combat.id, {
      label: LURKER_NAME,
      actorId: lurker.actorId,
      tokenId: lurker.tokenId,
      initiative: 10,
      isNpc: true,
    });

    for (const client of [table.gm, aria.page]) {
      await sitDown(table, client);
    }

    await test.step("the Game Master reads the name; the player's board and tracker read nothing of it", async () => {
      await expect
        .poll(() => plateFor(table.gm, lurker.tokenId), { timeout: 20_000 })
        .toEqual({ tokenId: lurker.tokenId, text: LURKER_NAME, dimmed: false });

      // The player's canvas is sent no name, so it has none to draw. Polled
      // to a settled answer, then held long enough for one that was going to
      // arrive to have arrived.
      await expect
        .poll(
          () =>
            aria.page.evaluate(
              () => window.__worldProbe?.state()?.counts.tokens ?? 0,
            ),
          { timeout: 20_000 },
        )
        .toBeGreaterThan(1);
      await aria.page.waitForTimeout(1_500);
      expect(
        await plateFor(aria.page, lurker.tokenId),
        "the player's canvas has no name to draw above a hidden creature",
      ).toBeNull();

      await openCombatPanel(aria.page);
      await expect(aria.page.getByTestId("combatant-list")).toContainText(
        "Unknown",
        { timeout: 20_000 },
      );
      expect(
        (await rosterOn(aria.page)).join(" | "),
        "the tracker names nobody the player may not know",
      ).not.toContain(LURKER_NAME);

      // And straight at the API, in case a panel merely declined to draw it.
      const tokens = await must<{ tokens: { tokenId: string; name: string | null }[] }>(
        aria.page,
        `query ($sceneId: UUID!) { tokens(sceneId: $sceneId) { tokenId name } }`,
        { sceneId: table.sceneId },
      );
      expect(
        tokens.tokens.find((token) => token.tokenId === lurker.tokenId)?.name,
        "the token list a player is sent carries no name",
      ).toBeNull();

      const everything = [...traffic.responses, ...traffic.frames];
      expect(
        everything.length,
        "the player's client did read the board (the check below is not vacuous)",
      ).toBeGreaterThan(0);
      const carrying = everything.filter((body) =>
        body.includes(LURKER_NAME),
      );
      testInfo.annotations.push({
        type: "Aria's traffic",
        description:
          `${traffic.responses.length} responses and ${traffic.frames.length} frames checked; ` +
          `${carrying.length} carry the hidden creature's name`,
      });
      expect(
        carrying,
        "no response or frame carries a hidden creature's token name",
      ).toEqual([]);
    });

    await test.step("the Game Master shows the NPC, and the name appears with no reload", async () => {
      await must(
        table.gm,
        `mutation ($actorId: UUID!) {
          setActorVisibleToPlayers(actorId: $actorId, visible: true) { id }
        }`,
        { actorId: lurker.actorId },
      );

      await expect
        .poll(() => plateFor(aria.page, lurker.tokenId), { timeout: 20_000 })
        .toEqual({ tokenId: lurker.tokenId, text: LURKER_NAME, dimmed: false });
      await expect(aria.page.getByTestId("combatant-list")).toContainText(
        LURKER_NAME,
        { timeout: 20_000 },
      );
    });
  } finally {
    await closeTable(table);
  }
});
