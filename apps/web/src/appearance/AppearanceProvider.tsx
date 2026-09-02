/**
 * Applies a world's chosen look, to the page and to the canvas.
 *
 * # Why custom properties rather than a stylesheet
 *
 * The application is already themed entirely through the CSS custom properties
 * in `styles/globals.css`, so applying a pack is writing values onto
 * `document.documentElement`. Nothing is fetched, nothing is injected, and
 * nothing reloads — which is most of why SC-001's thirty seconds has so much
 * headroom.
 *
 * It is also why a pack cannot hide a control. A stylesheet could position one
 * off-screen or set `pointer-events: none`; a named colour token cannot
 * (FR-012).
 *
 * # Why this mounts inside the world layout
 *
 * The binding is per world. A user with two worlds open must not see one
 * world's look leak into the other, and mounting at the app root would make
 * that the default rather than the bug.
 */
import { useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";

import { getInterfacePack, type InterfaceManifest } from "@/api/interfacePacks";
import { useTheme } from "@/hooks/theme-context";

import {
  AppearanceContext,
  BASE_PACK_ID,
  customPropertyName,
  resolveAppearance,
  type ResolvedAppearance,
} from "./appearance-context";

interface AppearanceProviderProps {
  /** The world's binding. `null` or absent means the base pack. */
  interfacePackId?: string | null;
  /**
   * Bumped when the world announces an appearance change, so the pack is
   * re-fetched without a reload (SC-001).
   */
  revision?: number;
  children: ReactNode;
}

const EMPTY: ResolvedAppearance = {
  packId: BASE_PACK_ID,
  missing: null,
  light: {},
  dark: {},
  canvas: null,
  layout: [],
  loading: true,
};

export function AppearanceProvider({
  interfacePackId,
  revision = 0,
  children,
}: AppearanceProviderProps) {
  const { theme } = useTheme();
  const [resolved, setResolved] = useState<ResolvedAppearance>(EMPTY);

  const requested = interfacePackId?.trim() ? interfacePackId.trim() : null;

  useEffect(() => {
    let isMounted = true;

    const load = async () => {
      let base: InterfaceManifest;
      try {
        base = await getInterfacePack(BASE_PACK_ID);
      } catch {
        // Without the base pack there is nothing to fall back to. The page
        // keeps whatever the stylesheet already gives it, which is the same
        // look — Forge is that stylesheet written down.
        if (isMounted) setResolved({ ...EMPTY, loading: false });
        return;
      }

      let chosen: InterfaceManifest | null = null;
      if (requested !== null && requested !== base.id) {
        try {
          chosen = await getInterfacePack(requested);
        } catch {
          // Missing or no longer valid. Fall back and say so once; block
          // nothing (FR-018).
          chosen = null;
        }
      }

      if (isMounted) {
        setResolved(resolveAppearance(base, chosen, requested));
      }
    };

    void load();
    return () => {
      isMounted = false;
    };
  }, [requested, revision]);

  // The reader keeps their own brightness; the world keeps its pack. A Game
  // Master picking a look is not picking a time of day for six other people's
  // rooms, and this is the accessibility escape hatch that survived making the
  // look table-wide.
  const tokens = theme === "dark" ? resolved.dark : resolved.light;

  useEffect(() => {
    const root = document.documentElement;
    const applied = Object.entries(tokens);
    for (const [token, value] of applied) {
      root.style.setProperty(customPropertyName(token), value);
    }
    return () => {
      // Removing rather than restoring: what is left is whatever
      // `globals.css` declares, which is the base look. Restoring a captured
      // previous value would reinstate the *last pack's* colours when this
      // provider unmounts, which is worse than reverting to the default.
      for (const [token] of applied) {
        root.style.removeProperty(customPropertyName(token));
      }
    };
  }, [tokens]);

  // The canvas half. Sent on resolve and on every change, so the engine's
  // status bars are drawn in the same pack as the chrome around them.
  useEffect(() => {
    if (resolved.loading || !resolved.canvas) {
      return;
    }
    void import("@/engine/bevy").then(({ setDisplayAppearance }) =>
      setDisplayAppearance(resolved.canvas),
    );
  }, [resolved.canvas, resolved.loading]);

  const value = useMemo(() => resolved, [resolved]);

  return (
    <AppearanceContext.Provider value={value}>
      {children}
    </AppearanceContext.Provider>
  );
}
