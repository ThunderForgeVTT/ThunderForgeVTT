import { useEffect, useSyncExternalStore } from "react";
import {
  currentFeatureFlags,
  loadFeatureFlags,
  subscribeToFeatureFlags,
} from "@/api/featureFlags";
import { useAuth } from "@/hooks/useAuth";

/**
 * Whether this instance has a feature switched on (spec 068 FR-013).
 *
 * The one way a component decides whether to show something a flag guards.
 * It reads as off until the server has answered, and for a flag this build
 * has never heard of.
 */
export function useFeatureFlag(key: string): boolean {
  const { user, isLoading } = useAuth();
  const who = user?.id ?? null;

  useEffect(() => {
    if (!isLoading) void loadFeatureFlags(who);
  }, [who, isLoading]);

  const flags = useSyncExternalStore(
    subscribeToFeatureFlags,
    currentFeatureFlags,
  );
  return flags[key] ?? false;
}
