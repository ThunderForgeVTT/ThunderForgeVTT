import { describe, expect, it } from "vitest";

import { feedInstant } from "../feedTime";

describe("feedInstant", () => {
  it("reads a zone-less server time as UTC", () => {
    expect(feedInstant("2026-10-07T12:00:00.123456")).toBe(
      Date.UTC(2026, 9, 7, 12, 0, 0, 123),
    );
  });

  it("keeps a time that names its zone", () => {
    expect(feedInstant("2026-10-07T12:00:00+00:00")).toBe(
      Date.UTC(2026, 9, 7, 12),
    );
    expect(feedInstant("2026-10-07T14:00:00+02:00")).toBe(
      Date.UTC(2026, 9, 7, 12),
    );
    expect(feedInstant("2026-10-07T12:00:00Z")).toBe(Date.UTC(2026, 9, 7, 12));
  });
});
