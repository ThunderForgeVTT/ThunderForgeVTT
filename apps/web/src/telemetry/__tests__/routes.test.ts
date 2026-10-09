import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { ROUTE_TEMPLATES, UNKNOWN_ROUTE, routeTemplate } from "../routes";

const here = path.dirname(fileURLToPath(import.meta.url));

describe("routeTemplate", () => {
  it("reports a template, never the ids in the path", () => {
    const id = "6f1c2a9e-0b7d-4d1e-9a5b-3c2d1e0f9a8b";
    expect(routeTemplate(`/world/${id}/play`)).toBe("/world/:id/play");
    expect(routeTemplate(`/world/${id}/lore/the-crypt/edit`)).toBe(
      "/world/:id/lore/:slug/edit",
    );
    expect(routeTemplate("/invite/ABC123")).toBe("/invite/:code");
    expect(routeTemplate("/join/xyz")).toBe("/join/:code");
  });

  it("prefers a static segment over a parameter", () => {
    expect(routeTemplate("/worlds/create")).toBe("/worlds/create");
    expect(routeTemplate("/setup/callback")).toBe("/setup/callback");
    expect(routeTemplate("/world/w1/compendium/npc/new")).toBe(
      "/world/:id/compendium/npc/new",
    );
  });

  it("names the root and the unknown", () => {
    expect(routeTemplate("/")).toBe("/");
    expect(routeTemplate("/no/such/page/anywhere")).toBe(UNKNOWN_ROUTE);
  });

  it("holds every path the router declares", () => {
    const source = readFileSync(
      path.resolve(here, "../../routes/AppRoutes.tsx"),
      "utf8",
    );
    const declared = [...source.matchAll(/path="([^"]+)"/g)]
      .map((m) => m[1])
      .filter((p) => p !== "*");
    const known = new Set<string>(ROUTE_TEMPLATES);
    expect(declared.filter((p) => !known.has(p))).toEqual([]);
  });
});
