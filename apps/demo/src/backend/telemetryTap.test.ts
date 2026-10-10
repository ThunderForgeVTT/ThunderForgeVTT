/**
 * Spec 086 US1: the backend's tap counts each accepted action by kind and
 * reaches the funnel's `token_moved` and `dice_rolled`, with no content.
 */
import { readFileSync } from "node:fs";
import { afterAll, beforeAll, beforeEach, describe, expect, it } from "vitest";
import type { Attrs, FunnelStep } from "@thunderforge/telemetry";
import { loadDiceForTest, seedDice } from "./handlers/dice";
import { reportNotInDemo } from "./notInDemo";
import { demoState } from "./state";
import {
  actionOf,
  refusedRootField,
  setTapSink,
  type TapSink,
} from "./telemetryTap";
import { freshWorld, must } from "./testing/world";

const sent: { name: string; attrs: Attrs }[] = [];
const steps: FunnelStep[] = [];
const sink: TapSink = {
  event: (name, attrs) => sent.push({ name, attrs }),
  funnel: (step) => steps.push(step),
};
let previous: TapSink;

beforeAll(() => {
  previous = setTapSink(sink);
  loadDiceForTest(
    readFileSync(
      new URL("../../../../dist/dice/dice_bg.wasm", import.meta.url),
    ),
  );
});
afterAll(() => setTapSink(previous));

beforeEach(async () => {
  await freshWorld();
  sent.length = 0;
  steps.length = 0;
});

const actions = () =>
  sent.filter((s) => s.name === "demo.action").map((s) => s.attrs.action);

describe("the telemetry tap", () => {
  it("hears nothing from the seed, so a first visit has not moved a token", async () => {
    await freshWorld();
    expect(steps).toEqual([]);
    expect(sent).toEqual([]);
  });

  it("counts a token move and reaches token_moved, with no identifier", async () => {
    const token = demoState().tokens[0];
    await must(
      `mutation ($id: UUID!, $x: Float!, $y: Float!) {
        moveOwnToken(tokenId: $id, x: $x, y: $y) { tokenId }
      }`,
      { id: token.tokenId, x: 321, y: 654 },
    );
    expect(actions()).toEqual(["token_moved"]);
    expect(steps).toEqual(["token_moved"]);
    expect(Object.keys(sent[0].attrs)).toEqual(["action"]);
  });

  it("counts a roll and reaches dice_rolled", async () => {
    seedDice([3]);
    await must(
      `mutation ($input: RollDiceInput!) { rollDice(input: $input) { formula } }`,
      { input: { worldId: demoState().world.id, formula: "1d20" } },
    );
    expect(actions()).toEqual(["dice_rolled"]);
    expect(steps).toEqual(["dice_rolled"]);
  });

  it("names each kind of change, and only changes the funnel counts", () => {
    expect(actionOf(14, { action: "updated" })).toBe("token_moved");
    expect(actionOf(34, {})).toBe("token_moved");
    expect(actionOf(14, { action: "created", reason: "party_arrived" })).toBe(
      null,
    );
    expect(actionOf(14, { action: "deleted" })).toBe(null);
    expect(actionOf(10, { action: "created" })).toBe("wall_drawn");
    expect(actionOf(10, { action: "updated" })).toBe(null);
    expect(actionOf(21, {})).toBe("door_toggled");
    expect(actionOf(11, { action: "created" })).toBe("light_placed");
    expect(actionOf(12, { action: "created" })).toBe("shape_drawn");
    expect(actionOf(36, {})).toBe("dice_rolled");
    expect(actionOf(16, {})).toBe("scene_changed");
    expect(actionOf(13, {})).toBe("map_imported");
    expect(actionOf(17, {})).toBe(null); // chat is never counted
  });

  it("reports a refusal by its field name, and anything else as unknown", () => {
    reportNotInDemo("generateInviteCode");
    reportNotInDemo("Reaching another website");
    expect(
      sent
        .filter((s) => s.name === "demo.not_in_demo")
        .map((s) => s.attrs.root_field),
    ).toEqual(["generateInviteCode", "unknown"]);
    expect(refusedRootField("A multipart/form-data GraphQL request")).toBe(
      "unknown",
    );
  });
});
