import { test, expect, type Page } from "./fixtures/test";
import { openDockTab, openGmTool } from "./fixtures/helpers";
import {
  uniqueSuffix,
  registerAndCreateWorld,
  createScene,
  waitForEngineReady,
  clickCanvasAt,
} from "./fixtures/canvasAuthoring";

test.describe("Placing a character from the actors pane (playtest 2026-09-10 P1)", () => {
  /** A 1x1 PNG — the smallest image the upload will decode. */
  const TINY_PNG = Buffer.from(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=",
    "base64",
  );

  async function csrfHeader(page: Page): Promise<Record<string, string>> {
    const token = (await page.context().cookies()).find(
      (cookie) => cookie.name === "csrf_token",
    )?.value;
    return token ? { "x-csrf-token": token } : {};
  }

  async function gql<T>(
    page: Page,
    query: string,
    variables: Record<string, unknown>,
  ): Promise<T> {
    const response = await page.request.post("/api/graphql", {
      headers: {
        "Content-Type": "application/json",
        ...(await csrfHeader(page)),
      },
      data: { query, variables },
    });
    return (await response.json()) as T;
  }

  test("Place creates a token, and the token shows the character's own art", async ({
    page,
  }) => {
    test.setTimeout(240_000);

    // Two defects, one symptom. Place emitted a drop that nothing listened
    // for, so no token was ever created (spec 031 T032's actor half, ticked
    // without being wired). And a token that was created drew a flat colour
    // square, because nothing connected the character's token image to what
    // the engine draws.
    await registerAndCreateWorld(page, `E2E Place ${uniqueSuffix()}`);
    await createScene(page, "Place Scene");
    await waitForEngineReady(page);
    const worldId = /\/world\/([^/]+)/.exec(new URL(page.url()).pathname)![1];

    const created = await gql<{ data: { createActor: { id: string } } }>(
      page,
      `mutation ($input: CreateActorInput!) { createActor(input: $input) { id } }`,
      {
        input: {
          worldId,
          label: `Placed Hero ${uniqueSuffix()}`,
          isNpc: false,
          gameSystemId: "genie",
        },
      },
    );
    const actorId = created.data.createActor.id;

    // Its token art, through the same multipart upload the sheet uses.
    const upload = await page.request.post("/api/graphql", {
      headers: await csrfHeader(page),
      multipart: {
        operations: JSON.stringify({
          query: `mutation ($actorId: UUID!, $role: String!, $file: Upload!) {
            uploadActorImage(actorId: $actorId, role: $role, file: $file) { assetId }
          }`,
          variables: { actorId, role: "token", file: null },
        }),
        map: JSON.stringify({ "0": ["variables.file"] }),
        "0": { name: "token.png", mimeType: "image/png", buffer: TINY_PNG },
      },
    });
    const uploaded = (await upload.json()) as {
      data?: { uploadActorImage: { assetId: string } };
      errors?: { message: string }[];
    };
    expect(uploaded.errors, "the token art should upload").toBeUndefined();

    const tokenCount = async () =>
      page.evaluate(() => window.__worldProbe?.state()?.counts.tokens ?? null);
    const before = await tokenCount();
    expect(
      before,
      "the world probe should be available in a dev build",
    ).not.toBeNull();

    // Place, then drop it on the map.
    await openDockTab(page, "actors");
    await page.getByTestId(`actor-place-${actorId}`).click();
    await clickCanvasAt(page, 0, 0);
    await expect
      .poll(tokenCount, {
        timeout: 15_000,
        message: "Place then a click must create a token",
      })
      .toBe(before! + 1);

    // The token answers with the character's art, in a form the engine can
    // load: `.webp`, because Bevy picks an image loader by extension.
    const scenes = await gql<{
      data: { scenes: { sceneId: string; name: string }[] };
    }>(
      page,
      `query ($worldId: UUID!) { scenes(worldId: $worldId) { sceneId name } }`,
      {
        worldId,
      },
    );
    const sceneId = scenes.data.scenes.find(
      (scene) => scene.name === "Place Scene",
    )!.sceneId;
    const tokens = await gql<{
      data: { tokens: { actorId: string | null; photoUrl: string | null }[] };
    }>(
      page,
      `query ($sceneId: UUID!) { tokens(sceneId: $sceneId) { actorId photoUrl } }`,
      {
        sceneId,
      },
    );
    const placed = tokens.data.tokens.find(
      (token) => token.actorId === actorId,
    );
    expect(placed, "the placed token belongs to the character").toBeTruthy();
    expect(placed!.photoUrl).toBe(
      `/api/actor-assets/${uploaded.data!.uploadActorImage.assetId}.webp`,
    );

    // And that URL serves the image — while Bevy's `.meta` probe beside it is
    // a 404, which is what lets the engine fall back to default settings.
    const art = await page.request.get(placed!.photoUrl!);
    expect(art.status()).toBe(200);
    expect(art.headers()["content-type"]).toContain("image/webp");
    const meta = await page.request.get(`${placed!.photoUrl!}.meta`);
    expect(meta.status()).toBe(404);
  });
});

test.describe("Placing a token from the actors pane (spec 031 US1)", () => {
  test("a cancelled placement leaves nothing behind", async ({ page }) => {
    test.setTimeout(180_000);

    // FR-005 and the reason placement is a state rather than a flag: leaving
    // the carry has exactly one exit, so "no trace" is guaranteed in one place
    // instead of at every path out.
    //
    // Driven through the engine boundary rather than the actors pane, because
    // the behaviour under test is the engine's. Whether a button is wired to
    // it is a different question, and mixing the two would leave a failure
    // ambiguous between them.
    await registerAndCreateWorld(page, `E2E Place Cancel ${uniqueSuffix()}`);
    await createScene(page, "Placement Scene");
    await waitForEngineReady(page);

    const tokenCount = async () =>
      page.evaluate(() => window.__worldProbe?.state().counts.tokens ?? -1);

    const before = await tokenCount();
    expect(
      before,
      "the world probe should be available in a dev build",
    ).toBeGreaterThanOrEqual(0);

    const began = await page.evaluate(async () => {
      const bevy = (await import(
        /* @vite-ignore */ "/src/engine/bevy/index.ts"
      )) as typeof import("../src/engine/bevy/index");
      return bevy.beginTokenPlacement("00000000-0000-0000-0000-00000000abcd");
    });
    expect(began, "the engine should accept a placement request").toBe(true);

    // Escape is the user-facing cancel; the engine treats a chrome-side cancel
    // identically, and both run the same OnExit.
    await page.keyboard.press("Escape");
    await page.waitForTimeout(500);

    expect(
      await tokenCount(),
      "a cancelled placement must not create a token",
    ).toBe(before);
  });

  test("the Select tool's filter is remembered, and says when it selects nothing", async ({
    page,
  }) => {
    test.setTimeout(180_000);

    // FR-009 and FR-010, plus the edge case: a filter that excludes everything
    // is legitimate and invisible, so the interface has to say so — including
    // when the menu is collapsed, which is the one combination with nothing on
    // screen to explain the silence.
    await registerAndCreateWorld(page, `E2E Select Filter ${uniqueSuffix()}`);
    await createScene(page, "Filter Scene");
    await waitForEngineReady(page);

    // `openGmTool`, not a bare click: the rail is a toggle and Select is open
    // by default, so clicking it *closes* the panel this test needs. The
    // helper checks first, which is exactly why it exists.
    //
    // It also works for Select now, where it did not before: the helper waits
    // for `gm-tool-panel-select`, which only started existing when Select
    // gained this filter menu.
    await openGmTool(page, "select");
    await expect(page.getByTestId("selection-filter")).toBeVisible();

    for (const kind of ["tokens", "walls", "lights", "shapes"]) {
      await page.getByTestId(`selection-filter-${kind}`).uncheck();
    }
    await expect(page.getByTestId("selection-filter-empty")).toBeVisible();

    await page.getByTestId("selection-filter-collapse").click();
    const collapsed = page.getByTestId("selection-filter-expand");
    await expect(collapsed).toBeVisible();
    await expect(collapsed).toHaveAttribute("data-selects-nothing", "true");

    // Survives a reload: the preference is per-person, not per-visit.
    await page.reload();
    await waitForEngineReady(page);
    // Collapsed state persisted, so the panel does not reopen — the collapsed
    // control is what should be showing, carrying the warning.
    await expect(page.getByTestId("selection-filter-expand")).toHaveAttribute(
      "data-selects-nothing",
      "true",
    );
  });
});
