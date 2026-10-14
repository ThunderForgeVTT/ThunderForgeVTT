import { expect, test, type Page } from "@playwright/test";
import { data, openDemo, WORLD_ID } from "./support";

/**
 * Spec 074 T023 (FR-013): every page of the web app's router, opened in the
 * demo, and what the demo does with it. The decision for each is recorded in
 * `specs/074-a-world-to-try/gm-pages.md`; this file is that table's proof.
 *
 *  - `absent`: the page is never shown. The app sends the visitor on to
 *    `/welcome`, as it does on a real instance that needs no setup and has
 *    the visitor signed in.
 *  - `notice`: the page opens, what it needs is refused, and the one notice
 *    says which part of ThunderForge is not part of the demo.
 *  - `works`: the page opens and nothing is refused.
 *
 * The sidebar's own pages are walked in `demo-pages.spec.ts`, and the board
 * in `demo-board.spec.ts`; they are listed here once more so the table has
 * one proof per row, in one file.
 */

let page: Page;
let outside: string[];

const W = `/world/${WORLD_ID}`;
const FAKE = "00000000-0000-4000-8000-000000000000";

test.describe.configure({ mode: "serial" });

test.beforeAll(async ({ browser, baseURL }) => {
  ({ page, outside } = await openDemo(browser, baseURL));
});

test.afterAll(async () => {
  await page.context().close();
});

test.afterEach(() => {
  expect(outside, "nothing leaves the demo's own static files").toEqual([]);
});

async function refused(): Promise<string[]> {
  return page.evaluate(() => [
    ...((window as { __thunderforgeDemoRefused?: string[] })
      .__thunderforgeDemoRefused ?? []),
  ]);
}

/** A full load of `route`, so what is refused is this page's alone. */
async function open(route: string): Promise<void> {
  await page.goto(`/demo${route}`);
  await page.waitForLoadState("networkidle");
}

/**
 * The notice's title, exactly: a page's own words ("Repository sync is not
 * part of the demo.", on the system settings page) are not a refusal.
 */
const notice = () => page.getByText("Not part of the demo", { exact: true });

/** Pages the demo never shows: the app sends the visitor on to Welcome. */
const ABSENT = [
  "/setup",
  "/setup/abc",
  "/setup/callback",
  "/login",
  "/register",
  "/signup",
  "/admin",
  "/admin/welcome",
  "/admin/settings",
  "/admin/configuration",
  "/admin/instance",
  "/admin/readiness",
  "/admin/mail",
  "/admin/legal",
  "/admin/analytics",
  "/admin/storage",
  "/admin/oauth",
  "/admin/system",
  "/admin/access",
  "/admin/security",
  "/admin/moderation",
  "/admin/play-pauses",
];

test("pages that need setup, sign-in or an administrator are never shown", async () => {
  for (const route of ABSENT) {
    await open(route);
    await expect(page, route).toHaveURL(/\/demo\/welcome$/);
    await expect(
      page.getByRole("heading", { name: "Welcome back to ThunderForge." }),
    ).toBeVisible();
    expect(await refused(), route).toEqual([]);
    await expect(notice(), route).toHaveCount(0);
  }
});

/**
 * Pages that open and say what they are missing: the route, the area the
 * notice names, and the words the page itself shows.
 */
const NOTICE: { route: string; area: string; page: string }[] = [
  {
    route: "/collection/abc123",
    area: "Share links",
    page: "Go to ThunderForge",
  },
  {
    route: `/library/${FAKE}`,
    area: "The compendium library",
    page: "could not be read",
  },
  { route: "/shared/actor/abc123", area: "Share links", page: "Return home" },
  {
    route: "/shared/ability/abc123",
    area: "Share links",
    page: "no longer available",
  },
  { route: "/shared/item/abc123", area: "Share links", page: "Return home" },
  {
    route: `${W}/ability/${FAKE}/view`,
    area: "That feature",
    page: "Ability not found",
  },
  {
    route: `${W}/ability/${FAKE}/edit`,
    area: "That feature",
    page: "Ability not found",
  },
  {
    route: `${W}/item/${FAKE}/view`,
    area: "That feature",
    page: "Item not found",
  },
  {
    route: `${W}/item/${FAKE}/edit`,
    area: "That feature",
    page: "Item not found",
  },
  {
    route: `${W}/compendium/item/${FAKE}/edit`,
    area: "That feature",
    page: "Item not found",
  },
  {
    route: "/settings/storage",
    area: "World settings",
    page: "Storage and sharing on this device",
  },
  {
    route: "/settings/feedback",
    area: "That feature",
    page: "Feedback you have sent",
  },
  {
    route: "/settings/security",
    area: "Your account",
    page: "Account security",
  },
  {
    route: "/settings/standing",
    area: "That feature",
    page: "Account standing",
  },
  {
    route: "/join/abc123",
    area: "Invite links",
    page: "This link could not be opened",
  },
  { route: "/worlds/create", area: "That feature", page: "Create a new world" },
];

test("pages that reach for a server say which part is not part of the demo", async () => {
  for (const { route, area, page: words } of NOTICE) {
    await open(route);
    await expect(page, route).toHaveURL(new RegExp(`/demo${route}$`));
    expect((await refused()).length, `refused on ${route}`).toBeGreaterThan(0);
    await expect(
      page.getByText(`${area} needs a real ThunderForge instance.`).first(),
      `notice on ${route}`,
    ).toBeVisible();
    await expect(page.getByText(words).first(), route).toBeVisible();
  }
});

test("creating another world is refused by name, from the page that offers it", async () => {
  await open("/worlds/create");
  await page.getByLabel("World name").fill("A second world");
  await page.getByRole("button", { name: "Create world" }).click();
  await expect(
    page.getByText(
      "Creating another world needs a real ThunderForge instance.",
    ),
  ).toBeVisible();
  await expect(page).toHaveURL(/\/demo\/worlds\/create$/);
});

test("sharing a character is refused by name, from the sheet's own Share", async () => {
  const { worldActors } = await data<{
    worldActors: { id: string; label: string }[];
  }>(page, "query ($w: UUID!) { worldActors(worldId: $w) { id label } }", {
    w: WORLD_ID,
  });
  const actor = worldActors.find((a) => a.label === "Brannoc Stoneward");
  expect(actor).toBeDefined();
  await open(`${W}/actor/${actor?.id}/view`);
  expect(await refused()).toEqual([]);
  await page.getByRole("button", { name: "Share", exact: true }).click();
  await expect(page.getByTestId("share-terms-error")).toBeVisible();
  await expect(
    page.getByText("Share links needs a real ThunderForge instance."),
  ).toBeVisible();
  await expect(page.getByTestId("share-terms-agree")).toHaveCount(0);
});

test("a sign-in provider's return and an instance invitation lead nowhere but Welcome", async () => {
  // No provider is offered, so the only way here is the address itself. The
  // page reports the missing exchange and offers the login, which is absent.
  await open("/oauth/callback/github");
  await expect(
    page.getByText("OAuth callback is missing the provider, code, or state."),
  ).toBeVisible();
  expect(await refused()).toEqual([]);
  await page.getByRole("link", { name: "Return to login" }).click();
  await expect(page).toHaveURL(/\/demo\/welcome$/);

  await open("/invite/abc123");
  await expect(
    page.getByRole("heading", { name: "You have been invited" }),
  ).toBeVisible();
  expect(await refused()).toEqual([]);
  await page.getByTestId("invite-register").click();
  await expect(page).toHaveURL(/\/demo\/welcome$/);
});

/** Pages that open with nothing refused: the route and its heading. */
const WORKS: { route: string; heading: string | RegExp }[] = [
  { route: "/welcome", heading: "Welcome back to ThunderForge." },
  { route: "/worlds", heading: "Every world in your library." },
  { route: "/library", heading: "Your library" },
  { route: "/legal/dmca", heading: "DMCA / Copyright Policy" },
  { route: "/legal/terms", heading: "Terms of Service" },
  { route: "/legal/privacy", heading: "Privacy Policy" },
  { route: "/legal/operator", heading: "Operator responsibilities" },
  { route: W, heading: "A World To Try" },
  { route: `${W}/paused`, heading: "Play has resumed" },
  { route: `${W}/staging`, heading: "A World To Try" },
  { route: `${W}/actor-select`, heading: "Choose your character" },
  { route: `${W}/collections`, heading: "Collections" },
  { route: `${W}/compendium`, heading: "A World To Try artifacts" },
  { route: `${W}/compendium/npc/new`, heading: "New NPC" },
  { route: `${W}/compendium/item/new`, heading: "New item" },
  { route: `${W}/scenes`, heading: "Scenes" },
  { route: `${W}/players`, heading: "Players" },
  { route: `${W}/lore/the-ambush/view`, heading: "The Ambush" },
  { route: `${W}/lore/the-ambush/edit`, heading: "The Ambush" },
  { route: `${W}/lore/the-ambush/history`, heading: "The Ambush" },
  { route: `${W}/settings/system`, heading: "A World To Try" },
  { route: "/settings/account", heading: "Your account" },
  { route: "/status", heading: "ThunderForge instance status" },
  { route: "/no-such-page", heading: "That page does not exist." },
];

test("every other page opens with nothing refused", async () => {
  for (const { route, heading } of WORKS) {
    await open(route);
    await expect(page, route).toHaveURL(new RegExp(`/demo${route}$`));
    await expect(
      page.getByRole("heading", { name: heading }).first(),
      route,
    ).toBeVisible();
    expect(await refused(), `refused on ${route}`).toEqual([]);
    await expect(notice(), `notice on ${route}`).toHaveCount(0);
  }
});

test("the pages of the seeded fighter and the first scene open with nothing refused", async () => {
  const { worldActors, scenes } = await data<{
    worldActors: { id: string; label: string }[];
    scenes: { sceneId: string; name: string }[];
  }>(
    page,
    `query ($w: UUID!) {
      worldActors(worldId: $w) { id label }
      scenes(worldId: $w) { sceneId name }
    }`,
    { w: WORLD_ID },
  );
  const actor = worldActors.find((a) => a.label === "Brannoc Stoneward");
  expect(actor).toBeDefined();
  const scene = scenes[0];
  const rows = [
    { route: `${W}/actor/${actor?.id}/view`, heading: "Brannoc Stoneward" },
    { route: `${W}/actor/${actor?.id}/edit`, heading: "Brannoc Stoneward" },
    { route: `${W}/actor/${actor?.id}/import`, heading: "Bring in a sheet" },
    {
      route: `${W}/compendium/npc/${actor?.id}/edit`,
      heading: "Edit Brannoc Stoneward",
    },
    { route: `${W}/scenes/${scene.sceneId}`, heading: scene.name },
  ];
  for (const { route, heading } of rows) {
    await open(route);
    await expect(
      page.getByRole("heading", { name: heading }).first(),
      route,
    ).toBeVisible();
    expect(await refused(), `refused on ${route}`).toEqual([]);
    await expect(notice(), `notice on ${route}`).toHaveCount(0);
  }
});

test("the root lands on the demo world, and play opens the board", async () => {
  await open("/");
  await expect(page).toHaveURL(new RegExp(`/demo${W}$`));
  await open(`${W}/play`);
  await expect(page.locator("canvas")).toBeVisible({ timeout: 60_000 });
  expect(await refused()).toEqual([]);
  await expect(notice()).toHaveCount(0);
});
