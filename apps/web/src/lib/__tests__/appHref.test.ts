import { afterEach, describe, expect, it, vi } from "vitest";
import { appHref } from "../appHref";

afterEach(() => {
  vi.unstubAllEnvs();
});

describe("appHref", () => {
  it("leaves a path alone where the app is served from the root", () => {
    vi.stubEnv("BASE_URL", "/");
    expect(appHref("/world/w/lore/e")).toBe("/world/w/lore/e");
  });

  it("puts the demo's base in front of a path", () => {
    vi.stubEnv("BASE_URL", "/demo/");
    expect(appHref("/world/w/actor/a/view")).toBe("/demo/world/w/actor/a/view");
  });
});
