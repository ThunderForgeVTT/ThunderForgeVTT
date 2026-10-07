import { afterEach, describe, expect, it, vi } from "vitest";

const downloadBytes = vi.fn();
vi.mock("@/services/downloads", () => ({ downloadBytes }));

const { preloadScene } = await import("@/services/scenePreload");

afterEach(() => downloadBytes.mockReset());

describe("preloadScene (spec 031, through spec 080's downloader)", () => {
  it("has nothing to warm without a background", async () => {
    expect(await preloadScene({ backgroundUrl: null })).toEqual({
      warmed: false,
      reason: "no-background",
    });
    expect(downloadBytes).not.toHaveBeenCalled();
  });

  it("warms the background into the browser cache", async () => {
    downloadBytes.mockResolvedValue(new Uint8Array(4));
    expect(await preloadScene({ backgroundUrl: "/assets/scene/1" })).toEqual({
      warmed: true,
    });
    expect(downloadBytes).toHaveBeenCalledWith("/assets/scene/1", {
      init: { cache: "force-cache", credentials: "same-origin" },
    });
  });

  it("says it failed, quietly", async () => {
    downloadBytes.mockRejectedValue(new Error("403"));
    expect(await preloadScene({ backgroundUrl: "/assets/scene/1" })).toEqual({
      warmed: false,
      reason: "failed",
    });
  });
});
