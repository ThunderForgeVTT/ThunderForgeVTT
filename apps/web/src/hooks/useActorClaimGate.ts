/**
 * useActorClaimGate.ts
 * Spec 017 (FR-001/FR-002/FR-003, research.md §5): redirects a non-GM
 * world member who has not yet claimed a character to the Actor Selection
 * screen. Applied both right after `joinWorld` and on any later direct
 * visit to the world (WorldDashboardPage) — `myActorClaim` is re-checked
 * every time rather than cached from join time.
 *
 * Deliberately does NOT use the RxDB-backed `useWorldRole` hook: a member
 * who has *just* joined via `joinWorld` (the exact moment this gate
 * matters) cannot assume RxDB's world-member collection has replicated
 * their own brand-new row yet, which left this gate hanging indefinitely
 * during verification. `getMyWorldMemberRole` hits the server directly
 * instead, and is itself the thing that decides whether to call
 * `myActorClaim` at all — that query returns `null` for the GM too (per
 * contracts/graphql-actor-claim.md), so this hook must distinguish
 * "never gated" from "gated, no claim yet" before deciding whether to
 * check.
 */

import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { getMyActorClaim } from "@/api/actorClaims";
import { getMyWorldMemberRole } from "@/api/world";
import { useAuth } from "@/hooks/useAuth";
import type { WorldRecord } from "@/types/world";

export interface UseActorClaimGateResult {
  /** True once the gate has decided this page may render (either the
   * caller is exempt, or they already hold a claim). False while the
   * check is in flight, or after a redirect has been issued. */
  cleared: boolean;
}

export function useActorClaimGate(
  worldId: string,
  world: WorldRecord | null,
): UseActorClaimGateResult {
  const navigate = useNavigate();
  const { user } = useAuth();
  const [exempt, setExempt] = useState(false);
  const [hasClaim, setHasClaim] = useState(false);

  // A world's creator commonly has no `world_members` row at all (this
  // app never backfills one at `createWorld` time — see
  // `require_world_member`'s server-side fallback doc comment), so
  // `worldMember` would return a null role for a genuine Owner. Mirror
  // that same created_by fallback here, matching `useWorldRole`'s own
  // logic, rather than relying on the query alone.
  const isCreator = !!world && !!user && world.createdBy === user.id;

  useEffect(() => {
    if (!worldId || !world || !user || isCreator) {
      return;
    }

    let active = true;

    getMyWorldMemberRole(worldId, user.id)
      .then((role) => {
        if (!active) {
          return;
        }
        if (role === "Owner" || role === "GM") {
          setExempt(true);
          return;
        }
        return getMyActorClaim(worldId).then((claim) => {
          if (!active) {
            return;
          }
          if (claim) {
            setHasClaim(true);
          } else {
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
  }, [worldId, world, user, isCreator, navigate]);

  return { cleared: isCreator || exempt || hasClaim };
}
