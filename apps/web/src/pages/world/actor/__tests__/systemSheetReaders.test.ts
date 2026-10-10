import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import {
  loadSheetReader,
  type SheetAnswer,
  type SheetReaderModule,
} from "@/pages/world/actor/systemSheetReaders";

/**
 * Spec 048 T032: the host finds a pack's sheet reader by discovery, and the
 * reader it finds is the real wasm, answering the shape the import page reads.
 *
 * Names systems because a test has to name something to assert it;
 * `check-system-registry` exempts tests for that reason.
 */
const ROOT = fileURLToPath(new URL("../../../../../../../", import.meta.url));

async function dnd5eReader(): Promise<SheetReaderModule> {
  const pack =
    (await import("../../../../../../../packs/systems/dnd5e/web/src/index.ts")) as {
      sheetReader: () => Promise<unknown>;
    };
  const reader = (await pack.sheetReader()) as SheetReaderModule;
  // In the browser the initialiser fetches the wasm; here it is handed it.
  await reader.default({
    module_or_path: readFileSync(`${ROOT}dist/sheet-dnd5e/sheet_bg.wasm`),
  });
  return reader;
}

function fixture(name: string): Uint8Array {
  return new Uint8Array(
    readFileSync(`${ROOT}packs/systems/dnd5e/sheet/tests/fixtures/${name}`),
  );
}

describe("loadSheetReader", () => {
  it("is null for no system, an unknown one, and a pack that ships no reader", async () => {
    expect(await loadSheetReader(null)).toBeNull();
    expect(await loadSheetReader("no-such-system")).toBeNull();
    expect(await loadSheetReader("genie")).toBeNull();
  });
});

describe("the dnd5e reader in the browser build", () => {
  it("reads a D&D Beyond sheet", async () => {
    const reader = await dnd5eReader();
    const answer = JSON.parse(
      reader.readSheet(fixture("fighter-5.pdf")),
    ) as SheetAnswer;
    expect(answer.recognised).toBe(true);
    expect(answer.error).toBeUndefined();
    expect(answer.reading).toMatchObject({
      reader: { id: "ddb-pdf" },
    });
  });

  it("answers, rather than throws, for a PDF that is not one", async () => {
    const reader = await dnd5eReader();
    const answer = JSON.parse(
      reader.readSheet(fixture("not-a-ddb-sheet.pdf")),
    ) as SheetAnswer;
    expect(answer).toEqual({
      recognised: false,
      code: "SHEET_NOT_RECOGNISED",
      error: "This does not look like a D&D Beyond character sheet.",
    });
  });
});
