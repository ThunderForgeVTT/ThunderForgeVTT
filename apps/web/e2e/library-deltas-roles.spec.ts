import { expect, test, type Page } from "./fixtures/test";
import {
  freshCredentials,
  graphql,
  inviteAndJoinAsPlayer,
  register,
} from "./fixtures/helpers";
import {
  aReadField,
  aWorldRunning,
  baseAsStored,
  BOOK,
  browseTheBook,
  deltaOrigins,
  type Entry,
  type Gql,
  named,
  openEntry,
  readInTheBook,
  sql,
  uuid,
  worldReads,
} from "./fixtures/library-deltas";

/**
 * A world's own changes to the books it inherited, end to end (spec 050 US2,
 * T077; FR-020 to FR-028, FR-052, FR-052a, FR-081, FR-087).
 *
 * The claims, each of which a unit test can only assert about code:
 *
 * 5. **A Trusted Player may change what a world inherited, and a Player may
 *    not** (FR-020a, ADR-099) — the Player refused by the server, not merely
 *    shown no controls.
 */

test.describe.configure({ mode: "serial" });

test.describe("A world's changes to its books (spec 050 US2)", () => {
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
});
