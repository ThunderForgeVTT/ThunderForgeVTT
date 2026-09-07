import { test, expect } from "@playwright/test";
import { freshCredentials, graphql, register } from "./fixtures/helpers";
import { openAnotherClient } from "./fixtures/clients";

/**
 * Spec 036 US1 (a second window signs in without evicting the first) and the
 * regression guard FR-020 asks for.
 *
 * # The defect this covers
 *
 * `create_session` revoked every live session for an account on each login,
 * deliberately — "to reduce session replay risk". The cost was that one
 * account could be signed in in exactly one place: opening ThunderForge in a
 * second browser, a second profile or a private window signed the first one
 * out. It bit a live demo, and it is why every cross-client test in this
 * suite had to use two different accounts.
 *
 * ADR-073 removes the eviction and replaces what it was protecting with
 * sessions a person can see and end. These tests are the half that says the
 * removal happened and stays happened.
 */

/** A world, created by whoever is signed in on `page`. */
const CREATE_WORLD = `
  mutation CreateWorld($name: String!) {
    createWorld(input: { name: $name }) { id name }
  }
`;

const MY_WORLDS = `query MyWorlds { myWorlds { id name } }`;

test.describe("Spec 036: an account may be signed in more than once", () => {
  test("a second sign-in leaves the first session working", async ({
    page,
    browser,
  }) => {
    const creds = freshCredentials("e2econc");
    await register(page, creds);

    // Prove the first client works *before* the second signs in, so a failure
    // afterwards can only be the second sign-in's doing.
    const before = await graphql<{ data: { myWorlds: unknown[] } }>(
      page,
      MY_WORLDS,
      {},
    );
    expect(before.data).not.toBeNull();

    const second = await openAnotherClient(browser, creds, "context");

    // The first client is still authenticated. Under the old behaviour this
    // is the assertion that failed: its session had just been revoked.
    const after = await graphql<{ data: { myWorlds: unknown[] } | null }>(
      page,
      MY_WORLDS,
      {},
    );
    expect(after.data).not.toBeNull();

    // And so is the second.
    const fromSecond = await graphql<{ data: { myWorlds: unknown[] } | null }>(
      second,
      MY_WORLDS,
      {},
    );
    expect(fromSecond.data).not.toBeNull();

    await second.context().close();
  });

  test("either client can act, and the other sees what it did", async ({
    page,
    browser,
  }) => {
    const creds = freshCredentials("e2econcact");
    await register(page, creds);
    const second = await openAnotherClient(browser, creds, "context");

    const name = `Second Window World ${Date.now().toString(36)}`;
    const created = await graphql<{
      data: { createWorld: { id: string; name: string } } | null;
    }>(second, CREATE_WORLD, { name });
    expect(created.data).not.toBeNull();
    const worldId = created.data!.createWorld.id;

    // The first client, which never reloaded and was never signed out, is
    // still authorised and sees the world the second one made.
    const seen = await graphql<{
      data: { myWorlds: { id: string }[] } | null;
    }>(page, MY_WORLDS, {});
    expect(seen.data).not.toBeNull();
    expect(seen.data!.myWorlds.map((w) => w.id)).toContain(worldId);

    await second.context().close();
  });

  test("a second tab in the same browser is the same session", async ({
    page,
    browser,
  }) => {
    const creds = freshCredentials("e2econctab");
    await register(page, creds);

    // The case that always worked, asserted so a test can say which of the
    // two kinds of "second window" it means. A tab shares the cookie jar, so
    // it is not a second sign-in at all.
    const tab = await openAnotherClient(browser, creds, "tab", page);
    await tab.goto("/worlds");

    const fromTab = await graphql<{ data: { myWorlds: unknown[] } | null }>(
      tab,
      MY_WORLDS,
      {},
    );
    expect(fromTab.data).not.toBeNull();

    const fromOriginal = await graphql<{
      data: { myWorlds: unknown[] } | null;
    }>(page, MY_WORLDS, {});
    expect(fromOriginal.data).not.toBeNull();

    await tab.close();
  });

  /**
   * FR-020. The guard, stated as its own test so that restoring the
   * revoke-on-login statement fails something whose name says what happened.
   *
   * Three contexts rather than two: a bug that revoked "all but the newest"
   * would still leave two clients alive, and this would not catch it.
   */
  test("the login-time eviction has not come back", async ({
    page,
    browser,
  }) => {
    const creds = freshCredentials("e2econcguard");
    await register(page, creds);

    const second = await openAnotherClient(browser, creds, "context");
    const third = await openAnotherClient(browser, creds, "context");

    for (const [label, client] of [
      ["the first", page],
      ["the second", second],
      ["the third", third],
    ] as const) {
      const result = await graphql<{ data: unknown | null }>(
        client,
        MY_WORLDS,
        {},
      );
      expect(
        result.data,
        `${label} client should still be signed in — signing in must not end a session that already exists`,
      ).not.toBeNull();
    }

    await second.context().close();
    await third.context().close();
  });
});
