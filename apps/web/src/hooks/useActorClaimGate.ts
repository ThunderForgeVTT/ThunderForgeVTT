/**
 * useActorClaimGate.ts
 * Spec 017 (FR-001/FR-002/FR-003, research.md §5): redirects a non-GM
 * world member who has not yet claimed a character to the Actor Selection
 * screen. Applied both right after `joinWorld` and on any later direct
 * visit to the world (WorldDashboardPage) — `myActorClaim` is re-checked
 * every time rather than cached from join time.
 *
 * Deliberately does NOT use the `useWorldRole` roster: a member who has
 * *just* joined via `joinWorld` (the exact moment this gate matters) may
 * not be in a roster read before the join, which once left this gate
 * hanging indefinitely during verification. `getMyWorldMemberRole` hits the server directly
 * instead, and is itself the thing that decides whether to call
 * `myActorClaim` at all — that query returns `null` for the GM too (per
 * contracts/graphql-actor-claim.md), so this hook must distinguish
 * "never gated" from "gated, no claim yet" before deciding whether to
 * check.
 *
 * Spec 101 US3: a gate that let this person in is remembered in the world
 * store, so a return to the overview renders at once. The check still runs
 * on every visit, behind the page, and a lost claim still redirects.
 */

import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { getMyActorClaim } from "@/api/actorClaims";
import { getMyWorldMemberRole } from "@/api/world";
import { useAuth } from "@/hooks/useAuth";
import { useWorldChromeStore } from "@/hooks/world-chrome-context";
import {
  isWorldMemberRole,
  runsTheWorld,
  type WorldRecord,
} from "@/types/world";

export interface UseActorClaimGateResult {
  /** True once the gate has decided this page may render (either the
   * caller is exempt, or they already hold a claim). False while the
   * check is in flight, or after a redirect has been issued. */
  cleared: boolean;
  /** Cleared without a claim: the member chose to look around first. */
  lookingAround: boolean;
}

// Spec 100 FR-011: the old staging page let a member with no character see
// the world before choosing one. The overview replaced it, so the choice is
// made on Actor Selection instead and kept for this tab only: a new visit
// asks again. Storage that throws (a private window) just means asking.
const lookingAroundKey = (worldId: string) => `tf:looking-around:${worldId}`;

/** Lets the member into the world's overview before they claim a character. */
export function lookAroundFirst(worldId: string) {
  try {
    sessionStorage.setItem(lookingAroundKey(worldId), "1");
  } catch {
    // The overview will send them back to choose; nothing is lost.
  }
}

function isLookingAround(worldId: string): boolean {
  try {
    return sessionStorage.getItem(lookingAroundKey(worldId)) === "1";
  } catch {
    return false;
  }
}

export function useActorClaimGate(
  worldId: string,
  world: WorldRecord | null,
): UseActorClaimGateResult {
  const navigate = useNavigate();
  const { user } = useAuth();
  const { chrome, remember } = useWorldChromeStore();
  const remembered = chrome[worldId]?.claimCleared === true;
  const [exempt, setExempt] = useState(false);
  const [hasClaim, setHasClaim] = useState(false);
  const [lookingAround, setLookingAround] = useState(false);

  // A world's creator commonly has no `world_members` row at all (this
  // app never backfills one at `createWorld` time — see
  // `require_world_member`'s server-side fallback doc comment), so
  // `worldMember` would return a null role for a genuine Owner. Mirror
  // that same created_by fallback here, matching `useWorldRole`'s own
  // logic, rather than relying on the query alone.
  const isCreator = !!world && !!user && world.createdBy === user.id;
  // The world's id, not the record: a background refresh of the world must
  // not run the check a second time.
  const knownWorldId = world?.id;

  useEffect(() => {
    if (!worldId || !knownWorldId || !user || isCreator) {
      return;
    }

    let active = true;

    getMyWorldMemberRole(worldId, user.id)
      .then((role) => {
        if (!active) {
          return;
        }
        // Exempt only those who run the world. A Trusted Player plays a
        // character like any Player (ADR-099), so they are sent to choose one.
        if (role !== null && isWorldMemberRole(role) && runsTheWorld(role)) {
          setExempt(true);
          remember(worldId, { claimCleared: true });
          return;
        }
        return getMyActorClaim(worldId).then((claim) => {
          if (!active) {
            return;
          }
          if (claim) {
            setHasClaim(true);
            remember(worldId, { claimCleared: true });
          } else if (isLookingAround(worldId)) {
            setLookingAround(true);
          } else {
            remember(worldId, { claimCleared: false });
            navigate(`/world/${worldId}/actor-select`, { replace: true });
          }
        });
      })
      .catch(() => {
        // Fail open — a transient query error must not trap a member on
        // a redirect loop.
        if (active) {
          setExempt(true);
        }
      });

    return () => {
      active = false;
    };
  }, [worldId, knownWorldId, user, isCreator, navigate, remember]);

  return {
    cleared: isCreator || remembered || exempt || hasClaim || lookingAround,
    lookingAround: !isCreator && !exempt && !hasClaim && lookingAround,
  };
}
