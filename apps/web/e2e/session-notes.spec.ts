import { test, expect, type Page } from "@playwright/test";

/**
 * specs/011-world-compendium (US3): Session Setup's "Last Session Notes"
 * panel — a single freeform per-world recap, DM/GM-editable via a
 * Markdown code editor (CodeMirror, not a plain `<textarea>`), read-only
 * for everyone else. Also confirms Session Setup's simplified shape (Play,
 * Players, Last Session Notes only — no NPC list, no Lore placeholder).
 */

/** CodeMirror renders a contenteditable `.cm-content`, not a native
 * textarea — `.fill()`/`toHaveValue()` don't apply. Click in, select all,
 * and type over it instead. */
async function fillSessionNotes(page: Page, text: string): Promise<void> {
  const editor = page
    .getByTestId("session-notes-editor")
    .locator(".cm-content");
  await editor.click();
  await page.keyboard.press("ControlOrMeta+a");
  await page.keyboard.press("Backspace");
  if (text) {
    await page.keyboard.type(text);
  }
}

/** Reads the editor's actual document content (one `.cm-line` per line),
 * distinct from the placeholder widget CodeMirror renders when empty. */
async function readSessionNotes(page: Page): Promise<string> {
  const lines = await page
    .getByTestId("session-notes-editor")
    .locator(".cm-line")
    .allTextContents();
  return lines.join("\n");
}

function uniqueSuffix(): string {
  return `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}

interface Credentials {
  username: string;
  email: string;
  password: string;
}

function freshCredentials(prefix: string): Credentials {
  const suffix = uniqueSuffix();
  const username = `${prefix}${suffix}`;
  return {
    username,
    email: `${username}@example.test`,
    password: "Sup3r-Secret-Passphrase!",
  };
}

async function register(page: Page, creds: Credentials): Promise<void> {
  await page.goto("/register");
  await page.locator("#register-username").fill(creds.username);
  await page.locator("#register-email").fill(creds.email);
  await page.locator("#register-password").fill(creds.password);
  await page.locator("#register-password-confirmation").fill(creds.password);
  await page.getByRole("button", { name: "Create account" }).click();
}

async function registerAndCreateWorld(
  page: Page,
  worldName: string,
): Promise<string> {
  await register(page, freshCredentials("e2esessnotes"));
  await page.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
  await page.locator("#world-name").fill(worldName);
  await page.getByRole("button", { name: /create world/i }).click();
  await page.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });
  const match = /\/world\/([^/]+)\/staging$/.exec(new URL(page.url()).pathname);
  if (!match) {
    throw new Error(`Could not extract world id from URL: ${page.url()}`);
  }
  return match[1];
}

test("Session Setup shows exactly Play, Players, and Last Session Notes", async ({
  page,
}) => {
  await registerAndCreateWorld(page, `E2E Session Shape ${uniqueSuffix()}`);

  await expect(page.getByTestId("play-button")).toBeVisible();
  await expect(page.getByText("Players")).toBeVisible();
  await expect(page.getByTestId("session-notes-panel")).toBeVisible();
  await expect(page.getByText("Lore — coming soon")).toHaveCount(0);
  // The authoring entry point, not the old inline form's placeholder: that
  // placeholder no longer exists anywhere, so asserting its absence here
  // would pass without testing anything (spec 031 FR-035).
  await expect(page.getByTestId("new-npc-link")).toHaveCount(0);
});

test("DM edits and saves Last Session Notes; a Player sees it read-only", async ({
  page,
  browser,
}) => {
  const worldName = `E2E Session Notes ${uniqueSuffix()}`;
  const worldId = await registerAndCreateWorld(page, worldName);

  const notesText = `We defeated the goblin ambush ${uniqueSuffix()}`;
  await fillSessionNotes(page, notesText);
  await page.getByTestId("session-notes-save-button").click();
  await expect(page.getByText("Saved.")).toBeVisible({ timeout: 10_000 });

  // Persists across reload.
  await page.reload();
  await expect.poll(() => readSessionNotes(page)).toBe(notesText);

  // A Player sees the same text, read-only (no textarea/save control).
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.goto(`/world/${worldId}`);
  await page.getByRole("button", { name: "Generate Join Link" }).click();
  const inviteInput = page.locator("input[readonly]");
  await expect(inviteInput).toBeVisible({ timeout: 10_000 });
  const inviteRelativePath = new URL(await inviteInput.inputValue()).pathname;

  const playerContext = await browser.newContext();
  const playerPage = await playerContext.newPage();
  try {
    await register(playerPage, freshCredentials("e2esessnotesplyr"));
    await playerPage.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
    await playerPage.goto(inviteRelativePath);
    await playerPage.getByRole("button", { name: "Join Campaign" }).click();
    await playerPage.waitForURL(
      new RegExp(`/world/${worldId}(/actor-select)?$`),
      { timeout: 15_000 },
    );

    await playerPage.goto(`/world/${worldId}/staging`);
    await expect(
      playerPage.getByTestId("session-notes-readonly"),
    ).toContainText(notesText);
    await expect(playerPage.getByTestId("session-notes-editor")).toHaveCount(0);
    await expect(
      playerPage.getByTestId("session-notes-save-button"),
    ).toHaveCount(0);
  } finally {
    await playerContext.close();
  }
});

test("saving an empty value is a valid save, not an error", async ({
  page,
}) => {
  await registerAndCreateWorld(
    page,
    `E2E Session Notes Empty ${uniqueSuffix()}`,
  );

  // First, save some text so there's something to clear.
  await fillSessionNotes(page, "Something to clear");
  await page.getByTestId("session-notes-save-button").click();
  await expect(page.getByText("Saved.")).toBeVisible({ timeout: 10_000 });

  // Now clear it and save again.
  await fillSessionNotes(page, "");
  await page.getByTestId("session-notes-save-button").click();
  await expect(page.getByText("Saved.")).toBeVisible({ timeout: 10_000 });

  await page.reload();
  await expect.poll(() => readSessionNotes(page)).toBe("");
});

test("spec 021: the editor shows line numbers and a working fold gutter", async ({
  page,
}) => {
  await registerAndCreateWorld(
    page,
    `E2E Session Notes Fold ${uniqueSuffix()}`,
  );

  const editor = page.getByTestId("session-notes-editor");
  await fillSessionNotes(
    page,
    "# Heading One\nSome text under it.\nMore text.",
  );

  // Line numbers are visible (basicSetup.lineNumbers, spec 021 FR-001).
  // `.cm-gutterElement` includes a hidden width-calculation spacer
  // (`visibility: hidden`, used only to size the gutter) alongside the
  // real, visible per-line markers — exclude it explicitly.
  const realLineNumbers = editor.locator(
    ".cm-gutter.cm-lineNumbers .cm-gutterElement:not([style*='visibility: hidden'])",
  );
  await expect(realLineNumbers.first()).toBeVisible();
  await expect(realLineNumbers).toHaveCount(3);

  // A fold marker is present next to the foldable heading line, and
  // clicking it actually collapses the section beneath it (spec 021
  // FR-002 — @codemirror/lang-markdown's real heading-based folding,
  // research.md R1, not just a decorative gutter).
  const foldMarker = editor.getByTitle("Unfold line").first();
  await expect(foldMarker).toHaveCount(1);
  // Playwright's default actionability check treats this marker as
  // covered/not-visible even though a real click lands on it correctly
  // (confirmed live) — force the click rather than fight that check.
  await foldMarker.click({ force: true });

  await expect(editor.locator(".cm-foldPlaceholder")).toHaveCount(1);
  await expect.poll(() => readSessionNotes(page)).toContain("Heading One");
  await expect
    .poll(() => readSessionNotes(page))
    .not.toContain("Some text under it.");

  // Unfold restores it.
  await editor.getByTitle("Fold line").first().click({ force: true });
  await expect
    .poll(() => readSessionNotes(page))
    .toContain("Some text under it.");
});
