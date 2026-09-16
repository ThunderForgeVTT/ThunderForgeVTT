import { useEffect, useState } from "react";
import { getWorldMembers } from "@/api/worldMembers";

/**
 * How many people are in each of these worlds.
 *
 * # Why a batch hook and not `useWorldMembers` per row
 *
 * The world archive's table wants one number per world, and `worldMembers` is
 * the only query that knows it — there is no "members of my worlds" query and
 * this change is not allowed to invent one. A `useWorldMembers` call inside
 * each row would work and would also mean: a fresh roster fetch per row on
 * every mount, so flipping to tiles and back re-asks for all of them, and a
 * person with thirty worlds opening thirty rosters at once.
 *
 * So the reads are shared. A module-level cache survives the flip (a member
 * count is not something that changes while you look at a list), and the
 * fetches go out in small batches rather than all at once, because thirty
 * simultaneous requests is how a list page becomes the reason a server is
 * slow.
 *
 * # The cache is the state
 *
 * Render reads the cache directly and the effect only ever bumps a counter,
 * from inside an async continuation, to say "there is more in it now". The
 * tempting shape — mirroring the cache into component state and seeding that
 * state at the top of the effect — is a `setState` in an effect body, which
 * is a cascading render and which this codebase's lint refuses by rule.
 *
 * # What `null` means
 *
 * Not known — either still arriving, or refused. Both render as an em dash
 * rather than a `0`, because "this world has no members" and "I could not
 * find out" are different facts and only one of them is ever true here: the
 * caller is a member of every world it lists, so the true count is never 0.
 */

/** Counts already known, keyed by world id. Lives past a component. */
const COUNT_CACHE = new Map<string, number>();

/** How many rosters to ask for at once. */
const BATCH_SIZE = 6;

export function useWorldMemberCounts(
  worldIds: readonly string[],
): Record<string, number | null> {
  const [, setArrivals] = useState(0);

  // The ids as one string: the array identity changes on every render of the
  // page above, and depending on it directly would re-run this forever.
  const key = worldIds.join(",");

  useEffect(() => {
    let active = true;
    const ids = key ? key.split(",") : [];
    const missing = ids.filter((id) => !COUNT_CACHE.has(id));
    if (missing.length === 0) {
      return;
    }

    void (async () => {
      for (let start = 0; start < missing.length; start += BATCH_SIZE) {
        if (!active) {
          return;
        }
        const batch = missing.slice(start, start + BATCH_SIZE);
        await Promise.all(
          batch.map((id) =>
            getWorldMembers(id)
              .then((members) => {
                COUNT_CACHE.set(id, members.length);
              })
              // A refusal is not a zero. Left out of the cache so a later
              // visit — perhaps after the membership row that was missing
              // has been written — asks again.
              .catch(() => undefined),
          ),
        );
        if (active) {
          setArrivals((count) => count + 1);
        }
      }
    })();

    return () => {
      active = false;
    };
  }, [key]);

  const result: Record<string, number | null> = {};
  for (const id of worldIds) {
    result[id] = COUNT_CACHE.get(id) ?? null;
  }
  return result;
}

/** Test seam: the cache is module state and outlives a test's render. */
export function clearWorldMemberCountCache(): void {
  COUNT_CACHE.clear();
}
