/**
 * Wraps a world's surfaces in the look that world has chosen.
 *
 * Owns three things the provider deliberately does not: reading the world's
 * binding, listening for it changing, and telling a participant when the pack
 * it names has gone. The provider below only resolves and applies, so it stays
 * testable without a network or a subscription.
 *
 * Mounted per world rather than at the application root. A user with two
 * worlds open must not see one world's look leak into the other, and mounting
 * at the root would make that the default rather than the bug.
 */
import { useEffect, useState } from "react";
import type { ReactNode } from "react";

import { getWorld } from "@/api/world";
import { subscribeToWorldEvents } from "@/engine/world/sync/subscriptionClient";
import { startAppearanceEventSync } from "@/engine/world/sync/appearance";

import { AppearanceProvider } from "./AppearanceProvider";
import { MissingPackNotice } from "./MissingPackNotice";

interface WorldAppearanceProps {
  worldId: string;
  children: ReactNode;
}

export function WorldAppearance({ worldId, children }: WorldAppearanceProps) {
  const [packId, setPackId] = useState<string | null>(null);
  const [revision, setRevision] = useState(0);

  useEffect(() => {
    let isMounted = true;
    getWorld(worldId)
      .then((world) => {
        if (isMounted) setPackId(world?.interfacePackId ?? null);
      })
      // A look that cannot be read costs nothing: the base pack applies and
      // every action still works (FR-018).
      .catch(() => undefined);
    return () => {
      isMounted = false;
    };
  }, [worldId]);

  useEffect(() => {
    const stop = startAppearanceEventSync(
      {
        onAppearanceChanged: (id) => {
          setPackId(id);
          // Bumped even when the id is unchanged: a pack can be corrected or
          // reinstalled in place, and "the same id" is not "the same pack".
          setRevision((n) => n + 1);
        },
      },
      subscribeToWorldEvents(worldId),
    );
    return stop;
  }, [worldId]);

  return (
    <AppearanceProvider interfacePackId={packId} revision={revision}>
      <MissingPackNotice />
      {children}
    </AppearanceProvider>
  );
}
