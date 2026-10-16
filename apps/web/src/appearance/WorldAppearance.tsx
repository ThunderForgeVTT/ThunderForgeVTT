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
import {
  useRememberWorldChrome,
  useWorldChrome,
} from "@/hooks/world-chrome-context";
import { subscribeToWorldEvents } from "@/engine/world/sync/subscriptionClient";
import { startAppearanceEventSync } from "@/engine/world/sync/appearance";

import { AppearanceProvider } from "./AppearanceProvider";
import { MissingPackNotice } from "./MissingPackNotice";

interface WorldAppearanceProps {
  worldId: string;
  children: ReactNode;
}

const readInterfacePack = (worldId: string) =>
  getWorld(worldId).then((world) => world?.interfacePackId ?? null);

export function WorldAppearance({ worldId, children }: WorldAppearanceProps) {
  // Shared, so a page change keeps the world's look instead of flashing the
  // base pack while this page reads it again (`world-chrome-context`). A
  // look that cannot be read costs nothing: the base pack applies and every
  // action still works (FR-018).
  const packId =
    useWorldChrome(worldId, "interfacePackId", readInterfacePack) ?? null;
  const remember = useRememberWorldChrome();
  const [revision, setRevision] = useState(0);

  useEffect(() => {
    const stop = startAppearanceEventSync(
      {
        onAppearanceChanged: (id) => {
          remember(worldId, { interfacePackId: id });
          // Bumped even when the id is unchanged: a pack can be corrected or
          // reinstalled in place, and "the same id" is not "the same pack".
          setRevision((n) => n + 1);
        },
      },
      // Not play: this wraps a world's hub pages, which stay open to its
      // members while an operator has paused its play (spec 051 FR-024).
      // The stream still ends on a pause; it does not send anyone to the
      // notice. The playfield's own streams do that.
      subscribeToWorldEvents(worldId, { announcePause: false }),
    );
    return stop;
  }, [worldId, remember]);

  return (
    <AppearanceProvider interfacePackId={packId} revision={revision}>
      <MissingPackNotice />
      {children}
    </AppearanceProvider>
  );
}
