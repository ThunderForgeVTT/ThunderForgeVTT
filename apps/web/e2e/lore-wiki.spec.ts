import { test, expect, type Page } from "@playwright/test";

/**
 * specs/012-lore-wiki: the world-scoped lore wiki — GFM authoring/render,
 * `[[...]]` correlation to other lore entries and to actors with
 * auto-maintained "linked from" backlinks, the entry-level ownership
 * block (FR-021: Owner-level, not DM-only, deletion), and Viewer-gated
 * editing. Mirrors the helper patterns established in
 * actor-ownership.spec.ts / world-compendium.spec.ts.
 */

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

async function extractInviteCode(page: Page): Promise<string> {
  const input = page.locator("input[readonly]").first();
  await expect(input).toBeVisible({ timeout: 10_000 });
  const url = await input.inputValue();
  const code = new URL(url).pathname.split("/").pop();
  if (!code) throw new Error(`Could not extract invite code from URL: ${url}`);
  return code;
}

async function registerAndCreateWorld(page: Page, worldName: string): Promise<string> {
  await register(page, freshCredentials("e2elore"));
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

async function currentUserId(page: Page): Promise<string> {
  const cookies = await page.context().cookies();
  const csrfToken = cookies.find((c) => c.name === "csrf_token")?.value;
  const response = await page.request.post("/api/graphql", {
    headers: csrfToken ? { "x-csrf-token": csrfToken } : {},
    data: { query: "query { me { id } }" },
  });
  const payload = (await response.json()) as { data?: { me?: { id?: string } } };
  const id = payload.data?.me?.id;
  if (!id) throw new Error("Could not resolve current user id via /api/graphql");
  return id;
}

/** Creates a lore entry from the Compendium's Lore tab and follows "View". */
async function createLoreEntry(page: Page, worldId: string, title: string): Promise<string> {
  await page.goto(`/world/${worldId}/compendium`);
  await page.getByRole("tab", { name: "Lore" }).click();
  await page.getByTestId("new-lore-entry-title-input").fill(title);
  await page.getByTestId("add-lore-entry-button").click();
  const row = page.getByTestId("lore-catalog-table").locator("tr", { hasText: title });
  await expect(row).toBeVisible({ timeout: 10_000 });
  await row.getByRole("link", { name: "View" }).click();
  await page.waitForURL(/\/world\/[^/]+\/lore\/[^/]+\/view$/, { timeout: 15_000 });
  const match = /\/lore\/([^/]+)\/view$/.exec(new URL(page.url()).pathname);
  if (!match) throw new Error(`Could not extract lore slug from URL: ${page.url()}`);
  return match[1];
}

const PNG_BASE64 =
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

/** Dispatches a synthetic `paste` ClipboardEvent on the focused lore
 * editor textarea, mirroring canvas-asset-paste.spec.ts's approach —
 * LoreMarkdownEditor's `onPaste` handler lives on the textarea itself
 * (not `document`), so the event must be dispatched there. */
async function pasteImageIntoEditor(page: Page): Promise<void> {
  await page.getByTestId("lore-markdown-editor-textarea").evaluate((el, base64) => {
    const binary = atob(base64);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) {
      bytes[i] = binary.charCodeAt(i);
    }
    const file = new File([bytes], "pasted.png", { type: "image/png" });
    const dt = new DataTransfer();
    dt.items.add(file);
    const event = new ClipboardEvent("paste", { bubbles: true, cancelable: true, clipboardData: dt });
    el.dispatchEvent(event);
  }, PNG_BASE64);
}

test.describe("US1: DM authors a lore entry with rich Markdown", () => {
  test("GFM constructs (table, task list, code block, blockquote) render correctly", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(page, `E2E Lore GFM ${uniqueSuffix()}`);
    const title = `Ancient Ruins ${uniqueSuffix()}`;
    const slug = await createLoreEntry(page, worldId, title);

    await page.goto(`/world/${worldId}/lore/${slug}/edit`);
    const markdown = [
      "# Heading One",
      "## Heading Two",
      "",
      "**bold** and *italic* and ~~strike~~ text.",
      "",
      "| Col A | Col B |",
      "|-------|-------|",
      "| 1     | 2     |",
      "",
      "- [x] done task",
      "- [ ] open task",
      "",
      "> a blockquote",
      "",
      "```js",
      "const x = 1;",
      "```",
      "",
      "See https://example.com for more.",
    ].join("\n");
    await page.getByTestId("lore-markdown-editor-textarea").fill(markdown);
    await page.getByRole("button", { name: "Save" }).click();
    await expect(page.getByText("Saved.")).toBeVisible({ timeout: 10_000 });

    await page.goto(`/world/${worldId}/lore/${slug}/view`);
    const rendered = page.getByTestId("lore-markdown-rendered");
    await expect(rendered.locator("h1")).toContainText("Heading One");
    await expect(rendered.locator("h2")).toContainText("Heading Two");
    await expect(rendered.locator("strong")).toContainText("bold");
    await expect(rendered.locator("em")).toContainText("italic");
    await expect(rendered.locator("del, s")).toContainText("strike");
    await expect(rendered.locator("table")).toBeVisible();
    await expect(rendered.locator('input[type="checkbox"]').first()).toBeChecked();
    await expect(rendered.locator('input[type="checkbox"]').nth(1)).not.toBeChecked();
    await expect(rendered.locator("blockquote")).toContainText("a blockquote");
    await expect(rendered.locator("pre code")).toContainText("const x = 1;");
    await expect(rendered.locator('a[href="https://example.com"]')).toBeVisible();
  });
});

test.describe("US2: correlate lore entries with each other and with actors", () => {
  test("[[links]] to another entry and an actor resolve, with reciprocal linked-from backlinks", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(page, `E2E Lore Correlate ${uniqueSuffix()}`);

    // Seed an actor (NPC) to link to, via the Compendium's NPC tab.
    const npcName = `Linked NPC ${uniqueSuffix()}`;
    await page.goto(`/world/${worldId}/compendium`);
    await page.getByPlaceholder("New NPC name").fill(npcName);
    await page.getByRole("button", { name: "Add NPC" }).click();
    await expect(page.getByText(npcName)).toBeVisible({ timeout: 10_000 });

    const entryBTitle = `Entry B ${uniqueSuffix()}`;
    await createLoreEntry(page, worldId, entryBTitle);

    const entryATitle = `Entry A ${uniqueSuffix()}`;
    const slugA = await createLoreEntry(page, worldId, entryATitle);
    await page.goto(`/world/${worldId}/lore/${slugA}/edit`);
    const textarea = page.getByTestId("lore-markdown-editor-textarea");
    await textarea.fill(`See [[${entryBTitle.slice(0, 6)}`);
    await expect(page.getByTestId("lore-link-autocomplete")).toBeVisible({ timeout: 10_000 });
    await page.getByText(entryBTitle, { exact: true }).click();

    await textarea.focus();
    await page.keyboard.press("End");
    await page.keyboard.type(` and [[${npcName.slice(0, 6)}`);
    await expect(page.getByTestId("lore-link-autocomplete")).toBeVisible({ timeout: 10_000 });
    await page.getByText(npcName, { exact: true }).click();

    const currentValue = await textarea.inputValue();
    expect(currentValue).toContain(`[[${entryBTitle}]]`);
    expect(currentValue).toContain(`[[${npcName}]]`);

    // Also add an unresolved link to prove FR-007's broken-link rendering.
    await textarea.focus();
    await page.keyboard.press("End");
    await page.keyboard.type(" and [[Totally Nonexistent Title]]");

    await page.getByRole("button", { name: "Save" }).click();
    await expect(page.getByText("Saved.")).toBeVisible({ timeout: 10_000 });

    await page.goto(`/world/${worldId}/lore/${slugA}/view`);
    const rendered = page.getByTestId("lore-markdown-rendered");
    await expect(rendered.getByText(entryBTitle)).toBeVisible();
    await expect(rendered.getByText(npcName)).toBeVisible();
    await expect(rendered.locator(".lore-link-broken")).toContainText("Totally Nonexistent Title");

    // Backlink appears on Entry B's detail page.
    await page.goto(`/world/${worldId}/compendium`);
    await page.getByRole("tab", { name: "Lore" }).click();
    await page.getByTestId("lore-catalog-table").locator("tr", { hasText: entryBTitle }).getByRole("link", { name: "View" }).click();
    await expect(page.getByTestId("lore-linked-from")).toContainText(entryATitle, { timeout: 10_000 });

    // Backlink appears on the actor's detail page too.
    await page.goto(`/world/${worldId}/compendium`);
    await page.getByTestId("npc-catalog-table").getByText(npcName).click();
    await page.getByTestId("actor-preview-panel-view").click();
    await expect(page.getByTestId("actor-lore-linked-from")).toContainText(entryATitle, {
      timeout: 10_000,
    });
  });

  // Spec 013 US3 (T038-T042): the same [[...]] correlation extended to
  // Items, which had resolver-level coverage
  // (deleting_an_item_nulls_referencing_lore_links_instead_of_blocking in
  // mutations_items.rs) but no browser-level check that a lore entry
  // actually resolves an [[Item Name]] link and that the item's own
  // "Linked from (lore)" section (ItemDetailPage.tsx) shows it.
  test("[[links]] to an Item resolve, with a reciprocal linked-from backlink on the item's page", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(page, `E2E Lore Item Correlate ${uniqueSuffix()}`);

    const itemName = `Amulet of Linking ${uniqueSuffix()}`;
    await page.goto(`/world/${worldId}/compendium`);
    await page.getByRole("tab", { name: "Items" }).click();
    await page.getByTestId("new-item-name-input").fill(itemName);
    await page.getByTestId("add-item-button").click();
    await expect(page.getByTestId("item-catalog-table")).toContainText(itemName, { timeout: 10_000 });

    const entryTitle = `Entry Linking an Item ${uniqueSuffix()}`;
    const slug = await createLoreEntry(page, worldId, entryTitle);
    await page.goto(`/world/${worldId}/lore/${slug}/edit`);
    const textarea = page.getByTestId("lore-markdown-editor-textarea");
    await textarea.fill(`Forged from [[${itemName.slice(0, 6)}`);
    await expect(page.getByTestId("lore-link-autocomplete")).toBeVisible({ timeout: 10_000 });
    await page.getByText(itemName, { exact: true }).click();
    expect(await textarea.inputValue()).toContain(`[[${itemName}]]`);

    await page.getByRole("button", { name: "Save" }).click();
    await expect(page.getByText("Saved.")).toBeVisible({ timeout: 10_000 });

    await page.goto(`/world/${worldId}/lore/${slug}/view`);
    await expect(page.getByTestId("lore-markdown-rendered").getByText(itemName)).toBeVisible();

    // Reciprocal backlink on the item's own detail page.
    await page.goto(`/world/${worldId}/compendium`);
    await page.getByRole("tab", { name: "Items" }).click();
    await page.getByTestId("item-catalog-table").getByText(itemName).click();
    await page.getByTestId("item-preview-panel-view").click();
    await page.waitForURL(/\/item\/[^/]+\/view$/, { timeout: 15_000 });
    await expect(page.getByTestId("item-lore-linked-from")).toContainText(entryTitle, {
      timeout: 10_000,
    });
  });
});

test.describe("US3: paste an image into the editor", () => {
  test("pasted image uploads and renders inline", async ({ page }) => {
    const worldId = await registerAndCreateWorld(page, `E2E Lore Image ${uniqueSuffix()}`);
    const title = `Illustrated Entry ${uniqueSuffix()}`;
    const slug = await createLoreEntry(page, worldId, title);

    await page.goto(`/world/${worldId}/lore/${slug}/edit`);
    await page.getByTestId("lore-markdown-editor-textarea").click();
    await pasteImageIntoEditor(page);

    await expect(async () => {
      const value = await page.getByTestId("lore-markdown-editor-textarea").inputValue();
      expect(value).toMatch(/!\[.*\]\(.*\)/);
    }).toPass({ timeout: 10_000 });

    await page.getByRole("button", { name: "Save" }).click();
    await expect(page.getByText("Saved.")).toBeVisible({ timeout: 10_000 });

    await page.goto(`/world/${worldId}/lore/${slug}/view`);
    await expect(page.getByTestId("lore-markdown-rendered").locator("img")).toBeVisible({
      timeout: 10_000,
    });
  });
});

test.describe("US4: shareable urlified URL", () => {
  test("URL uses a readable slug and updates when the title changes", async ({ page }) => {
    const worldId = await registerAndCreateWorld(page, `E2E Lore Slug ${uniqueSuffix()}`);
    const title = `The Sunken Spire ${uniqueSuffix()}`;
    const slug = await createLoreEntry(page, worldId, title);
    expect(slug).not.toMatch(/^[0-9a-f-]{36}$/); // not a raw UUID
    expect(slug).toContain("sunken-spire");

    await page.goto(`/world/${worldId}/lore/${slug}/edit`);
    await page.locator("#lore-entry-title").fill(`Renamed Spire ${uniqueSuffix()}`);
    await page.getByRole("button", { name: "Save" }).click();
    await expect(page).toHaveURL(/\/lore\/renamed-spire[^/]*\/edit$/, { timeout: 10_000 });
  });
});

test.describe("FR-021: entry-level Owner (not DM-only) can delete; Viewer cannot edit", () => {
  test("DM grants a player Owner via the ownership block; that player deletes the entry", async ({
    page,
    browser,
  }) => {
    const worldId = await registerAndCreateWorld(page, `E2E Lore Ownership ${uniqueSuffix()}`);
    const title = `Delegable Entry ${uniqueSuffix()}`;
    const slug = await createLoreEntry(page, worldId, title);

    await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
    await page.goto(`/world/${worldId}`);
    await page.getByRole("button", { name: "Generate Join Link" }).click();
    const inviteCode = await extractInviteCode(page);

    const playerContext = await browser.newContext();
    const playerPage = await playerContext.newPage();
    try {
      await register(playerPage, freshCredentials("e2eloreowner"));
      await playerPage.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
      await playerPage.locator("#world-name").fill(`E2E Lore Player World ${uniqueSuffix()}`);
      await playerPage.getByRole("button", { name: /create world/i }).click();
      await playerPage.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });
      const playerId = await currentUserId(playerPage);

      await playerPage.goto(`/join/${inviteCode}`);
      await playerPage.getByRole("button", { name: "Join Campaign" }).click();
      await playerPage.waitForURL(new RegExp(`/world/${worldId}$`), { timeout: 15_000 });

      // Before any grant: player has default Viewer — no Edit link in the
      // catalog, and hitting /edit directly redirects to /view.
      await playerPage.goto(`/world/${worldId}/lore/${slug}/view`);
      await expect(playerPage.getByRole("button", { name: "Edit" })).toHaveCount(0);
      await playerPage.goto(`/world/${worldId}/lore/${slug}/edit`);
      await expect(playerPage).toHaveURL(new RegExp(`/lore/${slug}/view$`), { timeout: 10_000 });

      // DM grants the player Owner via the ownership block.
      await page.goto(`/world/${worldId}/lore/${slug}/edit`);
      await expect(page.getByTestId("lore-ownership-block")).toBeVisible({ timeout: 10_000 });
      const ownershipRow = page.getByTestId(`lore-ownership-row-${playerId}`);
      try {
        await expect(ownershipRow).toBeVisible({ timeout: 15_000 });
      } catch {
        await page.reload();
        await expect(page.getByTestId("lore-ownership-block")).toBeVisible({ timeout: 10_000 });
        await expect(ownershipRow).toBeVisible({ timeout: 15_000 });
      }
      await page.getByTestId(`lore-ownership-select-${playerId}`).selectOption("OWNER");
      await expect(page.getByTestId(`lore-ownership-select-${playerId}`)).toHaveValue("OWNER");

      // Player now holds entry-level Owner (not DM) and can delete it.
      await playerPage.goto(`/world/${worldId}/lore/${slug}/edit`);
      await expect(playerPage).toHaveURL(new RegExp(`/lore/${slug}/edit$`));
      await expect(playerPage.getByTestId("lore-ownership-block")).toHaveCount(0); // non-DM never sees the block, even as entry Owner
      await playerPage.getByRole("button", { name: "Delete" }).click();
      await playerPage.waitForURL(new RegExp(`/world/${worldId}/compendium$`), { timeout: 10_000 });
      await playerPage.getByRole("tab", { name: "Lore" }).click();
      await expect(playerPage.getByText(title)).toHaveCount(0);
    } finally {
      await playerContext.close();
    }
  });
});
