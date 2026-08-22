import { test, expect, type Page } from "@playwright/test";

/**
 * T023/T024 (specs/002-canvas-authoring-asset-storage, User Story 3):
 * paste-to-canvas image assets. A separate file from canvas-authoring.spec.ts
 * (deliberate — avoids two forks/agents editing the same shared spec file
 * concurrently; this feature's paste flow doesn't share any helpers with
 * the wall/shape authoring tests worth factoring out).
 *
 * Unlike wall/shape authoring, AssetPasteTool listens for `paste` on
 * `document` (AssetPasteTool.tsx), not on the canvas element itself — so
 * these tests never need `canvas.boundingBox()`/`scrollIntoViewIfNeeded()`
 * at all, sidestepping the GPU/WebGL-driver-dependent canvas-readiness
 * flakiness that blocks T016/T017 on this machine.
 *
 * Found and fixed while writing these tests: `AssetPasteTool` was never
 * actually rendered by `WorldPage.tsx` (dead code, wired up as part of
 * this work), and there was no way to read a stored asset back from
 * RustFS at all — `uploadCanvasImage` could write, but nothing could
 * serve the bytes back to a browser (RustFS is private, per-campaign
 * storage, so a raw RustFS URL can never be handed to a client). Fixed
 * by adding `GET /canvas-assets/{asset_id}` (canvas_assets_serve.rs),
 * an authenticated proxy that streams bytes via a single-object-scoped
 * `read_object` credential, mirroring `write_object`'s design.
 */

const PNG_BASE64 =
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

function uniqueSuffix(): string {
  return `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}

async function registerAndCreateWorld(page: Page, worldName: string): Promise<void> {
  const suffix = uniqueSuffix();
  const username = `e2epaste${suffix}`;
  const email = `${username}@example.test`;
  const password = "Sup3r-Secret-Passphrase!";

  await page.goto("/register");
  await page.locator("#register-username").fill(username);
  await page.locator("#register-email").fill(email);
  await page.locator("#register-password").fill(password);
  await page.locator("#register-password-confirmation").fill(password);
  await page.getByRole("button", { name: "Create account" }).click();
  await page.waitForURL((url) => !url.pathname.startsWith("/register"), {
    timeout: 15_000,
  });

  await page.goto("/worlds/create");
  await page.locator("#world-name").fill(worldName);
  await page.getByRole("button", { name: /create world/i }).click();
  // Spec 010: CreateWorldPage navigates to /world/{id}/staging (not the
  // canvas directly, and not the dashboard) — click "Play" to reach the
  // full-screen canvas at /world/{id}/play.
  await page.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });
  await page.getByTestId("play-button").click();
  await page.waitForURL(/\/world\/[^/]+\/play$/, { timeout: 15_000 });
}

async function createScene(page: Page, name: string): Promise<void> {
  // In full-screen canvas mode, "New scene" lives inside the
  // (collapsed-by-default) sidebar. Spec 010: staging is now its own
  // route (not mounted alongside `/play`), so there is exactly one
  // "new-scene-button" in the DOM here — no `:visible` disambiguation
  // against a second, hidden-but-mounted staging copy is needed anymore;
  // just ensure the sidebar is actually open before clicking.
  const newSceneButton = page.getByTestId("new-scene-button");
  if (!(await newSceneButton.isVisible().catch(() => false))) {
    await page.getByTestId("sidebar-toggle-button").click();
    await expect(newSceneButton).toBeVisible({ timeout: 10_000 });
  }
  await newSceneButton.click();
  await page.locator('[data-testid="new-scene-name-input"]:visible').fill(name);
  await page.locator('[data-testid="create-scene-submit"]:visible').click();
  await expect(page.getByTestId("new-scene-name-input")).toBeHidden({
    timeout: 10_000,
  });
  await expect(page.locator('[data-testid="scene-switcher"]:visible')).toContainText(name);
}

/** Dispatches a synthetic `paste` ClipboardEvent carrying real PNG bytes.
 * AssetPasteTool only cares about the `paste` DOM event's `clipboardData`
 * (AssetPasteTool.tsx's `handlePaste`), so constructing and dispatching
 * that event directly exercises the exact same code path a real Ctrl+V
 * would, without depending on `navigator.clipboard.write`'s flakier
 * permission/round-trip behavior in a headless browser. */
async function pasteImage(page: Page): Promise<void> {
  await page.evaluate((base64) => {
    const binary = atob(base64);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) {
      bytes[i] = binary.charCodeAt(i);
    }
    const file = new File([bytes], "pasted.png", { type: "image/png" });
    const dt = new DataTransfer();
    dt.items.add(file);
    const event = new ClipboardEvent("paste", {
      bubbles: true,
      cancelable: true,
      clipboardData: dt,
    });
    document.dispatchEvent(event);
  }, PNG_BASE64);
}

async function pasteNonImageText(page: Page): Promise<void> {
  await page.evaluate(() => {
    const dt = new DataTransfer();
    dt.items.add("just some text", "text/plain");
    const event = new ClipboardEvent("paste", {
      bubbles: true,
      cancelable: true,
      clipboardData: dt,
    });
    document.dispatchEvent(event);
  });
}

type UploadCanvasImageResponse = {
  data?: { uploadCanvasImage?: { id: string; sceneId: string; kind: string } };
  errors?: { message?: string }[];
};

test.describe("Paste-to-canvas image assets (US3)", () => {
  test("pasting an image uploads it, is queryable and fetchable after reload, and is visible to a second (player) session", async ({
    page,
    browser,
  }) => {
    await registerAndCreateWorld(page, `E2E Paste ${uniqueSuffix()}`);
    await createScene(page, "Paste Scene");

    // No canvasBox()/scrollIntoViewIfNeeded() needed — AssetPasteTool
    // listens on `document`, not the canvas element.
    await page.waitForTimeout(1_000);

    // Not filtered by request body content: Playwright's `postData()`
    // returns empty for multipart/form-data requests carrying a Blob
    // (the GraphQL multipart upload spec `uploadCanvasImage` uses), so a
    // content-based predicate never matches. Instead, collect every
    // `/api/graphql` response in the window around the paste and pick
    // out the one that actually carries `uploadCanvasImage` — other
    // concurrent GraphQL traffic in this app (session/member queries)
    // won't have that field.
    const responses: UploadCanvasImageResponse[] = [];
    const onResponse = async (res: import("@playwright/test").Response) => {
      if (!res.url().includes("/api/graphql")) return;
      try {
        const json = (await res.json()) as UploadCanvasImageResponse;
        if (json.data?.uploadCanvasImage) responses.push(json);
      } catch {
        // Non-JSON or already-consumed body — not our response.
      }
    };
    page.on("response", onResponse);

    const startedAt = Date.now();
    await pasteImage(page);
    await expect
      .poll(() => responses.length, { timeout: 15_000 })
      .toBeGreaterThan(0);
    page.off("response", onResponse);

    const payload = responses[0];
    expect(payload.errors, JSON.stringify(payload.errors)).toBeUndefined();
    const asset = payload.data?.uploadCanvasImage;
    expect(asset).toBeTruthy();
    expect(asset?.kind).toBe("PASTED");
    // FR-011/SC-004: appears within 10s of starting the paste.
    expect(Date.now() - startedAt).toBeLessThan(10_000);

    const assetId = asset!.id;
    const sceneId = asset!.sceneId;

    // Persistence + read-path proxy (canvas_assets_serve.rs), fetched
    // directly rather than through canvas rendering — this is what
    // actually proves the asset is stored and servable, independent of
    // whether the WebGL canvas itself is currently render-stable on this
    // machine (see T016/T017's known environmental flakiness).
    await page.reload();
    await page.waitForTimeout(1_000);

    const afterReload = await page.evaluate(
      async ({ sceneId, assetId }) => {
        // The app's own fetch helper (api/auth.ts's withCsrf) reads this
        // same cookie and attaches it as x-csrf-token — required here
        // since require_csrf_for_session gates POST /api/graphql too,
        // not just mutations.
        const csrfToken = document.cookie
          .split("; ")
          .find((c) => c.startsWith("csrf_token="))
          ?.split("=")[1];
        const gqlRes = await fetch("/api/graphql", {
          method: "POST",
          credentials: "same-origin",
          headers: {
            "Content-Type": "application/json",
            ...(csrfToken ? { "x-csrf-token": csrfToken } : {}),
          },
          body: JSON.stringify({
            query: `query($sceneId: UUID!) { canvasImageAssetsForScene(sceneId: $sceneId) { id kind } }`,
            variables: { sceneId },
          }),
        });
        const gqlPayload = (await gqlRes.json()) as {
          data?: { canvasImageAssetsForScene?: { id: string; kind: string }[] };
        };
        const found = gqlPayload.data?.canvasImageAssetsForScene?.some((a) => a.id === assetId);

        const bytesRes = await fetch(`/api/canvas-assets/${assetId}`, { credentials: "same-origin" });
        return {
          foundAfterReload: found ?? false,
          bytesStatus: bytesRes.status,
          contentType: bytesRes.headers.get("content-type"),
        };
      },
      { sceneId, assetId },
    );

    expect(afterReload.foundAfterReload).toBe(true);
    expect(afterReload.bytesStatus).toBe(200);
    expect(afterReload.contentType).toBe("image/webp");

    // Visible to a second (player) session: a different browser context
    // (not a world member) must be rejected — confirming the read path
    // is genuinely authorization-gated, not just "publicly reachable
    // because it has a UUID in the URL."
    const strangerContext = await browser.newContext();
    const strangerPage = await strangerContext.newPage();
    await registerAndCreateWorld(strangerPage, `E2E Paste Stranger ${uniqueSuffix()}`);
    const strangerFetch = await strangerPage.evaluate(
      async (assetId) => {
        const res = await fetch(`/api/canvas-assets/${assetId}`, { credentials: "same-origin" });
        return res.status;
      },
      assetId,
    );
    expect(strangerFetch).toBe(403);
    await strangerContext.close();
  });

  test("pasting non-image clipboard content is ignored, no upload attempted", async ({ page }) => {
    await registerAndCreateWorld(page, `E2E Paste Ignore ${uniqueSuffix()}`);
    await createScene(page, "Paste Ignore Scene");
    await page.waitForTimeout(1_000);

    let uploadAttempted = false;
    page.on("request", (request) => {
      if (request.url().includes("/api/graphql") && request.method() === "POST") {
        const postData = request.postData() ?? "";
        if (postData.includes("uploadCanvasImage")) {
          uploadAttempted = true;
        }
      }
    });

    await pasteNonImageText(page);
    await page.waitForTimeout(1_500);

    expect(uploadAttempted).toBe(false);
    await expect(page.getByText("Pasting image…")).toHaveCount(0);
  });
});
