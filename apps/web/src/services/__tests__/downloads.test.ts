import { afterEach, describe, expect, it, vi } from "vitest";
import { forgetFeatureFlags } from "@/api/featureFlags";
import { downloadSettings } from "@/services/downloads";

vi.mock("@/api/graphqlClient", () => ({
  postGraphQL: vi.fn(async () => ({
    featureFlags: [{ key: "feature.download_in_parts", on: false }],
  })),
}));

afterEach(() => {
  forgetFeatureFlags();
  delete (globalThis as { __thunderforgeDownloadSettings?: unknown })
    .__thunderforgeDownloadSettings;
});

describe("downloadSettings (spec 080)", () => {
  it("reads as on before the flags have answered", () => {
    expect(downloadSettings().enabled).toBe(true);
  });

  it("is off when the instance switched parts off", async () => {
    const { loadFeatureFlags } = await import("@/api/featureFlags");
    await loadFeatureFlags(null);
    expect(downloadSettings().enabled).toBe(false);
  });

  it("takes a test's numbers in a dev build", () => {
    (
      globalThis as { __thunderforgeDownloadSettings?: unknown }
    ).__thunderforgeDownloadSettings = { threshold: 1024, partSize: 512 };
    expect(downloadSettings()).toMatchObject({
      threshold: 1024,
      partSize: 512,
      enabled: true,
    });
  });
});
