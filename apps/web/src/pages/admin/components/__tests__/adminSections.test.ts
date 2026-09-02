import { describe, expect, it } from "vitest";
import {
  ADMIN_SECTIONS,
  adminSectionForPath,
} from "@/pages/admin/components/adminSections";

/**
 * Spec 031 FR-032. The nav is only useful if every admin screen can point
 * at itself, so these check the two ways that quietly stops being true: a
 * section whose path nothing routes to, and a match rule loose enough to
 * highlight Overview everywhere.
 */
describe("admin sections", () => {
  it("covers every admin route the app mounts", () => {
    expect(ADMIN_SECTIONS.map((section) => section.to)).toEqual([
      "/admin",
      "/admin/configuration",
      "/admin/storage",
      "/admin/security",
      "/admin/moderation",
    ]);
  });

  it("gives each section its own test id and label", () => {
    const testIds = new Set(ADMIN_SECTIONS.map((section) => section.testId));
    const labels = new Set(ADMIN_SECTIONS.map((section) => section.label));
    expect(testIds.size).toBe(ADMIN_SECTIONS.length);
    expect(labels.size).toBe(ADMIN_SECTIONS.length);
  });

  it("resolves a path to exactly the section being shown", () => {
    expect(adminSectionForPath("/admin/storage")?.label).toBe("Storage");
    expect(adminSectionForPath("/admin")?.label).toBe("Overview");
  });

  it("does not treat /admin as the active section on every admin page", () => {
    // The failure a `startsWith` match would produce: two links lit at
    // once, and the rail no longer saying where you are.
    expect(adminSectionForPath("/admin/moderation")?.label).toBe("Moderation");
  });

  it("has no section for a path outside the admin area", () => {
    expect(adminSectionForPath("/worlds")).toBeNull();
    expect(adminSectionForPath("/admin/settings")).toBeNull();
  });
});
