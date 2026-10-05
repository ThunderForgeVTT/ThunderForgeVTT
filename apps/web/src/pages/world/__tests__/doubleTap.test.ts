import { describe, expect, it } from "vitest";
import { createDoubleTap } from "../doubleTap";

const at = (x: number, y: number, t: number) => ({ x, y, at: t });

/** One tap at a place and time; says whether it completed a pair. */
function tap(
  taps: ReturnType<typeof createDoubleTap>,
  x: number,
  y: number,
  t: number,
  held = 60,
): boolean {
  taps.down(at(x, y, t));
  return taps.up(at(x, y, t + held));
}

describe("createDoubleTap", () => {
  it("two quick taps in one place are a double tap", () => {
    const taps = createDoubleTap();
    expect(tap(taps, 100, 100, 0)).toBe(false);
    expect(tap(taps, 104, 98, 150)).toBe(true);
  });

  it("a third tap starts a new pair", () => {
    const taps = createDoubleTap();
    tap(taps, 100, 100, 0);
    tap(taps, 100, 100, 150);
    expect(tap(taps, 100, 100, 300)).toBe(false);
    expect(tap(taps, 100, 100, 450)).toBe(true);
  });

  it("two taps too far apart in time or place are two taps", () => {
    const slow = createDoubleTap();
    tap(slow, 100, 100, 0);
    expect(tap(slow, 100, 100, 600)).toBe(false);

    const apart = createDoubleTap();
    tap(apart, 100, 100, 0);
    expect(tap(apart, 200, 100, 150)).toBe(false);
  });

  it("a held finger is not a tap, and does not pair with one", () => {
    const taps = createDoubleTap();
    tap(taps, 100, 100, 0);
    expect(tap(taps, 100, 100, 150, 600)).toBe(false);
    expect(tap(taps, 100, 100, 800)).toBe(false);
  });

  it("a drag is not a tap", () => {
    const taps = createDoubleTap();
    tap(taps, 100, 100, 0);
    taps.down(at(100, 100, 150));
    expect(taps.up(at(180, 100, 200))).toBe(false);
  });

  it("a lift with no press is nothing", () => {
    const taps = createDoubleTap();
    expect(taps.up(at(1, 1, 0))).toBe(false);
  });
});
