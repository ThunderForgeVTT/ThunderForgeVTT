/**
 * Tells a participant once that the world's chosen pack is unavailable.
 *
 * **Once**, not once per navigation (FR-018). That is harder than it looks
 * here: `WorldSectionShell` is a thin wrapper rather than a route layout, so
 * every world route renders its own copy of this component and a ref inside it
 * resets on each navigation. `shouldTellAboutMissingPack` keeps the record in
 * module state, where a remount cannot reach it.
 *
 * Living below the provider is what lets this read the resolved fallback
 * instead of duplicating the rule that produced it.
 *
 * A notice and nothing else: nothing here blocks an action, because a look
 * that failed to load has cost the world nothing it can act on.
 */
import { useEffect } from "react";
import { toast } from "sonner";

import { useAppearance } from "./appearance-context";
import { shouldTellAboutMissingPack } from "./told-about-pack";

export function MissingPackNotice() {
  const appearance = useAppearance();
  const missing = appearance?.missing ?? null;

  useEffect(() => {
    if (shouldTellAboutMissingPack(missing)) {
      toast.warning(
        `This world's interface pack "${missing}" is not installed. Showing the default look.`,
      );
    }
  }, [missing]);

  return null;
}
