import type { WorldMemberRole } from "@/types/world";

/**
 * Whether the person looking at a world got in on site-admin rights alone.
 *
 * The dashboard used to decide this with `world.createdBy !== user.id`, so
 * every player, trusted player and Game Master who had not created the world
 * was told they were "viewing this world through administrator access". That
 * is false for all of them, and alarming for a player who has no idea what
 * administrator access would be.
 *
 * The true condition has two halves, and both are needed. The person is a
 * site administrator, **and** they hold no role in this world. An operator
 * who is also a member of their own table is there as a member, and is told
 * nothing.
 *
 * While the roster is still loading, the role is not yet known, so the
 * answer is "no". A warning that flashes and then disappears is worse than
 * one that appears a moment late.
 */
export function reachedThroughAdminAccess({
  isAdmin,
  role,
  roleLoading,
}: {
  isAdmin: boolean;
  role: WorldMemberRole | null;
  roleLoading: boolean;
}): boolean {
  return isAdmin && !roleLoading && role === null;
}
