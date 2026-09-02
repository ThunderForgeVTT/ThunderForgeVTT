/**
 * Tells a participant once that the world's chosen pack is unavailable.
 *
 * **Once**, not once per navigation (FR-018). Keying on the missing id rather
 * than firing on every render is what makes that true, and living below the
 * provider is what lets it read the resolved fallback instead of duplicating
 * the rule that produced it.
 *
 * A notice and nothing else: nothing here blocks an action, because a look
 * that failed to load has cost the world nothing it can act on.
 */
import { useEffect, useRef } from "react";
import { toast } from "sonner";

import { useAppearance } from "./appearance-context";

export function MissingPackNotice() {
  const appearance = useAppearance();
  const missing = appearance?.missing ?? null;
  // A ref rather than state: nothing renders from this, so state would buy a
  // re-render for a value only the effect below ever reads.
  const told = useRef<string | null>(null);

  useEffect(() => {
    if (missing && missing !== told.current) {
      told.current = missing;
      toast.warning(
        `This world's interface pack "${missing}" is not installed. Showing the default look.`,
      );
    }
  }, [missing]);

  return null;
}
