import { expect, test, type Page } from "./fixtures/test";
import {
  graphql,
  registerAndCreateWorld,
  setWorldSystem,
  uniqueSuffix,
} from "./fixtures/helpers";
import {
  createNpc,
  importsOf,
  openImport,
  sheetFixture,
  withSheetImportOn,
} from "./fixtures/sheetImport";

/**
 * Spec 048 User Story 2: what is refused, and that a refusal writes
 * nothing. The browser reads the file first and says why in the sentence the
 * server would use, with its code, before anything is uploaded. Every file
 * is generated: the 5e pack's tests make the protected one and the book
 * page, and the oversized one is made here.
 */

test.describe.configure({ mode: "serial" });
withSheetImportOn();

/** The 10 MB limit, plus one byte. */
const OVERSIZED = Buffer.concat([
  Buffer.from("%PDF-1.4\n"),
  Buffer.alloc(10 * 1024 * 1024 + 1 - 9, 0x20),
]);

async function expectRefused(page: Page, code: string): Promise<void> {
  const problem = page.getByTestId("sheet-import-problem");
  await expect(problem).toHaveAttribute("data-code", code, {
    timeout: 30_000,
  });
  await expect(problem).not.toBeEmpty();
  await expect(page.getByTestId("sheet-import-fields")).toHaveCount(0);
  await expect(page.getByTestId("sheet-import-file")).toBeEnabled();
}

test.describe("Spec 048: what a sheet import refuses", () => {
  test("a protected file, an oversized file and a book page are refused before upload, and a system with no mapping offers nothing and is refused; nothing is written", async ({
    page: gm,
  }) => {
    test.setTimeout(240_000);
    const worldId = await registerAndCreateWorld(
      gm,
      `E2E Sheet Refusals ${uniqueSuffix()}`,
    );
    await setWorldSystem(gm, worldId, "dnd5e");
    const actorId = await createNpc(gm, worldId, "dnd5e");
    await openImport(gm, worldId, actorId);

    const file = gm.getByTestId("sheet-import-file");
    await file.setInputFiles(sheetFixture("password-protected.pdf"));
    await expectRefused(gm, "SHEET_ENCRYPTED");
    await expect(gm.getByTestId("sheet-import-problem")).toContainText(
      "password",
    );

    await file.setInputFiles({
      name: "huge.pdf",
      mimeType: "application/pdf",
      buffer: OVERSIZED,
    });
    await expectRefused(gm, "SHEET_TOO_LARGE");

    await file.setInputFiles(sheetFixture("not-a-ddb-sheet.pdf"));
    await expectRefused(gm, "SHEET_NOT_RECOGNISED");

    expect(await importsOf(gm, actorId)).toHaveLength(0);

    // A system that declares no mapping: no door, and the server refuses.
    // The GM is signed in already, so the second world is made directly.
    const made = await graphql<{
      data?: { createWorld?: { id: string } };
      errors?: { message: string }[];
    }>(
      gm,
      `
        mutation ($input: GraphQLCreateWorldInput!) {
          createWorld(input: $input) {
            id
          }
        }
      `,
      {
        input: {
          name: `E2E Sheet No Mapping ${uniqueSuffix()}`,
          gameSystemId: "fate_core",
        },
      },
    );
    const other = made.data?.createWorld?.id;
    if (!other) {
      throw new Error(`no second world: ${JSON.stringify(made.errors)}`);
    }
    const unmapped = await createNpc(gm, other, "fate_core");
    await gm.goto(`/world/${other}/actor/${unmapped}/view`);
    await expect(gm.getByTestId("actor-visibility-block")).toBeVisible({
      timeout: 15_000,
    });
    await expect(gm.getByTestId("actor-bring-in-sheet")).toHaveCount(0);
    const refused = await graphql<{
      errors?: { extensions?: { code?: string } }[];
    }>(
      gm,
      `
        query ($actorId: UUID!, $reading: JSON!) {
          sheetImportPreview(actorId: $actorId, reading: $reading) {
            planHash
          }
        }
      `,
      { actorId: unmapped, reading: {} },
    );
    expect(refused.errors?.[0]?.extensions?.code).toBe("SYSTEM_HAS_NO_MAPPING");
    expect(await importsOf(gm, unmapped)).toHaveLength(0);
  });
});
