import { describe, expect, it } from "vitest";
import type { ActorImageRecord } from "@/api/actors";
import {
  imageForRole,
  portraitOf,
  tokenImageOf,
} from "@/pages/world/actor/actorImagery";

function image(role: string): ActorImageRecord {
  return {
    id: `image-${role}`,
    actorId: "actor-1",
    role,
    assetId: `asset-${role}`,
    url: `/api/actor-assets/asset-${role}`,
    thumbnailUrl: `/api/actor-assets/asset-${role}/thumb`,
  };
}

describe("actor imagery by role", () => {
  it("keeps a portrait and a token apart", () => {
    const images = [image("portrait"), image("token")];
    expect(portraitOf(images)?.assetId).toBe("asset-portrait");
    expect(tokenImageOf(images)?.assetId).toBe("asset-token");
  });

  it("reports a missing role rather than substituting another", () => {
    const images = [image("token")];
    expect(portraitOf(images)).toBeNull();
    expect(tokenImageOf(images)?.role).toBe("token");
  });

  it("ignores a role it does not recognise (ADR-057)", () => {
    const images = [image("background")];
    expect(portraitOf(images)).toBeNull();
    expect(tokenImageOf(images)).toBeNull();
    expect(imageForRole(images, "background")?.role).toBe("background");
  });

  it("handles an actor whose imagery has not loaded yet", () => {
    expect(portraitOf(null)).toBeNull();
    expect(tokenImageOf(undefined)).toBeNull();
    expect(imageForRole([], "portrait")).toBeNull();
  });
});
