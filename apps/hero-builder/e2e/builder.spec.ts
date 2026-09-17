/**
 * Spec 044 phase (a): the hero builder, alone on its page.
 *
 * Every expectation about which controls exist is read from
 * `@thunderforge/heroes` when the test runs — no count or key list is
 * written here — so a part added to the catalogue is found, selected and
 * drawn by this file with no edit (SC-002).
 */
import { readFile } from "node:fs/promises";
import { expect, test, type Page } from "@playwright/test";
import {
  HERO_COLORS,
  HERO_FLAGS,
  HERO_PALETTES,
  HERO_PARTS,
  HERO_RACES,
  PRESET_HEROES,
  raceProblems,
  renderPortrait,
  renderToken,
  resolveHero,
  SIZES,
  SKIN_TONES,
  validateHero,
  type HeroSpec,
} from "@thunderforge/heroes";

const CHOICES: readonly [string, readonly string[]][] = [
  ["size", SIZES],
  ...Object.entries(HERO_PARTS),
];

async function openBuilder(page: Page) {
  await page.goto("/");
  await expect(page.getByTestId("hero-builder")).toBeVisible();
}

/** Clicks a choice the way a person does: on its visible chip. */
async function choose(page: Page, field: string, choice: string) {
  await page
    .getByTestId(`hero-choice-${field}-${choice}`)
    .locator("xpath=..")
    .click();
  await expect(
    page.getByTestId(`hero-choice-${field}-${choice}`),
  ).toBeChecked();
}

/**
 * Clicks an export and reads the file it saves.
 *
 * Tries twice. Chromium throttles a page that downloads many files in quick
 * succession and silently drops one in about every eleven — measured
 * 2026-09-16 with the same button clicked back to back, the dropped click
 * changing nothing on the page. No person exports ten files in two seconds;
 * this suite does, so a click that saves nothing is clicked once more.
 */
async function download(page: Page, testId: string): Promise<string> {
  for (let attempt = 1; ; attempt += 1) {
    try {
      const [file] = await Promise.all([
        page.waitForEvent("download", { timeout: 3_000 }),
        page.getByTestId(testId).click(),
      ]);
      return await readFile(await file.path(), "utf8");
    } catch (error) {
      if (attempt === 2) throw error;
    }
  }
}

async function exportedSpec(page: Page): Promise<HeroSpec> {
  return JSON.parse(await download(page, "export-json")) as HeroSpec;
}

async function openPreset(page: Page, slug: string) {
  await page.getByTestId("preset-picker").selectOption(slug);
}

async function portraitMarkup(page: Page): Promise<string> {
  return page.getByTestId("hero-preview-portrait").innerHTML();
}

/** Rolls with a seed typed in, as a person re-entering a shown seed does. */
async function rollSeed(page: Page, seed: string) {
  await page.getByTestId("hero-seed").fill(seed);
  await page.getByTestId("hero-roll-seed").click();
  await expect(page.getByTestId("hero-seed")).toHaveValue(seed);
}

/** A copied preset entry, read back as the object it declares. */
function readPresetSource(source: string): { slug: string; spec: HeroSpec } {
  const body = source.trim().replace(/,$/, "");
  return new Function("SKIN_TONES", `return (${body});`)(SKIN_TONES);
}

test.describe("the hero builder", () => {
  test("part 1: one labelled, selectable group per catalogue field", async ({
    page,
  }) => {
    await openBuilder(page);
    for (const [field, choices] of CHOICES) {
      const group = page.getByTestId(`hero-field-${field}`);
      await expect(group, field).toBeVisible();
      await expect(group.locator("legend")).not.toHaveText("");
      // Every choice is found, selected and drawn: a choice that draws the
      // same as the one before it would be a choice nobody could see.
      for (const choice of choices) {
        const radio = page.getByTestId(`hero-choice-${field}-${choice}`);
        await expect(radio, `${field}.${choice}`).toHaveCount(1);
        await expect(radio.locator("xpath=..")).not.toHaveText("");
        if (await radio.isChecked()) continue;
        const before = await portraitMarkup(page);
        await choose(page, field, choice);
        await expect
          .poll(() => portraitMarkup(page), `${field}.${choice} draws`)
          .not.toBe(before);
      }
    }
    for (const field of HERO_COLORS) {
      const group = page.getByTestId(`hero-field-${field}`);
      await expect(group, field).toBeVisible();
      const swatches = group.locator('input[type="radio"]');
      await expect(swatches).toHaveCount(HERO_PALETTES[field].length);
      await expect(page.getByTestId(`hero-hex-${field}`)).toHaveValue(
        /^#[0-9a-f]{6}$/,
      );
      await page
        .getByTestId(`hero-swatch-${field}-0`)
        .locator("xpath=..")
        .click();
      await expect(page.getByTestId(`hero-swatch-${field}-0`)).toBeChecked();
    }
    for (const field of HERO_FLAGS) {
      const toggle = page.getByTestId(`hero-flag-${field}`);
      await expect(toggle, field).toHaveCount(1);
      const before = await toggle.isChecked();
      await toggle.locator("xpath=..").click();
      await expect(toggle).toBeChecked({ checked: !before });
    }
  });

  test("part 2: every change redraws both pictures within 100 ms, with no reload", async ({
    page,
  }) => {
    await openBuilder(page);
    await page.evaluate(
      () => ((window as unknown as { marker: number }).marker = 1),
    );
    const timings: number[] = [];
    for (const [field, choices] of CHOICES) {
      for (const choice of choices.slice(1, 3)) {
        const ms = await page.evaluate(
          async ({ testId }) => {
            const input = document.querySelector<HTMLInputElement>(
              `[data-testid="${testId}"]`,
            );
            const portrait = document.querySelector(
              '[data-testid="hero-preview-portrait"]',
            );
            const token = document.querySelector(
              '[data-testid="hero-preview-token"]',
            );
            if (!input || !portrait || !token)
              throw new Error(`missing ${testId}`);
            if (input.checked) return -1;
            const before = [portrait.innerHTML, token.innerHTML];
            const start = performance.now();
            input.click();
            return await new Promise<number>((resolve, reject) => {
              const check = () => {
                if (
                  portrait.innerHTML !== before[0] ||
                  token.innerHTML !== before[1]
                ) {
                  resolve(performance.now() - start);
                } else if (performance.now() - start > 1000) {
                  reject(new Error(`${testId} did not redraw`));
                } else {
                  requestAnimationFrame(check);
                }
              };
              requestAnimationFrame(check);
            });
          },
          { testId: `hero-choice-${field}-${choice}` },
        );
        if (ms >= 0) timings.push(ms);
      }
    }
    expect(timings.length).toBeGreaterThan(CHOICES.length);
    const worst = Math.max(...timings);
    const sorted = [...timings].sort((a, b) => a - b);
    console.log(
      `SC-006: ${timings.length} changes, median ${sorted[Math.floor(sorted.length / 2)]!.toFixed(1)} ms, worst ${worst.toFixed(1)} ms`,
    );
    expect(worst).toBeLessThan(100);
    expect(
      await page.evaluate(
        () => (window as unknown as { marker?: number }).marker,
      ),
    ).toBe(1);
  });

  test("part 3: every preset draws what the package writes, and survives export and import", async ({
    page,
  }) => {
    await openBuilder(page);
    for (const { slug, spec } of PRESET_HEROES) {
      await openPreset(page, slug);
      await expect(page.getByTestId("hero-text-name")).toHaveValue(spec.name);
      const portrait = await download(page, "export-portrait");
      const token = await download(page, "export-token");
      expect(portrait, slug).toBe(renderPortrait(spec));
      expect(token, slug).toBe(renderToken(spec));

      // What is on screen is that drawing, under the builder's own ids.
      const shown = await portraitMarkup(page);
      const prefix = /id="([A-Za-z][\w-]*?)-portrait-/.exec(shown)?.[1];
      const expected = renderPortrait(
        spec,
        prefix ? { idPrefix: prefix } : undefined,
      );
      const parsed = await page.evaluate((svg) => {
        const holder = document.createElement("div");
        holder.innerHTML = svg;
        return holder.innerHTML;
      }, expected);
      expect(shown, slug).toBe(parsed);

      const json = await download(page, "export-json");
      await page.getByTestId("import-text").fill(json);
      await page.getByTestId("import-apply").click();
      await expect(page.getByTestId("status")).toHaveText(
        `Imported ${spec.name}.`,
      );
      expect(
        await download(page, "export-portrait"),
        `${slug} round trip`,
      ).toBe(portrait);
      expect(await download(page, "export-token"), `${slug} round trip`).toBe(
        token,
      );
      expect(await download(page, "export-json"), `${slug} round trip`).toBe(
        json,
      );
    }
  });

  test("part 4: no duplicated id, and an invalid spec is refused whole", async ({
    page,
  }) => {
    await openBuilder(page);
    const duplicates = () =>
      page.evaluate(() => {
        const seen = new Map<string, number>();
        for (const element of document.querySelectorAll("[id]")) {
          seen.set(element.id, (seen.get(element.id) ?? 0) + 1);
        }
        return [...seen].filter(([, count]) => count > 1).map(([id]) => id);
      });
    for (const { slug } of PRESET_HEROES) {
      await openPreset(page, slug);
      expect(await duplicates(), slug).toEqual([]);
    }

    const before = await portraitMarkup(page);
    const name = await page.getByTestId("hero-text-name").inputValue();
    const [aPart] = Object.keys(HERO_PARTS);
    const [aColor] = HERO_COLORS;
    await page
      .getByTestId("import-text")
      .fill(
        JSON.stringify({
          name: "",
          [aPart!]: "no-such-choice",
          [aColor]: "red",
          race: "anything",
        }),
      );
    await page.getByTestId("import-apply").click();
    const problems = page.getByTestId("import-problems").locator("li");
    const fields = await problems.evaluateAll((items) =>
      items.map((item) => item.getAttribute("data-field")),
    );
    expect(new Set(fields)).toEqual(new Set(["name", aPart, aColor, "race"]));
    expect(await portraitMarkup(page)).toBe(before);
    await expect(page.getByTestId("hero-text-name")).toHaveValue(name);

    await page.getByTestId("import-text").fill("not json at all");
    await page.getByTestId("import-apply").click();
    await expect(problems).toHaveCount(1);
    expect(await portraitMarkup(page)).toBe(before);
  });

  test("part 5: the whole builder by keyboard, named, and usable at 375 px", async ({
    page,
  }) => {
    await openBuilder(page);
    const reached = new Set<string>();
    await page.getByTestId("preset-picker").focus();
    for (let step = 0; step < 400; step += 1) {
      await page.keyboard.press("Tab");
      const id = await page.evaluate(
        () => document.activeElement?.getAttribute("data-testid") ?? "",
      );
      if (id === "import-text") break;
      reached.add(id);
    }
    for (const id of [
      "hero-race",
      "hero-roll",
      "hero-seed",
      "hero-roll-seed",
      "hero-text-name",
      "export-json",
      "copy-preset",
    ]) {
      expect(reached, id).toContain(id);
    }
    for (const [field] of CHOICES) {
      expect(
        [...reached].some((id) => id.startsWith(`hero-choice-${field}-`)),
        field,
      ).toBe(true);
    }
    for (const field of HERO_COLORS)
      expect(reached, field).toContain(`hero-hex-${field}`);
    for (const field of HERO_FLAGS)
      expect(reached, field).toContain(`hero-flag-${field}`);

    // Arrow keys move through a part, and the hero follows.
    const [part, choices] = Object.entries(HERO_PARTS)[0]!;
    await page.getByTestId(`hero-choice-${part}-${choices[0]}`).focus();
    await page.getByTestId(`hero-choice-${part}-${choices[0]}`).press("Space");
    const before = await portraitMarkup(page);
    await page.keyboard.press("ArrowRight");
    await expect(
      page.getByTestId(`hero-choice-${part}-${choices[1]}`),
    ).toBeChecked();
    await expect.poll(() => portraitMarkup(page)).not.toBe(before);

    // The dice by keyboard.
    await page.getByTestId("hero-race").focus();
    await page.keyboard.press("ArrowDown");
    await page.getByTestId("hero-roll").focus();
    await page.keyboard.press("Enter");
    await expect(page.getByTestId("hero-seed")).toHaveValue(/^[0-9a-f]{8}$/);

    // Every choice and swatch has a name; every hex is text.
    const unnamed = await page.getByTestId("hero-builder").evaluate((root) =>
      [...root.querySelectorAll("input, select, button")]
        .filter((element) => {
          const labelled = (
            element as HTMLInputElement
          ).labels?.[0]?.textContent?.trim();
          return !(
            element.getAttribute("aria-label") ||
            labelled ||
            element.textContent?.trim()
          );
        })
        .map((element) => element.outerHTML.slice(0, 80)),
    );
    expect(unnamed).toEqual([]);
    for (const field of HERO_COLORS) {
      await expect(page.getByTestId(`hero-hex-${field}`)).toHaveValue(
        /^#[0-9a-f]{6}$/,
      );
      await expect(page.getByTestId(`hero-swatch-${field}-0`)).toHaveAttribute(
        "aria-label",
        /#[0-9a-f]{6}$/,
      );
    }

    await page.setViewportSize({ width: 375, height: 812 });
    await page.evaluate(() => window.scrollTo(0, document.body.scrollHeight));
    const overflow = await page.evaluate(
      () =>
        document.documentElement.scrollWidth -
        document.documentElement.clientWidth,
    );
    expect(overflow).toBeLessThanOrEqual(0);
    await expect(page.getByTestId("hero-preview-portrait")).toBeInViewport();
    await expect(page.getByTestId("hero-preview-token")).toBeInViewport();
    const small = await page.getByTestId("hero-builder").evaluate((root) =>
      [...root.querySelectorAll("input, select, button")]
        .map((element) => {
          const hidden =
            element instanceof HTMLInputElement &&
            (element.type === "radio" || element.type === "checkbox");
          const target = hidden
            ? (element.nextElementSibling ?? element)
            : element;
          const box = target.getBoundingClientRect();
          return {
            what:
              element.getAttribute("data-testid") ??
              element.outerHTML.slice(0, 60),
            w: box.width,
            h: box.height,
          };
        })
        .filter(({ w, h }) => w < 44 || h < 44),
    );
    expect(small).toEqual([]);
  });

  test("part 6: dice reproduce from the seed, keep a lock, and copy as a working preset", async ({
    page,
    context,
  }) => {
    await openBuilder(page);
    await rollSeed(page, "tavern-7");
    const first = await exportedSpec(page);
    await page.getByTestId("hero-roll").click();
    await rollSeed(page, "tavern-7");
    expect(await exportedSpec(page)).toEqual(first);

    const [part, choices] = Object.entries(HERO_PARTS).find(
      ([, list]) => list.length > 3,
    )!;
    const kept = choices[choices.length - 1]!;
    await choose(page, part, kept);
    await page.getByTestId(`hero-lock-${part}`).click();
    for (const seed of ["a", "b", "c", "d", "e"]) {
      await rollSeed(page, seed);
      await expect(
        page.getByTestId(`hero-choice-${part}-${kept}`),
      ).toBeChecked();
    }

    await context.grantPermissions(["clipboard-read", "clipboard-write"]);
    const spec = await exportedSpec(page);
    await page.getByTestId("copy-preset").click();
    await expect(page.getByTestId("status")).toHaveText(
      "Copied as a preset entry.",
    );
    const source = await page.evaluate(() => navigator.clipboard.readText());
    const entry = readPresetSource(source);
    expect(validateHero(entry.spec).ok).toBe(true);
    expect(renderPortrait(entry.spec)).toBe(renderPortrait(spec));
    expect(renderToken(entry.spec)).toBe(renderToken(spec));
  });

  test("part 7: the race roll narrows the look and is never saved", async ({
    page,
    context,
  }) => {
    await openBuilder(page);
    const options = await page
      .getByTestId("hero-race")
      .locator("option")
      .evaluateAll((items) =>
        items.map((item) => (item as HTMLOptionElement).value),
      );
    expect(options).toEqual(["", ...Object.keys(HERO_RACES)]);
    await expect(page.getByTestId("hero-race")).toHaveValue("");

    for (const race of Object.keys(HERO_RACES)) {
      await page.getByTestId("hero-race").selectOption(race);
      for (const seed of ["one", "two", "three"]) {
        await rollSeed(page, seed);
        const spec = await exportedSpec(page);
        expect(Object.keys(spec)).not.toContain("race");
        expect(
          raceProblems(resolveHero(spec), race),
          `${race}/${seed}`,
        ).toEqual([]);
      }
    }

    // A lock beats the race: pick a race that narrows a part, set that part
    // to something the race would not roll, and lock it.
    const [race, look] = Object.entries(HERO_RACES).find(([, entry]) =>
      Object.keys(entry.choices ?? {}).some((field) => field in HERO_PARTS),
    )!;
    const [field, allowed] = Object.entries(look.choices!).find(
      ([name]) => name in HERO_PARTS,
    )!;
    const against = (
      HERO_PARTS[field as keyof typeof HERO_PARTS] as readonly string[]
    ).find((choice) => !(allowed as readonly string[]).includes(choice))!;
    await page.getByTestId("hero-race").selectOption(race);
    await choose(page, field, against);
    await page.getByTestId(`hero-lock-${field}`).click();
    await rollSeed(page, "locked");
    await expect(
      page.getByTestId(`hero-choice-${field}-${against}`),
    ).toBeChecked();

    await context.grantPermissions(["clipboard-read", "clipboard-write"]);
    await page.getByTestId("copy-preset").click();
    await expect(page.getByTestId("status")).toHaveText(
      "Copied as a preset entry.",
    );
    const source = await page.evaluate(() => navigator.clipboard.readText());
    expect(source).not.toMatch(/\brace\b/);
    expect(await download(page, "export-json")).not.toMatch(/"race"/);
  });
});
