import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";

/**
 * Spec 048 T043: the browser reads every fixture sheet exactly as the server
 * does. The native answers are `cargo test`'s
 * (`tests/browser.rs`, `target/sheet-fixtures/<fixture>.json`); this reads
 * the same bytes with the wasm reader and compares the two answers whole,
 * the refusals included.
 *
 * Every fixture is generated from invented characters (`tests/fixtures/gen.rs`).
 */
const here = path.dirname(fileURLToPath(import.meta.url));
const FIXTURES = path.resolve(here, "../../sheet/tests/fixtures");
const NATIVE = path.resolve(
  process.env.CARGO_TARGET_DIR ?? path.resolve(here, "../../../../../target"),
  "sheet-fixtures",
);

const sheets = fs
  .readdirSync(FIXTURES)
  .filter((file) => file.endsWith(".pdf"))
  .sort();

test("there are fixtures to read", () => {
  expect(sheets.length).toBeGreaterThanOrEqual(8);
});

for (const sheet of sheets) {
  test(`${sheet}: the browser's reading is the server's`, async ({ page }) => {
    const nativePath = path.join(NATIVE, sheet.replace(/\.pdf$/, ".json"));
    expect(
      fs.existsSync(nativePath),
      `${nativePath} is missing: run cargo test -p thunderforge-system-dnd5e-sheet --test browser`,
    ).toBe(true);
    const native = JSON.parse(fs.readFileSync(nativePath, "utf8"));
    const bytes = [...fs.readFileSync(path.join(FIXTURES, sheet))];

    await page.goto("/");
    await expect(page.getByTestId("sheet-reader-status")).toHaveText("ready", {
      timeout: 30_000,
    });
    const answer = await page.evaluate(
      (input) => window.readSheetAnswer(input),
      bytes,
    );
    expect(answer).toEqual(native);
  });
}
