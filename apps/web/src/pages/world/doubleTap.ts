/**
 * Two taps of a finger, told apart from everything else a finger does.
 *
 * A double-click over the board pins a token's panel, and the board hears it
 * as the DOM's `dblclick`. A browser does not owe a touchscreen that event:
 * over a canvas that has claimed its own touches, two taps arrive as two
 * pairs of pointer events and nothing more (spec 069's touch proof found the
 * panel could not be pinned with a finger at all). So the taps are counted
 * here.
 */

/** A press lifted this soon, this close to where it landed, was a tap. */
export const TAP_MS = 300;
export const TAP_SLOP_PX = 12;
/** A second tap this soon, this close to the first, makes a pair. */
export const DOUBLE_TAP_MS = 400;
export const DOUBLE_TAP_SLOP_PX = 24;

export type Touch = { x: number; y: number; at: number };

function near(a: Touch, b: Touch, slop: number): boolean {
  return Math.hypot(a.x - b.x, a.y - b.y) <= slop;
}

export type DoubleTap = {
  down: (touch: Touch) => void;
  /** True when this lift completes a second tap. */
  up: (touch: Touch) => boolean;
};

export function createDoubleTap(): DoubleTap {
  let pressed: Touch | null = null;
  let lastTap: Touch | null = null;
  return {
    down(touch) {
      pressed = touch;
    },
    up(touch) {
      const from = pressed;
      pressed = null;
      // A hold (the long press that opens the menu) or a drag is not a tap,
      // and it breaks a pair: tap, drag, tap is not a double tap.
      if (
        !from ||
        touch.at - from.at > TAP_MS ||
        !near(from, touch, TAP_SLOP_PX)
      ) {
        lastTap = null;
        return false;
      }
      const first = lastTap;
      if (
        first &&
        touch.at - first.at <= DOUBLE_TAP_MS &&
        near(first, touch, DOUBLE_TAP_SLOP_PX)
      ) {
        // A third tap starts over rather than pairing with the second.
        lastTap = null;
        return true;
      }
      lastTap = touch;
      return false;
    },
  };
}
