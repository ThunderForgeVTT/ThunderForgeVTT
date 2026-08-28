import { describe, expect, it } from "vitest";
import {
  offlineEditVerdict,
  type OfflineEditKind,
} from "../tokenControl";

/**
 * What may be changed while disconnected (spec 028 FR-035a, T074).
 *
 * The rule is narrow on purpose and the reason is not storage: the outbox
 * would queue a deletion perfectly well. It is that `conflict::resolve`
 * decides *precedence*, which settles two edits to the same position — the
 * loser's value is unused and nothing is lost the user cannot see and redo —
 * and cannot settle a deletion racing an edit, where the choices are to
 * destroy work someone was still doing or to resurrect something someone
 * deliberately removed.
 */
describe("offlineEditVerdict", () => {
  it("permits exactly position, rotation and scale", () => {
    expect(offlineEditVerdict("move").permitted).toBe(true);
    expect(offlineEditVerdict("rotate").permitted).toBe(true);
    expect(offlineEditVerdict("scale").permitted).toBe(true);
  });

  it("refuses creation and deletion", () => {
    expect(offlineEditVerdict("create").permitted).toBe(false);
    expect(offlineEditVerdict("delete").permitted).toBe(false);
  });

  /**
   * Art is not position, rotation or scale, and it carries a second problem:
   * it points at an asset that may not exist server-side when the change
   * replays.
   */
  it("refuses art changes", () => {
    expect(offlineEditVerdict("setArt").permitted).toBe(false);
  });

  /**
   * FR-035a says "refused **with a clear explanation**". A refusal with no
   * reason is indistinguishable from the application being broken, and the
   * user's next move is to try again rather than to reconnect.
   */
  it("explains every refusal, and does not explain what it allows", () => {
    const kinds: OfflineEditKind[] = [
      "move",
      "rotate",
      "scale",
      "setArt",
      "create",
      "delete",
    ];

    for (const kind of kinds) {
      const verdict = offlineEditVerdict(kind);
      if (verdict.permitted) {
        expect(verdict.explanation, `${kind} is allowed and needs no excuse`).toBeUndefined();
        continue;
      }
      expect(verdict.explanation, `${kind} must say why`).toBeTruthy();
      expect(verdict.explanation!.length, `${kind}'s reason must be a sentence`)
        .toBeGreaterThan(30);
    }
  });

  /**
   * Every refusal should leave the user knowing what they *can* still do, or
   * what to do next. A message that only says no invites them to sit and
   * wait, when in fact the table can keep playing.
   */
  it("tells the user what still works, or what to do about it", () => {
    for (const kind of ["setArt", "create", "delete"] as OfflineEditKind[]) {
      const explanation = offlineEditVerdict(kind).explanation ?? "";
      expect(
        /still work|Reconnect|reconnect/.test(explanation),
        `${kind}'s refusal should point somewhere: ${explanation}`,
      ).toBe(true);
    }
  });
});
