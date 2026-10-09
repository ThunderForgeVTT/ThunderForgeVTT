import { expect, test } from "./fixtures/test";
import { openAdminPage } from "./fixtures/admin";
import { freshCredentials, graphql, register } from "./fixtures/helpers";

/**
 * Spec 086 US8, FR-035.4: an administrator can see where this instance's
 * telemetry goes, and nobody else can.
 *
 * The test stack runs with `TELEMETRY=false` (tests send nothing), so the
 * panel reads `off` on both rows. That is the state this file can assert
 * without a collector; the other two states' wording is compared with
 * Appendix A.4 by `TelemetryPanel.test.tsx`.
 */

const TELEMETRY_STATUS = `query TelemetryStatus {
  telemetryStatus { enabled serverTier browserTier instanceId }
}`;

test("an administrator sees Telemetry off, both rows and the install id", async ({
  browser,
}) => {
  const admin = await openAdminPage(browser);
  try {
    await admin.goto("/admin/readiness");
    const card = admin.locator("#telemetry");
    await expect(card.getByRole("heading", { name: "Telemetry" })).toBeVisible({
      timeout: 20_000,
    });
    await expect(card.getByTestId("telemetry-summary")).toHaveText(
      "Telemetry is off. Nothing is sent anywhere.",
    );
    await expect(card.getByRole("rowheader", { name: "Server" })).toBeVisible();
    await expect(
      card.getByRole("rowheader", { name: "Browsers" }),
    ).toBeVisible();
    await expect(card.getByTestId("telemetry-install-id")).toHaveText(
      /^Install id: [0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/,
    );
    // Read-only: the change is made in the environment, not here.
    await expect(card.locator("input, button, select")).toHaveCount(0);
  } finally {
    await admin.context().close();
  }
});

test("a non-administrator cannot read it", async ({ page }) => {
  await register(page, freshCredentials("e2etelemetry"));

  const result = await graphql<{
    data?: { telemetryStatus?: unknown } | null;
    errors?: { message: string }[];
  }>(page, TELEMETRY_STATUS, {});
  expect(result.errors?.[0]?.message).toContain("Admin privileges required");
  expect(result.data?.telemetryStatus).toBeFalsy();

  await page.goto("/admin/readiness");
  await expect(page.locator("#telemetry")).toHaveCount(0);
});
