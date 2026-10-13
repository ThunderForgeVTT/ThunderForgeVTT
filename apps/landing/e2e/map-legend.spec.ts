import { expect, test, type Locator, type Page } from "@playwright/test";
import { expectClean, openLanding } from "./support/page.ts";

/**
 * Spec 090 T008: the map legend. The atlas picks the map, the legend lifts
 * each layer off it, and the maps are credited to their author.
 *
 * What the overlay should hold is read from the same generated catalogue
 * the page fetches (`/maps/index.json` and `/maps/<name>.json`), so the
 * expectations follow the maps rather than a hand-kept copy of them.
 */

interface Entry {
  name: string;
  title: string;
}
interface Geometry {
  width: number;
  height: number;
  gridSize: number;
  walls: Wall[];
  lights: unknown[];
}
interface Wall {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  doorState: string;
}
interface Expected {
  walls: number;
  doors: number;
  lights: number;
  grid: number;
}

type Layer = keyof Expected;
const LAYERS: { layer: Layer; label: string }[] = [
  { layer: "walls", label: "Walls" },
  { layer: "doors", label: "Doors" },
  { layer: "lights", label: "Lights" },
  { layer: "grid", label: "Grid" },
];

async function catalogue(page: Page): Promise<Entry[]> {
  const res = await page.request.get("/maps/index.json");
  expect(res.ok()).toBe(true);
  const list = (await res.json()) as Entry[];
  expect(list.length).toBeGreaterThan(1);
  return list;
}

async function expected(page: Page, name: string): Promise<Expected> {
  const res = await page.request.get(`/maps/${name}.json`);
  expect(res.ok()).toBe(true);
  const g = (await res.json()) as Geometry;
  const doors = g.walls.filter((w) => w.doorState !== "none").length;
  return {
    walls: g.walls.length - doors,
    doors,
    lights: g.lights.length,
    grid: Math.ceil(g.width / g.gridSize) + Math.ceil(g.height / g.gridSize),
  };
}

const board = (page: Page) => page.locator(".map-frame > svg");
const tiles = (page: Page) => page.locator("ul.atlas-list button");

/** What each layer draws, counted the way the board draws it. */
const marks = (page: Page): Record<Layer, Locator> => ({
  walls: board(page).locator(".map-walls line"),
  doors: board(page).locator(".map-doors line"),
  lights: board(page).locator(".map-lights > g"),
  grid: board(page).locator(".map-grid line"),
});
const group: Record<Layer, string> = {
  walls: ".map-walls",
  doors: ".map-doors",
  lights: ".map-lights",
  grid: ".map-grid",
};

/** The board shows `entry`: its art, its name, and its geometry, every layer on. */
async function expectShowing(page: Page, entry: Entry, want: Expected): Promise<void> {
  const svg = board(page);
  await expect(svg).toHaveAttribute("aria-label", new RegExp(`^${escape(entry.title)},`));
  await expect(svg.locator("image").last()).toHaveAttribute("href", `/maps/${entry.name}.webp`);
  await expect(page.locator(".map-frame figcaption strong")).toHaveText(entry.title);
  // The geometry arrives after the art; every layer is drawn once it has.
  for (const { layer } of LAYERS) {
    await expect(svg.locator(group[layer])).toHaveCount(1);
    await expect(marks(page)[layer]).toHaveCount(want[layer]);
  }
}

/**
 * The first wall of `name` sits where the map has it. The catalogue is in
 * world units, +y up; the board's own units are SVG's, +y down. So the
 * wall's middle (x, y) belongs at (x, -y) in the board's units, and the
 * board's screen matrix says where that is on the page.
 */
async function expectInPlace(page: Page, name: string): Promise<void> {
  const res = await page.request.get(`/maps/${name}.json`);
  const g = (await res.json()) as Geometry;
  const wall = g.walls[0];
  if (!wall) return;
  const layer = wall.doorState === "none" ? ".map-walls" : ".map-doors";
  const at = await board(page)
    .locator(`${layer} line`)
    .first()
    .evaluate((line, w) => {
      const svg = (line as SVGLineElement).ownerSVGElement!;
      const p = new DOMPoint((w.x1 + w.x2) / 2, -(w.y1 + w.y2) / 2).matrixTransform(svg.getScreenCTM()!);
      const box = line.getBoundingClientRect();
      return { want: { x: p.x, y: p.y }, got: { x: box.x + box.width / 2, y: box.y + box.height / 2 } };
    }, wall);
  expect(Math.abs(at.got.x - at.want.x), `${name} x`).toBeLessThan(2);
  expect(Math.abs(at.got.y - at.want.y), `${name} y`).toBeLessThan(2);
}

/** Only the tile at `index` is pressed. */
async function expectPicked(page: Page, index: number): Promise<void> {
  const all = tiles(page);
  const n = await all.count();
  for (let i = 0; i < n; i++) {
    await expect(all.nth(i)).toHaveAttribute("aria-pressed", i === index ? "true" : "false");
  }
}

const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

test.describe("the atlas", () => {
  test("picking each map shows that map and its geometry, and presses only its tile", async ({
    page,
  }) => {
    const opened = await openLanding(page);
    const list = await catalogue(page);
    await expect(tiles(page)).toHaveCount(list.length);
    await expect(page.locator(".atlas-title")).toHaveText(new RegExp(`^${list.length} maps`));

    // The first map is up before anything is picked.
    await expectPicked(page, 0);
    await expectShowing(page, list[0], await expected(page, list[0].name));
    await expectInPlace(page, list[0].name);

    // Every other map, then back to the first.
    const order = [...list.keys()].slice(1).concat(0);
    for (const i of order) {
      const tile = tiles(page).nth(i);
      await expect(tile).toContainText(list[i].title);
      await tile.click();
      await expectPicked(page, i);
      await expectShowing(page, list[i], await expected(page, list[i].name));
      await expectInPlace(page, list[i].name);
    }
    expectClean(opened);
  });

  test("a tile off the edge of the strip can be picked from the keyboard", async ({ page }) => {
    const opened = await openLanding(page);
    const list = await catalogue(page);
    const last = list.length - 1;
    await expectNarrowStripScrolls(page, last);

    // Into the strip at the first tile, then along it with Tab.
    await tiles(page).first().focus();
    await expect(tiles(page).first()).toBeFocused();
    for (let i = 1; i <= last; i++) {
      await page.keyboard.press("Tab");
      await expect(tiles(page).nth(i)).toBeFocused();
      // Every stop along the way is brought into view, not left past the edge.
      await expectShownInStrip(tiles(page).nth(i));
    }
    await page.keyboard.press("Enter");
    await expectPicked(page, last);
    await expectShowing(page, list[last], await expected(page, list[last].name));

    // Back one, and Space picks it too.
    await page.keyboard.press("Shift+Tab");
    await expect(tiles(page).nth(last - 1)).toBeFocused();
    await page.keyboard.press("Space");
    await expectPicked(page, last - 1);
    await expectShowing(page, list[last - 1], await expected(page, list[last - 1].name));
    expectClean(opened);
  });

  test("a tile off the edge of the strip can be picked by pointer", async ({ page }, info) => {
    const opened = await openLanding(page);
    const list = await catalogue(page);
    const last = list.length - 1;
    await expectNarrowStripScrolls(page, last);

    const tile = tiles(page).nth(last);
    if (info.project.use.hasTouch) await tile.tap();
    else await tile.click();
    await expectShownInStrip(tile);
    await expectPicked(page, last);
    await expectShowing(page, list[last], await expected(page, list[last].name));
    expectClean(opened);
  });
});

/**
 * The tile is wholly on screen: inside the strip's visible box and inside
 * the page's viewport. A few pixels of slack cover the hand-dealt tilt,
 * whose rotated corners poke past the box the tile is laid out in.
 */
async function expectShownInStrip(tile: Locator): Promise<void> {
  await expect
    .poll(() =>
      tile.evaluate((el) => {
        const SLACK = 4;
        const b = el.getBoundingClientRect();
        const s = el.closest("ul")!.getBoundingClientRect();
        const left = Math.max(s.left, 0);
        const right = Math.min(s.right, window.innerWidth);
        const ok =
          b.left >= left - SLACK &&
          b.right <= right + SLACK &&
          b.top >= -SLACK &&
          b.bottom <= window.innerHeight + SLACK;
        return ok
          ? "shown"
          : `tile ${Math.round(b.left)}-${Math.round(b.right)} x ${Math.round(b.top)}-${Math.round(b.bottom)}, ` +
              `strip shows ${Math.round(left)}-${Math.round(right)} in ${window.innerWidth}x${window.innerHeight}`;
      }),
    )
    .toBe("shown");
}

/**
 * Below the desktop layout the atlas is a sideways strip: prove it is one,
 * and that the tile under test starts out past its edge, so picking it means
 * reaching it. At desktop width every tile is laid out on the page.
 */
async function expectNarrowStripScrolls(page: Page, index: number): Promise<void> {
  const strip = page.locator("ul.atlas-list");
  await strip.scrollIntoViewIfNeeded();
  const narrow = (page.viewportSize()?.width ?? 0) < 1024;
  const shape = await strip.evaluate((el) => ({
    overflowX: getComputedStyle(el).overflowX,
    overflows: el.scrollWidth > el.clientWidth + 1,
  }));
  if (narrow) {
    expect(shape.overflowX).toBe("auto");
    expect(shape.overflows).toBe(true);
    await expect(tiles(page).nth(index)).not.toBeInViewport();
  } else {
    expect(shape.overflows).toBe(false);
    await expect(tiles(page).nth(index)).toBeInViewport();
  }
}

test.describe("the legend", () => {
  test("each layer toggle lifts its layer off the map and puts it back", async ({ page }) => {
    const opened = await openLanding(page);
    const list = await catalogue(page);
    // The first map has every layer in it, so lifting one is visible.
    const want = await expected(page, list[0].name);
    for (const { layer } of LAYERS) expect(want[layer], layer).toBeGreaterThan(0);
    await expectShowing(page, list[0], want);

    const toggles = page.locator("button.legend-toggle");
    await expect(toggles).toHaveCount(LAYERS.length);

    for (const [i, { layer, label }] of LAYERS.entries()) {
      const toggle = toggles.nth(i);
      await expect(toggle).toContainText(label);
      await expect(toggle).toHaveAttribute("aria-pressed", "true");

      await toggle.click();
      await expect(toggle).toHaveAttribute("aria-pressed", "false");
      await expect(board(page).locator(group[layer])).toHaveCount(0);
      // Only this layer went.
      for (const other of LAYERS) {
        if (other.layer === layer) continue;
        await expect(marks(page)[other.layer]).toHaveCount(want[other.layer]);
      }
      // The map itself stays.
      await expect(board(page).locator("image").last()).toHaveAttribute(
        "href",
        `/maps/${list[0].name}.webp`,
      );

      await toggle.click();
      await expect(toggle).toHaveAttribute("aria-pressed", "true");
      await expect(marks(page)[layer]).toHaveCount(want[layer]);
    }
    expectClean(opened);
  });

  test("a lifted layer stays lifted on the next map, and comes back on it", async ({ page }) => {
    const opened = await openLanding(page);
    const list = await catalogue(page);
    const next = 1;
    const want = await expected(page, list[next].name);
    const walls = page.locator("button.legend-toggle").first();

    await walls.click();
    await expect(walls).toHaveAttribute("aria-pressed", "false");
    await tiles(page).nth(next).click();
    await expectPicked(page, next);
    await expect(marks(page).doors).toHaveCount(want.doors);
    await expect(board(page).locator(".map-walls")).toHaveCount(0);

    await walls.click();
    await expect(walls).toHaveAttribute("aria-pressed", "true");
    await expect(marks(page).walls).toHaveCount(want.walls);
    expectClean(opened);
  });
});

test("the maps are credited to MBRound18, under CC BY-SA 4.0, with the catalog linked", async ({
  page,
}) => {
  const opened = await openLanding(page);
  const credit = page.locator(".map-frame figcaption");
  await credit.scrollIntoViewIfNeeded();
  await expect(credit).toBeVisible();
  await expect(credit).toContainText("Maps by MBRound18");
  await expect(credit).toContainText("CC BY-SA 4.0");

  const source = credit.getByRole("link", { name: "vtt-maps", exact: true });
  await expect(source).toBeVisible();
  await expect(source).toHaveAttribute("href", "https://github.com/mbround18/vtt-maps");

  const catalog = credit.getByRole("link", { name: "catalog", exact: true });
  await expect(catalog).toBeVisible();
  await expect(catalog).toHaveAttribute("href", "https://vtt-maps.dnd-apps.dev/catalog");

  const licence = credit.getByRole("link", { name: "CC BY-SA 4.0", exact: true });
  await expect(licence).toBeVisible();
  await expect(licence).toHaveAttribute(
    "href",
    "https://creativecommons.org/licenses/by-sa/4.0/",
  );
  expectClean(opened);
});
