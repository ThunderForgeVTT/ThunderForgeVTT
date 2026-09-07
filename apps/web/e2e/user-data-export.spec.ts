import { test, expect, type Page } from "@playwright/test";
import {
  freshCredentials,
  graphql,
  login,
  register,
  registerAndCreateWorld,
} from "./fixtures/helpers";
import { createNpcViaCompendium } from "./fixtures/content";

/**
 * ADR-011 (Export-My-Data Contract): `exportMyData`, `GET /api/user/data/export`
 * and `deleteMyData`, driven end to end against a real session.
 *
 * # Why this exists
 *
 * The contract has shipped since May 2026 with no end-to-end coverage at all —
 * neither `exportMyData` nor `deleteMyData` appeared anywhere in this suite.
 * It is also about to carry weight it has never carried: spec 039's
 * three-strike window offers a disabled account thirty days to *download its
 * data*, and a remedy is only a remedy if the payload is real.
 *
 * # There is no UI for this, deliberately
 *
 * No page in `apps/web/src` renders either operation, so the interface is the
 * whole surface. The session driving it is a real browser session all the
 * same, which is the half that was never proven: whether the contract answers
 * correctly for the person actually signed in, and refuses everybody else.
 */

interface ExportPayload {
  manifest: {
    schemaVersion: string;
    exportedAt: string;
    worlds: number;
    worldTokens: number;
    worldEvents: number;
  };
  user: { id: string; username: string };
  worlds: { id: string; name: string }[];
  worldTokens: { id: string }[];
  worldEvents: { id: string }[];
  scenes: { status: string }[];
  actors: { status: string }[];
  assetPacks: { status: string }[];
  gameSystems: { status: string }[];
}

const EXPORT_QUERY = `
  query ExportMyData {
    exportMyData {
      manifest {
        schemaVersion
        exportedAt
        worlds
        worldTokens
        worldEvents
      }
      user { id username }
      worlds { id name }
      worldTokens { id }
      worldEvents { id }
      scenes { schemaVersion status }
      actors { schemaVersion status }
      assetPacks { schemaVersion status }
      gameSystems { schemaVersion status }
    }
  }
`;

async function exportFor(page: Page): Promise<ExportPayload> {
  const result = await graphql<{ data: { exportMyData: ExportPayload } }>(
    page,
    EXPORT_QUERY,
    {},
  );
  return result.data.exportMyData;
}

async function createWorldNamed(page: Page, name: string): Promise<string> {
  await page.goto("/worlds/create");
  await page.locator("#world-name").fill(name);
  await page.getByRole("button", { name: /create world/i }).click();
  await page.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });
  const worldId = /\/world\/([^/]+)\/staging$/.exec(
    new URL(page.url()).pathname,
  )?.[1];
  if (!worldId) {
    throw new Error(`Could not extract world id from URL: ${page.url()}`);
  }
  return worldId;
}

test.describe("ADR-011: the export answers for the person signed in", () => {
  test("an export names the caller and the worlds they own", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `Export World ${Date.now().toString(36)}`,
      "e2eexport",
    );

    const payload = await exportFor(page);

    expect(payload.manifest.schemaVersion).toBe("v1");
    expect(Number.isNaN(new Date(payload.manifest.exportedAt).getTime())).toBe(
      false,
    );
    expect(payload.user.id).toBeTruthy();

    // The world just created is in it, and the manifest count agrees with the
    // body. A manifest that disagrees with what it describes is the failure
    // worth asserting against, because the count is what a person reads.
    expect(payload.worlds.map((world) => world.id)).toContain(worldId);
    expect(payload.manifest.worlds).toBe(payload.worlds.length);
    expect(payload.manifest.worldTokens).toBe(payload.worldTokens.length);
    expect(payload.manifest.worldEvents).toBe(payload.worldEvents.length);
  });

  test("an export carries nothing that authenticates the person it is about", async ({
    page,
  }) => {
    await registerAndCreateWorld(
      page,
      `Secret World ${Date.now().toString(36)}`,
      "e2eexpsec",
    );

    const rendered = JSON.stringify(await exportFor(page));

    // The export is built from the public projection of a user, and this is
    // what keeps it that way: a self-service download is precisely the payload
    // a credential must never appear in.
    for (const forbidden of [
      "passwordHash",
      "password_hash",
      "$argon2",
      "csrf_token",
    ]) {
      expect(rendered).not.toContain(forbidden);
    }
  });

  test("one person's export never contains another person's world", async ({
    browser,
  }) => {
    const firstContext = await browser.newContext();
    const firstPage = await firstContext.newPage();
    const mineId = await registerAndCreateWorld(
      firstPage,
      `Mine ${Date.now().toString(36)}`,
      "e2eexpmine",
    );

    const secondContext = await browser.newContext();
    const secondPage = await secondContext.newPage();
    const theirsId = await registerAndCreateWorld(
      secondPage,
      `Theirs ${Date.now().toString(36)}`,
      "e2eexpthem",
    );

    const owned = (await exportFor(secondPage)).worlds.map((w) => w.id);
    expect(owned).toContain(theirsId);
    expect(owned).not.toContain(mineId);

    await firstContext.close();
    await secondContext.close();
  });

  test("the download route serves the same contract, and refuses a stranger", async ({
    page,
    request,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `Route World ${Date.now().toString(36)}`,
      "e2eexproute",
    );

    const jsonResponse = await page.request.get("/api/user/data/export");
    expect(jsonResponse.status()).toBe(200);

    // The two paths share a builder and NOT a wire shape, which ADR-011 does
    // not say and this pins: the download serialises the Rust struct through
    // serde, so it is snake_case with a nested `counts`, while the GraphQL
    // projection is camelCase with the counts flattened onto the manifest.
    // Anybody writing a client against one and reading the ADR would expect
    // the other.
    const downloaded = (await jsonResponse.json()) as {
      manifest: { schema_version: string; counts: { worlds: number } };
      worlds: { id: string }[];
    };
    expect(downloaded.manifest.schema_version).toBe("v1");
    expect(downloaded.worlds.map((world) => world.id)).toContain(worldId);
    expect(downloaded.manifest.counts.worlds).toBe(downloaded.worlds.length);

    // ADR-011 names the route `/user/data/export`; the router it lives in is
    // merged into the `/api` nest, so that is the path the world sees. Worth
    // stating because the ADR's own text does not.
    //
    // ADR-011 says the route wraps the same JSON as a ZIP, so only the framing
    // differs and the framing is what this asserts.
    const zipResponse = await page.request.get(
      "/api/user/data/export?format=zip",
    );
    expect(zipResponse.status()).toBe(200);
    expect(zipResponse.headers()["content-type"]).toContain("zip");

    // An unsupported format is refused rather than quietly treated as JSON,
    // which would hand somebody a file that is not what they asked for.
    const badFormat = await page.request.get(
      "/api/user/data/export?format=tar",
    );
    expect(badFormat.status()).toBe(400);

    // `request` is a context with no session of its own. A self-only contract
    // must answer nothing at all to somebody who is not signed in.
    const anonymous = await request.get("/api/user/data/export");
    expect(anonymous.status()).toBe(401);
  });

  /**
   * A pinning test, not an aspiration.
   *
   * ADR-011 reserved `scenes`, `actors`, `assetPacks` and `gameSystems` as
   * placeholders in May 2026, "until those domains are implemented". They are
   * implemented now — this test creates an actor and the export still returns
   * an empty array — so the placeholders are stale rather than pending:
   * `export_user_data_payload` covers worlds, tokens and events only.
   *
   * The assertion records what the contract does today, so that filling it in
   * fails here and is noticed, rather than passing silently against a test
   * that hedged. Whoever fills it in should rewrite this to assert the
   * content and amend ADR-011 to say so.
   *
   * It matters beyond tidiness: spec 039 offers a disabled account thirty days
   * to download its data, and a download missing that person's characters,
   * lore and items is not the remedy that spec promises.
   */
  test("the placeholder domains are still empty, and that is a known gap", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `Gap World ${Date.now().toString(36)}`,
      "e2eexpgap",
    );
    const actorId = await createNpcViaCompendium(
      page,
      worldId,
      "Someone Worth Exporting",
    );
    expect(actorId).toBeTruthy();

    const payload = await exportFor(page);
    expect(payload.worlds.map((world) => world.id)).toContain(worldId);
    expect(payload.actors).toEqual([]);
    expect(payload.scenes).toEqual([]);
    expect(payload.assetPacks).toEqual([]);
    expect(payload.gameSystems).toEqual([]);
  });
});

test.describe("ADR-011: deleting my data", () => {
  test("a delete removes the account, its worlds and its sessions", async ({
    page,
  }) => {
    const creds = freshCredentials("e2edelete");
    await register(page, creds);
    const worldId = await createWorldNamed(
      page,
      `Doomed World ${Date.now().toString(36)}`,
    );

    expect((await exportFor(page)).worlds.map((w) => w.id)).toContain(worldId);

    const deleted = await graphql<{
      data: {
        deleteMyData: {
          worldsDeleted: number;
          usersDeleted: number;
          sessionsDeleted: number;
        };
      };
    }>(
      page,
      `
        mutation DeleteMyData {
          deleteMyData {
            worldsDeleted
            usersDeleted
            sessionsDeleted
          }
        }
      `,
      {},
    );

    expect(deleted.data.deleteMyData.worldsDeleted).toBeGreaterThanOrEqual(1);
    expect(deleted.data.deleteMyData.usersDeleted).toBe(1);
    expect(deleted.data.deleteMyData.sessionsDeleted).toBeGreaterThanOrEqual(1);

    // The account is gone, so the session it was holding must be worthless
    // immediately — a delete that leaves a usable session has not deleted the
    // thing that mattered most.
    const afterDelete = await page.request.get("/api/user/data/export");
    expect(afterDelete.status()).toBe(401);

    // And the credentials no longer open anything.
    await login(page, creds.username, creds.password);
    await expect(page).toHaveURL(/\/login/, { timeout: 15_000 });
  });

  test("deleting my data does not touch anybody else's", async ({
    browser,
  }) => {
    const keeperContext = await browser.newContext();
    const keeperPage = await keeperContext.newPage();
    const keptId = await registerAndCreateWorld(
      keeperPage,
      `Kept ${Date.now().toString(36)}`,
      "e2edelkeep",
    );

    const leaverContext = await browser.newContext();
    const leaverPage = await leaverContext.newPage();
    await registerAndCreateWorld(
      leaverPage,
      `Leaving ${Date.now().toString(36)}`,
      "e2edelleave",
    );
    await graphql(
      leaverPage,
      `
        mutation DeleteMyData {
          deleteMyData {
            worldsDeleted
          }
        }
      `,
      {},
    );

    expect((await exportFor(keeperPage)).worlds.map((w) => w.id)).toContain(
      keptId,
    );

    await keeperContext.close();
    await leaverContext.close();
  });
});
