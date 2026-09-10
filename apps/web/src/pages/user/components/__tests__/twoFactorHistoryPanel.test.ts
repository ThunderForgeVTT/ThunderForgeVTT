import { describe, expect, it } from "vitest";
import {
  HISTORY_WORDING,
  describeHistoryEntry,
} from "@/pages/user/components/TwoFactorHistoryPanel";

/**
 * Spec 041 FR-015. The seven event types the server declares, written down
 * here so a new one added in Rust without wording here fails a test rather
 * than reaching somebody's security page as a raw identifier.
 *
 * Mirrors `two_factor::events::event_type` — the same mirroring the SQL CHECK
 * constraint does, and for the same reason: three copies of a vocabulary drift
 * unless at least one of them complains.
 */
const SERVER_EVENT_TYPES = [
  "enrolled",
  "removed",
  "recovery_code_used",
  "recovery_codes_issued",
  "reset_by_operator",
  "requirement_set",
  "requirement_cleared",
];

describe("second-factor history wording", () => {
  it("has wording for every event type the server can write", () => {
    for (const eventType of SERVER_EVENT_TYPES) {
      expect(HISTORY_WORDING[eventType], eventType).toBeDefined();
    }
    expect(Object.keys(HISTORY_WORDING).sort()).toEqual(
      [...SERVER_EVENT_TYPES].sort(),
    );
  });

  /**
   * The case that matters most on a page somebody opens because they are
   * worried: an operator reset must not read as something they did.
   */
  it("says plainly when somebody else did it", () => {
    const mine = describeHistoryEntry({
      occurredAt: "2026-09-09T00:00:00",
      eventType: "reset_by_operator",
      bySomeoneElse: false,
    });
    const theirs = describeHistoryEntry({
      occurredAt: "2026-09-09T00:00:00",
      eventType: "reset_by_operator",
      bySomeoneElse: true,
    });
    expect(theirs).toMatch(/administrator/i);
    expect(theirs).not.toEqual(mine);
  });

  /**
   * A server newer than this build must not produce a blank line. It happened,
   * and a list that silently drops what it does not recognise is incomplete in
   * exactly the direction that matters.
   */
  it("still says something about an event type it does not know", () => {
    const text = describeHistoryEntry({
      occurredAt: "2026-09-09T00:00:00",
      eventType: "something_new",
      bySomeoneElse: true,
    });
    expect(text).toContain("something_new");
    expect(text.length).toBeGreaterThan(10);
  });
});
