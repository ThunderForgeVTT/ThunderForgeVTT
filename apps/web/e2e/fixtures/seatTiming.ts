import { expect, type Page, type TestInfo } from "./test";

/**
 * Spec 046's "within one second" (SC-001, SC-002, FR-002, FR-013), asserted.
 *
 * # What is measured
 *
 * From a moment the caller names — before the change is made, or once its
 * author has seen it — to the moment the *slowest* seat shows it. Every seat
 * is watched at once rather than one after another, because a seat read only
 * after the previous seat answered is charged for that seat's wait too, and
 * the figure would grow with the number of players rather than with the
 * product.
 *
 * Each seat is watched for up to five seconds whatever the budget, so a
 * failure reports how long the change actually took, not merely that one
 * second passed.
 *
 * # The one retry (owner decision, 2026-09-15)
 *
 * A single measurement over one second is measured again, once — not the
 * whole test. The caller's `again` puts the table back so the same change can
 * be made a second time (heal what was dealt, decline what was offered), and
 * the second figure decides. Both figures are annotated, so a run that needed
 * the retry says so. A seat that never shows the change within the five
 * seconds is not retried: that is not slow, it is broken.
 */

export const ONE_SECOND_MS = 1_000;
const OBSERVE_MS = 5_000;
const INTERVAL_MS = 25;

export interface SeatClaim {
  /** The annotation's type, naming the claim. */
  what: string;
  seats: readonly (readonly [string, Page])[];
  /** Anything before the change that is not the change, off the clock. */
  prepare?: () => Promise<void>;
  /**
   * Make the change. Called once per measurement. When the change is made
   * partway through `act` — a roll confirmed inside a longer flow — return
   * the `Date.now()` of that moment and the clock starts there instead.
   */
  act: () => Promise<void | number>;
  /**
   * `before-act` starts the clock before `act` (a Game Master's click); the
   * default `after-act` starts it once `act` returns (the author has seen it).
   * Either gives way to a moment `act` returns.
   */
  clock?: "before-act" | "after-act";
  /** Whether this seat shows the change. `attempt` is 1, or 2 on the retry. */
  shown: (page: Page, attempt: number) => Promise<boolean>;
  /** What a seat shows instead, for a failure message. */
  describe?: (page: Page) => Promise<string>;
  /** Put the table back so `act` can be measured once more. */
  again: () => Promise<void>;
}

async function measure(
  claim: SeatClaim,
  attempt: number,
): Promise<{ slowest: number; perSeat: string; missing: string[] }> {
  await claim.prepare?.();

  // The seats are watched while `act` runs, not after it: an act that goes on
  // past the change (a flow read and closed, a row waited out) would
  // otherwise charge its own tail to every seat. A seat cannot show the
  // change before it is made, so watching early only makes the reading exact.
  let started = Date.now();
  let acted = false;
  const acting = claim.act().then(
    (at) => {
      if (typeof at === "number") {
        started = at;
      } else if (claim.clock !== "before-act") {
        started = Date.now();
      }
      acted = true;
    },
    (error: unknown) => {
      acted = true;
      throw error;
    },
  );
  const watching = Promise.all(
    claim.seats.map(async ([who, page]) => {
      for (;;) {
        if (await claim.shown(page, attempt).catch(() => false)) {
          return [who, Date.now()] as const;
        }
        if (acted && Date.now() - started > OBSERVE_MS) {
          return [who, null] as const;
        }
        await page.waitForTimeout(INTERVAL_MS);
      }
    }),
  );
  await acting;
  const reached = (await watching).map(
    ([who, seenAt]) =>
      [who, seenAt === null ? null : Math.max(0, seenAt - started)] as const,
  );

  const missing = reached.filter(([, ms]) => ms === null).map(([who]) => who);
  const slowest = Math.max(...reached.map(([, ms]) => ms ?? Infinity));
  const perSeat = reached
    .map(([who, ms]) => `${who} ${ms === null ? "never" : `${ms} ms`}`)
    .join(", ");
  return { slowest, perSeat, missing };
}

/**
 * Make a change and assert every seat shows it within one second, measured
 * again once if the first figure is over.
 *
 * Returns the figure that decided, in milliseconds.
 */
export async function expectOnEverySeatWithinOneSecond(
  testInfo: TestInfo,
  claim: SeatClaim,
): Promise<number> {
  for (let attempt = 1; ; attempt += 1) {
    const { slowest, perSeat, missing } = await measure(claim, attempt);
    const description =
      `${attempt === 1 ? "measured" : "re-measured"}: slowest seat ` +
      `${Number.isFinite(slowest) ? `${slowest} ms` : "never"} (${perSeat})`;
    testInfo.annotations.push({ type: claim.what, description });
    console.log(`[seat timing] ${claim.what}: ${description}`);

    if (missing.length > 0) {
      const shows = claim.describe
        ? await Promise.all(
            claim.seats
              .filter(([who]) => missing.includes(who))
              .map(
                async ([who, page]) => `${who}: ${await claim.describe!(page)}`,
              ),
          )
        : [];
      throw new Error(
        `${claim.what}: not shown within ${OBSERVE_MS} ms on ${missing.join(", ")} ` +
          `(${perSeat})${shows.length ? `; ${shows.join("; ")}` : ""}`,
      );
    }

    if (slowest <= ONE_SECOND_MS) return slowest;
    if (attempt >= 2) {
      expect(
        slowest,
        `${claim.what}: every seat within one second, measured twice (${perSeat})`,
      ).toBeLessThanOrEqual(ONE_SECOND_MS);
      return slowest;
    }
    await claim.again();
  }
}
