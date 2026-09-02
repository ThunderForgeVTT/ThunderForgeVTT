import { describe, expect, it } from "vitest";
import {
  placeAuthoredProp,
  type PlacementCalls,
  type PropDraft,
} from "../placeAuthoredProp";

/**
 * Spec 031 FR-011, at the seam that cannot be reached from a server.
 *
 * The claims worth making here are about ordering and partial failure: the
 * prop has to exist before anything can point at it, and the call that points
 * at it can fail on its own. A real server refuses both or neither.
 */

const LORE_DRAFT: PropDraft = {
  effectId: "lore.open",
  effectConfig: { entry: "entry-1" },
  activation: "anyone",
  fireMode: "always",
};

function calls(overrides: Partial<PlacementCalls> = {}): {
  calls: PlacementCalls;
  seen: string[];
  inputs: unknown[];
} {
  const seen: string[] = [];
  const inputs: unknown[] = [];
  return {
    seen,
    inputs,
    calls: {
      placeProp: async (sceneId, x, y) => {
        seen.push(`placeProp:${sceneId}:${x}:${y}`);
        return { tokenId: "token-1" };
      },
      createInteractive: async (input) => {
        seen.push("createInteractive");
        inputs.push(input);
        return { interactiveId: "interactive-1" };
      },
      ...overrides,
    },
  };
}

describe("placeAuthoredProp", () => {
  it("creates the prop where it was dropped, then points an interactive at it", async () => {
    const { calls: injected, seen, inputs } = calls();

    const outcome = await placeAuthoredProp(
      injected,
      "scene-1",
      { x: 64, y: -32 },
      LORE_DRAFT,
    );

    expect(outcome).toEqual({
      kind: "placed",
      tokenId: "token-1",
      interactiveId: "interactive-1",
    });
    // Order, not merely both: an interactive created first would have nothing
    // to point at.
    expect(seen).toEqual(["placeProp:scene-1:64:-32", "createInteractive"]);
    expect(inputs[0]).toMatchObject({
      subjectKind: "prop",
      subjectRef: "token-1",
      effectId: "lore.open",
      trigger: "click",
    });
  });

  it("still authors an interactive for scenery, so the prop is findable", async () => {
    const { calls: injected, inputs } = calls();

    await placeAuthoredProp(
      injected,
      "scene-1",
      { x: 0, y: 0 },
      { ...LORE_DRAFT, effectId: null, effectConfig: null },
    );

    expect(inputs[0]).toMatchObject({ effectId: null, subjectKind: "prop" });
  });

  it("says nothing was placed when the prop itself was refused", async () => {
    const { calls: injected, seen } = calls({
      placeProp: async () => {
        throw new Error("no");
      },
    });

    const outcome = await placeAuthoredProp(
      injected,
      "scene-1",
      { x: 0, y: 0 },
      LORE_DRAFT,
    );

    expect(outcome.kind).toBe("refused");
    // Nothing was attempted afterwards: there is no subject to attach to.
    expect(seen).toEqual([]);
  });

  it("admits the prop exists when only its effect was refused", async () => {
    const { calls: injected } = calls({
      createInteractive: async () => {
        throw new Error("no");
      },
    });

    const outcome = await placeAuthoredProp(
      injected,
      "scene-1",
      { x: 0, y: 0 },
      LORE_DRAFT,
    );

    // The half-done state is reported as itself. Reporting a plain failure
    // would leave a token on the map the Game Master was told did not happen.
    expect(outcome.kind).toBe("propOnly");
    expect(outcome.kind === "propOnly" && outcome.tokenId).toBe("token-1");
  });
});
