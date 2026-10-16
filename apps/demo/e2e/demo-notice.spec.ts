import { expect, test } from "@playwright/test";
import { openDemo, WORLD_ID } from "./support";

/**
 * Spec 100 US6 (SC-007): on a phone the demo notice is a pill no taller than
 * 48px, a tap opens its full text and links, and another closes it. From
 * `sm` up it reads in full, as it always has.
 */

test("on a phone the notice is a pill that opens and closes on tap", async ({
  browser,
  baseURL,
}) => {
  const { page } = await openDemo(browser, baseURL);
  try {
    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto(`/demo/world/${WORLD_ID}`);

    const notice = page.getByTestId("demo-notice");
    const toggle = notice.getByTestId("demo-notice-toggle");
    const startOver = notice.getByRole("button", { name: "Start over" });

    await expect(toggle).toBeVisible();
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
    await expect(startOver).toBeHidden();
    const collapsed = await notice.boundingBox();
    expect(
      collapsed!.height,
      "the pill is at most 48px tall",
    ).toBeLessThanOrEqual(48);

    await toggle.click();
    await expect(toggle).toHaveAttribute("aria-expanded", "true");
    await expect(startOver).toBeVisible();
    await expect(notice.getByTestId("demo-viewer")).toBeVisible();
    await expect(notice.getByRole("link", { name: "MBRound18" })).toBeVisible();
    // Open, the chip stays where it was tapped and the card hangs below it,
    // so the same thumb closes it (the owner's T017 try-out).
    const toggleBox = await toggle.boundingBox();
    const openBox = await notice.boundingBox();
    expect(toggleBox!.y - openBox!.y).toBeLessThanOrEqual(4);
    const switchBox = await notice
      .getByTestId("demo-viewer")
      .getByRole("button")
      .boundingBox();
    expect(
      switchBox!.height,
      "the view switch is thumb-sized",
    ).toBeGreaterThanOrEqual(44);

    await toggle.click();
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
    await expect(startOver).toBeHidden();
  } finally {
    await page.context().close();
  }
});

test("from 640px up the notice reads in full, with no pill", async ({
  browser,
  baseURL,
}) => {
  const { page } = await openDemo(browser, baseURL);
  try {
    await page.goto(`/demo/world/${WORLD_ID}`);
    const notice = page.getByTestId("demo-notice");
    await expect(
      notice.getByRole("button", { name: "Start over" }),
    ).toBeVisible();
    await expect(notice.getByTestId("demo-notice-toggle")).toBeHidden();
  } finally {
    await page.context().close();
  }
});
