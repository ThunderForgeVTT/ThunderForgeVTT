# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: token-authoring.spec.ts >> Player-owned token dragging (US3, T021-T023) >> player drags their primary and an additionally-granted token; cannot drag an unassigned one; can edit their primary's photo; has no create control
- Location: e2e/token-authoring.spec.ts:1671:7

# Error details

```
Error: locator.click: Target page, context or browser has been closed
Call log:
  - waiting for getByRole('button', { name: 'Generate Join Link' })

```

# Test source

```ts
  234 |   };
  235 | }
  236 | 
  237 | async function register(page: Page, creds: Credentials): Promise<void> {
  238 |   await page.goto("/register");
  239 |   await page.locator("#register-username").fill(creds.username);
  240 |   await page.locator("#register-email").fill(creds.email);
  241 |   await page.locator("#register-password").fill(creds.password);
  242 |   await page.locator("#register-password-confirmation").fill(creds.password);
  243 |   await page.getByRole("button", { name: "Create account" }).click();
  244 |   await page.waitForURL((url) => !url.pathname.startsWith("/register"), {
  245 |     timeout: 15_000,
  246 |   });
  247 | }
  248 | 
  249 | async function extractInviteCode(page: Page): Promise<string> {
  250 |   const input = page.locator("input[readonly]").first();
  251 |   await expect(input).toBeVisible({ timeout: 10_000 });
  252 |   const url = await input.inputValue();
  253 |   const code = new URL(url).pathname.split("/").pop();
  254 |   if (!code) throw new Error(`Could not extract invite code from URL: ${url}`);
  255 |   return code;
  256 | }
  257 | 
  258 | async function registerAndCreateWorld(
  259 |   page: Page,
  260 |   worldName: string,
  261 | ): Promise<string> {
  262 |   await register(page, freshCredentials("e2etok"));
  263 | 
  264 |   await page.goto("/worlds/create");
  265 |   await page.locator("#world-name").fill(worldName);
  266 |   await page.getByRole("button", { name: /create world/i }).click();
  267 |   // Spec 010: CreateWorldPage now navigates to /world/{id}/staging (not
  268 |   // the canvas directly, and not the dashboard).
  269 |   await page.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });
  270 |   await clickPlay(page);
  271 |   const match = /\/world\/([^/]+)\/play$/.exec(new URL(page.url()).pathname);
  272 |   if (!match) {
  273 |     throw new Error(`Could not extract world id from URL: ${page.url()}`);
  274 |   }
  275 |   return match[1];
  276 | }
  277 | 
  278 | /** Registers a brand-new account and returns its user id, captured from
  279 |  * the register REST response body (`session.user.id` — see
  280 |  * `apps/web/src/types/auth.ts`'s `AuthSessionResponse`) rather than any
  281 |  * UI element, since no page in this app currently displays a user's own
  282 |  * id anywhere (T021-T023's ownership-grant flow needs it to type into
  283 |  * `TokenPanel`'s "Owner user ID" input). */
  284 | async function registerAndGetUserId(
  285 |   page: Page,
  286 |   prefix: string,
  287 | ): Promise<string> {
  288 |   const creds = freshCredentials(prefix);
  289 |   const [response] = await Promise.all([
  290 |     page.waitForResponse(
  291 |       // `.ok()` (2xx), not `status() === 200`: the register endpoint's
  292 |       // real status turned out to be a non-200 2xx (found live — this
  293 |       // exact-200 check made the very first version of this helper hang
  294 |       // forever waiting for a response that had already arrived).
  295 |       (resp) =>
  296 |         resp.url().includes("/api/authentication/register") && resp.ok(),
  297 |     ),
  298 |     (async () => {
  299 |       await page.goto("/register");
  300 |       await page.locator("#register-username").fill(creds.username);
  301 |       await page.locator("#register-email").fill(creds.email);
  302 |       await page.locator("#register-password").fill(creds.password);
  303 |       await page
  304 |         .locator("#register-password-confirmation")
  305 |         .fill(creds.password);
  306 |       await page.getByRole("button", { name: "Create account" }).click();
  307 |     })(),
  308 |   ]);
  309 |   await page.waitForURL((url) => !url.pathname.startsWith("/register"), {
  310 |     timeout: 15_000,
  311 |   });
  312 |   const body = (await response.json()) as {
  313 |     session?: { user?: { id?: string } };
  314 |   };
  315 |   const userId = body.session?.user?.id;
  316 |   if (!userId) {
  317 |     throw new Error("Could not extract user id from register response");
  318 |   }
  319 |   return userId;
  320 | }
  321 | 
  322 | /** As the GM (on the `/world/{worldId}/play` view), briefly navigates to
  323 |  * the world dashboard to generate an invite code via
  324 |  * `CampaignSettingsPanel` (spec 005 US4), then returns to the play view.
  325 |  * Mirrors `invite-membership.spec.ts`'s flow, adapted since this file's
  326 |  * `registerAndCreateWorld` (unlike that file's) leaves the GM already
  327 |  * inside the play view, not the dashboard. */
  328 | async function generateInviteCodeFromDashboard(
  329 |   page: Page,
  330 |   worldId: string,
  331 | ): Promise<string> {
  332 |   await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  333 |   await page.goto(`/world/${worldId}`);
> 334 |   await page.getByRole("button", { name: "Generate Join Link" }).click();
      |                                                                  ^ Error: locator.click: Target page, context or browser has been closed
  335 |   const inviteCode = await extractInviteCode(page);
  336 |   await page.goto(`/world/${worldId}/play`);
  337 |   return inviteCode;
  338 | }
  339 | 
  340 | /** As a freshly-registered player, redeems an invite code and lands on
  341 |  * the world's play view. Mirrors `invite-membership.spec.ts`'s join
  342 |  * flow, continuing past the dashboard into `/play` via the same "Enter
  343 |  * world" link `registerAndCreateWorld` uses for the GM. */
  344 | async function joinWorldAndEnterPlay(
  345 |   page: Page,
  346 |   inviteCode: string,
  347 |   worldId: string,
  348 | ): Promise<void> {
  349 |   await page.goto(`/join/${inviteCode}`);
  350 |   await expect(page.getByRole("button", { name: "Join Campaign" })).toBeVisible(
  351 |     {
  352 |       timeout: 10_000,
  353 |     },
  354 |   );
  355 |   await page.getByRole("button", { name: "Join Campaign" }).click();
  356 |   await page.waitForURL(new RegExp(`/world/${worldId}(/actor-select)?$`), {
  357 |     timeout: 15_000,
  358 |   });
  359 |   await page.getByRole("link", { name: "Enter world" }).first().click();
  360 |   await page.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });
  361 |   await clickPlay(page);
  362 | }
  363 | 
  364 | async function createScene(page: Page, name: string): Promise<void> {
  365 |   // In full-screen canvas mode, "New scene" lives inside the
  366 |   // (collapsed-by-default) sidebar. Spec 010: staging is now its own
  367 |   // route (not mounted alongside `/play`), so there is exactly one
  368 |   // "new-scene-button" in the DOM here — no `:visible` disambiguation
  369 |   // against a second, hidden-but-mounted staging copy is needed anymore;
  370 |   // just ensure the Settings dock section is actually open before clicking.
  371 |   const newSceneButton = page.getByTestId("new-scene-button");
  372 |   if (!(await newSceneButton.isVisible().catch(() => false))) {
  373 |     await page.getByTestId("world-dock-tab-settings").click();
  374 |     await expect(newSceneButton).toBeVisible({ timeout: 10_000 });
  375 |   }
  376 |   await newSceneButton.click();
  377 |   await page.locator('[data-testid="new-scene-name-input"]:visible').fill(name);
  378 |   await page.locator('[data-testid="create-scene-submit"]:visible').click();
  379 |   await expect(page.getByTestId("new-scene-name-input")).toBeHidden({
  380 |     timeout: 10_000,
  381 |   });
  382 |   await expect(
  383 |     page.locator('[data-testid="scene-switcher"]:visible'),
  384 |   ).toContainText(name);
  385 | 
  386 |   // Make it the world's *active* scene, not merely this client's selection.
  387 |   //
  388 |   // Which scene a reload lands on is server state (ADR-046, spec 022), and
  389 |   // creating one through the switcher does not launch it. Every test in this
  390 |   // file creates its scene, creates a token in it, and then reloads — which
  391 |   // silently returned to the world's auto-created default scene, where that
  392 |   // token does not exist. The US3 ownership test is where it showed: its
  393 |   // first token was left behind in the abandoned scene while the two created
  394 |   // after the reload landed in the default one, so assigning ownership to
  395 |   // the first found no such row in the panel.
  396 |   const worldId = /\/world\/([^/]+)/.exec(new URL(page.url()).pathname)?.[1];
  397 |   if (!worldId) {
  398 |     throw new Error(`createScene needs to be on a world route: ${page.url()}`);
  399 |   }
  400 |   await launchSceneByName(page, worldId, name);
  401 |   await page.goto(`/world/${worldId}/play`);
  402 | }
  403 | 
  404 | type Box = { x: number; y: number; width: number; height: number };
  405 | 
  406 | /** See canvas-authoring.spec.ts's identical helper for the full
  407 |  * rationale (Bevy mounts its canvas to `<body>`, not the named
  408 |  * container). */
  409 | async function canvasBox(page: Page): Promise<Box> {
  410 |   const canvas = page.locator("canvas");
  411 |   await canvas.scrollIntoViewIfNeeded();
  412 |   const box = await canvas.boundingBox();
  413 |   if (!box) {
  414 |     throw new Error("Bevy canvas element not found");
  415 |   }
  416 |   return box;
  417 | }
  418 | 
  419 | /** See canvas-authoring.spec.ts's identical helper for the full
  420 |  * rationale (GM flag / bridge-ready / canvas-focus race). */
  421 | async function waitForEngineReady(page: Page): Promise<void> {
  422 |   const canvas = page.locator("canvas");
  423 |   // Spec 010: `/world/:id/play` is a real route now, not a client-state
  424 |   // toggle — a reload keeps the same URL, so the canvas is simply still
  425 |   // mounting (WASM engine startup) and no staging "Play" button will
  426 |   // ever appear here. Only click Play when we're actually still on the
  427 |   // staging route (a one-shot `isVisible()` check on the canvas used to
  428 |   // stand in for this, but it raced the canvas's own mount and could
  429 |   // misfire a click on a "play-button" that doesn't exist on /play,
  430 |   // hanging for the full timeout).
  431 |   if (/\/staging$/.test(new URL(page.url()).pathname)) {
  432 |     await page.getByTestId("play-button").click({ timeout: 15_000 });
  433 |   }
  434 |   await expect(canvas).toBeVisible({ timeout: 15_000 });
```