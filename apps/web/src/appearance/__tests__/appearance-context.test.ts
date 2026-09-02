import { describe, expect, it } from "vitest";

import type { InterfaceManifest } from "@/api/interfacePacks";
import {
  BASE_PACK_ID,
  customPropertyName,
  overlay,
  resolveAppearance,
} from "../appearance-context";

const base: InterfaceManifest = {
  id: BASE_PACK_ID,
  type: "interface",
  title: "Forge",
  version: "1.0.0",
  description: "base",
  light: { background: "#fff", foreground: "#000", accent: "#ccc" },
  dark: { background: "#000", foreground: "#fff", accent: "#333" },
  targets: [],
  layout: [{ kind: "badgeGrid", of: "attributes" }],
};

const pack = (over: Partial<InterfaceManifest>): InterfaceManifest => ({
  ...base,
  id: "forged-steel",
  title: "Forged Steel",
  ...over,
});

describe("resolving a world's appearance", () => {
  it("lets a pack change one token and inherit the rest", () => {
    const resolved = resolveAppearance(
      base,
      pack({ light: { accent: "#f00" }, dark: {} }),
      "forged-steel",
    );

    expect(resolved.light.accent).toBe("#f00");
    expect(resolved.light.background).toBe("#fff");
    expect(resolved.light.foreground).toBe("#000");
  });

  it("inherits the base layout when a pack declares none", () => {
    const resolved = resolveAppearance(
      base,
      pack({ layout: undefined }),
      "forged-steel",
    );
    expect(resolved.layout).toEqual(base.layout);
  });

  it("prefers the pack's own layout when it has one", () => {
    const own = [{ kind: "barStack", of: "resources" }];
    const resolved = resolveAppearance(
      base,
      pack({ layout: own }),
      "forged-steel",
    );
    expect(resolved.layout).toEqual(own);
  });

  /**
   * FR-018. A look that cannot load must cost nothing: the world opens, the
   * base pack applies, and the participant is told once.
   */
  it("falls back to the base pack and names what is missing", () => {
    const resolved = resolveAppearance(base, null, "a-pack-that-left");

    expect(resolved.packId).toBe(BASE_PACK_ID);
    expect(resolved.missing).toBe("a-pack-that-left");
    expect(resolved.light).toEqual(base.light);
  });

  it("reports nothing missing when no pack was asked for", () => {
    const resolved = resolveAppearance(base, null, null);
    expect(resolved.missing).toBeNull();
    expect(resolved.packId).toBe(BASE_PACK_ID);
  });

  it("keeps both palettes, because the reader picks the mode", () => {
    const resolved = resolveAppearance(
      base,
      pack({ light: { background: "#eee" }, dark: { background: "#111" } }),
      "forged-steel",
    );
    expect(resolved.light.background).toBe("#eee");
    expect(resolved.dark.background).toBe("#111");
  });
});

describe("overlay", () => {
  it("takes the later value and keeps the rest", () => {
    expect(overlay({ a: "1", b: "2" }, { b: "3" })).toEqual({ a: "1", b: "3" });
  });
});

describe("custom property names", () => {
  it("maps camelCase onto the stylesheet's kebab-case", () => {
    expect(customPropertyName("background")).toBe("--background");
    expect(customPropertyName("cardForeground")).toBe("--card-foreground");
    expect(customPropertyName("sidebarPrimaryForeground")).toBe(
      "--sidebar-primary-foreground",
    );
  });

  /**
   * The one mapping that is not purely mechanical, and the one whose failure
   * would be silent: `--chart1` is a property nothing reads, so the chart
   * colours would simply stay whatever the base pack said and no error would
   * appear anywhere.
   */
  it("separates a chart's digit, because the stylesheet writes --chart-1", () => {
    expect(customPropertyName("chart1")).toBe("--chart-1");
    expect(customPropertyName("chart5")).toBe("--chart-5");
  });
});
