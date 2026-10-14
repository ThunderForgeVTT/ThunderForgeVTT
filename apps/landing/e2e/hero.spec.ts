import { expect, test, type Locator, type Page } from "@playwright/test";
import { expectClean, openLanding } from "./support/page.ts";

/**
 * Spec 090 T006: the hero's party tokens move the way tokens move at the
 * table. Arrow keys step one square, the marker walls refuse a move whose
 * straight line crosses them, and a mouse drag snaps to the grid.
 *
 * The page lays the mat out differently per width (the room's door is in
 * the east wall when the hero is 900px or wider, in the south wall below
 * that), so nothing here hard-codes a square. The test reads the mat back
 * from the DOM, rebuilds the walls exactly as `measure()` in
 * `src/sections/Hero.tsx` does, and plans its moves from that model.
 */

const FEET_PER_SQUARE = 5;
const SUFFIX = ". Arrow keys move one square.";

type Dir = "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight";
const STEP: Record<Dir, [number, number]> = {
  ArrowUp: [0, -1],
  ArrowDown: [0, 1],
  ArrowLeft: [-1, 0],
  ArrowRight: [1, 0],
};
const OPPOSITE: Record<Dir, Dir> = {
  ArrowUp: "ArrowDown",
  ArrowDown: "ArrowUp",
  ArrowLeft: "ArrowRight",
  ArrowRight: "ArrowLeft",
};

interface Spot {
  c: number;
  r: number;
}
interface Seg {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
}
interface Piece extends Spot {
  name: string;
  size: number;
  foe: boolean;
}
interface Mat {
  cell: number;
  cols: number;
  rows: number;
  walls: Seg[];
  pieces: Piece[];
}

/** Read the mat back: the grid, the walls `measure()` draws, every token. */
async function readMat(page: Page): Promise<Mat> {
  const raw = await page.evaluate((suffix) => {
    const hero = document.querySelector<HTMLElement>("header.hero")!;
    const room = hero.querySelector<HTMLElement>(".room")!;
    const cell = parseFloat(getComputedStyle(hero).getPropertyValue("--cell")) || 48;
    const h = hero.getBoundingClientRect();
    const rm = room.getBoundingClientRect();
    const tokens = [...hero.querySelectorAll<HTMLButtonElement>("button.token")].map((b) => {
      const m = /translate\(([-\d.]+)px,\s*([-\d.]+)px\)/.exec(b.style.transform)!;
      return {
        name: b.getAttribute("aria-label")!.replace(suffix, ""),
        x: parseFloat(m[1]),
        y: parseFloat(m[2]),
        w: parseFloat(b.style.width),
        foe: b.classList.contains("token--foe"),
      };
    });
    return {
      cell,
      w: h.width,
      h: h.height,
      room: {
        left: rm.left - h.left,
        top: rm.top - h.top,
        right: rm.right - h.left,
        bottom: rm.bottom - h.top,
      },
      tokens,
    };
  }, SUFFIX);

  const { cell } = raw;
  const cols = Math.floor(raw.w / cell);
  const rows = Math.floor(raw.h / cell);
  const wide = raw.w >= 900;
  const x0 = Math.floor(raw.room.left / cell);
  const y0 = Math.floor(raw.room.top / cell);
  const x1 = Math.ceil(raw.room.right / cell);
  const y1 = Math.ceil(raw.room.bottom / cell);
  const P = (n: number) => n * cell;
  const walls: Seg[] = [];
  if (wide) {
    const d0 = Math.max(y0 + 1, y1 - 4);
    walls.push(
      { x1: P(x0), y1: P(y0), x2: P(x1), y2: P(y0) },
      { x1: P(x0), y1: P(y0), x2: P(x0), y2: P(y1) },
      { x1: P(x0), y1: P(y1), x2: P(x1), y2: P(y1) },
      { x1: P(x1), y1: P(y0), x2: P(x1), y2: P(d0) },
      { x1: P(x1), y1: P(d0 + 2), x2: P(x1), y2: P(y1) },
    );
  } else {
    const d0 = Math.min(x1 - 3, x0 + 2);
    walls.push(
      { x1: P(x0), y1: P(y0), x2: P(x1), y2: P(y0) },
      { x1: P(x0), y1: P(y0), x2: P(x0), y2: P(y1) },
      { x1: P(x1), y1: P(y0), x2: P(x1), y2: P(y1) },
      { x1: P(x0), y1: P(y1), x2: P(d0), y2: P(y1) },
      { x1: P(d0 + 2), y1: P(y1), x2: P(x1), y2: P(y1) },
    );
  }
  const pieces = raw.tokens.map((t) => {
    // Every resting token must already sit on a grid line.
    expect(t.x % cell, `${t.name} x=${t.x} is off the grid`).toBe(0);
    expect(t.y % cell, `${t.name} y=${t.y} is off the grid`).toBe(0);
    return {
      name: t.name,
      c: t.x / cell,
      r: t.y / cell,
      size: Math.round(t.w / cell),
      foe: t.foe,
    };
  });
  return { cell, cols, rows, walls, pieces };
}

function orient(ax: number, ay: number, bx: number, by: number, cx: number, cy: number) {
  return Math.sign((bx - ax) * (cy - ay) - (by - ay) * (cx - ax));
}

/** The page's own rule: the move whose centre line crosses a wall is refused. */
function verdict(mat: Mat, piece: Piece, from: Spot, to: Spot): "ok" | "wall" | "taken" | "edge" {
  const s = piece.size;
  if (to.c < 0 || to.r < 2 || to.c + s > mat.cols || to.r + s > mat.rows) return "edge";
  for (const o of mat.pieces) {
    if (o.name === piece.name) continue;
    if (to.c < o.c + o.size && o.c < to.c + s && to.r < o.r + o.size && o.r < to.r + s) {
      return "taken";
    }
  }
  const half = (s * mat.cell) / 2;
  const [ax, ay, bx, by] = [
    from.c * mat.cell + half,
    from.r * mat.cell + half,
    to.c * mat.cell + half,
    to.r * mat.cell + half,
  ];
  const crosses = mat.walls.some(
    (w) =>
      orient(ax, ay, bx, by, w.x1, w.y1) !== orient(ax, ay, bx, by, w.x2, w.y2) &&
      orient(w.x1, w.y1, w.x2, w.y2, ax, ay) !== orient(w.x1, w.y1, w.x2, w.y2, bx, by),
  );
  return crosses ? "wall" : "ok";
}

const step = (at: Spot, d: Dir): Spot => ({ c: at.c + STEP[d][0], r: at.r + STEP[d][1] });

function party(mat: Mat): Piece[] {
  return mat.pieces.filter((p) => !p.foe);
}

function token(page: Page, name: string): Locator {
  return page.getByRole("button", { name: `${name}${SUFFIX}`, exact: true });
}

const note = (page: Page) => page.locator("header.hero p.sr-only[aria-live]");

/** Where a token rests, in squares, read from its transform. */
async function squareOf(page: Page, name: string, cell: number): Promise<Spot> {
  const t = await token(page, name).evaluate((b) => (b as HTMLElement).style.transform);
  const m = /translate\(([-\d.]+)px,\s*([-\d.]+)px\)/.exec(t)!;
  return { c: parseFloat(m[1]) / cell, r: parseFloat(m[2]) / cell };
}

async function expectAt(page: Page, name: string, cell: number, at: Spot) {
  await expect(token(page, name)).toHaveCSS(
    "transform",
    `matrix(1, 0, 0, 1, ${at.c * cell}, ${at.r * cell})`,
  );
}

test("a focused party token steps one square per arrow key and says how far it went", async ({
  page,
}) => {
  const opened = await openLanding(page);
  const mat = await readMat(page);
  expect(party(mat).length).toBe(4);

  for (const d of Object.keys(STEP) as Dir[]) {
    const back = OPPOSITE[d];
    const piece = party(mat).find((p) => verdict(mat, p, p, step(p, d)) === "ok");
    expect(piece, `no party token can step ${d} on this mat`).toBeDefined();
    const from: Spot = { c: piece!.c, r: piece!.r };
    const to = step(from, d);

    const button = token(page, piece!.name);
    await button.focus();
    await expect(button).toBeFocused();

    await page.keyboard.press(d);
    await expectAt(page, piece!.name, mat.cell, to);
    await expect(note(page)).toHaveText(`${piece!.name} moved ${FEET_PER_SQUARE} feet.`);

    // And back, so the next arrow plans against the mat as it was set out.
    await page.keyboard.press(back);
    await expectAt(page, piece!.name, mat.cell, from);
    expect(await squareOf(page, piece!.name, mat.cell)).toEqual(from);
  }
  expectClean(opened);
});

test("a move into a marker wall is refused, and the note says why", async ({ page }) => {
  const opened = await openLanding(page);
  const mat = await readMat(page);

  // Breadth-first from each party token's starting square, over moves the
  // page allows, to the nearest square whose next step hits a wall.
  let plan: { piece: Piece; path: Dir[]; at: Spot; into: Dir } | null = null;
  for (const piece of party(mat)) {
    const start: Spot = { c: piece.c, r: piece.r };
    const seen = new Set([`${start.c},${start.r}`]);
    const queue: { at: Spot; path: Dir[] }[] = [{ at: start, path: [] }];
    while (queue.length && !plan) {
      const { at, path } = queue.shift()!;
      for (const d of Object.keys(STEP) as Dir[]) {
        const v = verdict(mat, piece, at, step(at, d));
        if (v === "wall") {
          plan = { piece, path, at, into: d };
          break;
        }
        const next = step(at, d);
        const key = `${next.c},${next.r}`;
        if (v === "ok" && !seen.has(key) && path.length < 12) {
          seen.add(key);
          queue.push({ at: next, path: [...path, d] });
        }
      }
    }
    if (plan) break;
  }
  expect(plan, "no party token can reach a wall on this mat").not.toBeNull();
  const { piece, path, at, into } = plan!;

  const button = token(page, piece.name);
  await button.focus();
  let here: Spot = { c: piece.c, r: piece.r };
  for (const d of path) {
    await page.keyboard.press(d);
    here = step(here, d);
    await expectAt(page, piece.name, mat.cell, here);
  }
  expect(here).toEqual(at);

  await page.keyboard.press(into);
  await expect(note(page)).toHaveText(`${piece.name} can't move there: a wall is in the way.`);
  await expectAt(page, piece.name, mat.cell, at);
  expect(await squareOf(page, piece.name, mat.cell)).toEqual(at);
  expectClean(opened);
});

test("a mouse drag lets go off the grid and the token snaps to a square", async ({ page }) => {
  const opened = await openLanding(page);
  const mat = await readMat(page);

  // A square two away that the page will accept, for any party token.
  const offsets: [number, number][] = [
    [2, 0],
    [-2, 0],
    [0, 2],
    [0, -2],
    [1, 1],
    [-1, 1],
    [1, -1],
    [-1, -1],
  ];
  let choice: { piece: Piece; to: Spot } | null = null;
  for (const piece of party(mat)) {
    for (const [dc, dr] of offsets) {
      const to = { c: piece.c + dc, r: piece.r + dr };
      if (verdict(mat, piece, piece, to) === "ok") {
        choice = { piece, to };
        break;
      }
    }
    if (choice) break;
  }
  expect(choice, "no party token has a free square two away").not.toBeNull();
  const { piece, to } = choice!;

  const button = token(page, piece.name);
  await button.scrollIntoViewIfNeeded();
  const box = (await button.boundingBox())!;
  const sx = box.x + box.width / 2;
  const sy = box.y + box.height / 2;
  // Let go 0.3 of a square past the target on both axes: off the grid, but
  // nearest to `to`.
  const ex = sx + (to.c - piece.c) * mat.cell + mat.cell * 0.3;
  const ey = sy + (to.r - piece.r) * mat.cell - mat.cell * 0.3;

  await page.mouse.move(sx, sy);
  await page.mouse.down();
  await expect(button).toHaveClass(/is-dragging/);
  await page.mouse.move(ex, ey, { steps: 8 });
  // Mid-drag the token follows the pointer, off the grid.
  const mid = await button.evaluate((b) => (b as HTMLElement).style.transform);
  const m = /translate\(([-\d.]+)px,\s*([-\d.]+)px\)/.exec(mid)!;
  expect(parseFloat(m[1]) % mat.cell).not.toBe(0);
  await page.mouse.up();

  await expect(button).not.toHaveClass(/is-dragging/);
  await expectAt(page, piece.name, mat.cell, to);
  const final = await squareOf(page, piece.name, mat.cell);
  expect(Number.isInteger(final.c) && Number.isInteger(final.r)).toBe(true);
  expect(final).toEqual(to);
  const feet = Math.max(Math.abs(to.c - piece.c), Math.abs(to.r - piece.r)) * FEET_PER_SQUARE;
  await expect(note(page)).toHaveText(`${piece.name} moved ${feet} feet.`);
  expectClean(opened);
});

test("on a wide screen the hero says there is more below, until the visitor scrolls", async ({
  page,
}, info) => {
  test.skip(info.project.name !== "desktop", "the cue shows where the hero fills the screen");
  // Settling scrolls the whole page, which is exactly what sends the cue away.
  const opened = await openLanding(page, { settle: false });
  const cue = page.getByRole("link", { name: "Scroll for more" });
  await expect(cue).toBeVisible();
  await expect(cue).toBeInViewport();

  // Following it lands on the next section, and the note is gone from then on.
  await cue.click();
  await expect(page.locator('[data-section="dream"]')).toBeInViewport();
  await expect(cue).toBeHidden();
  await page.evaluate(() => window.scrollTo(0, 0));
  await expect(cue).toBeHidden();
  expectClean(opened);
});
