import { useEffect } from "react";
import { onPlacementConfirmed } from "@/engine/bevy";
import type { WorldStore } from "@/engine/world/store";

/**
 * Turns an actor's placement into a token — playtest 2026-09-10 P1, and the
 * half of spec 031 T032 that was ticked without being wired.
 *
 * The actors pane's Place starts a carry: the engine moves a preview under the
 * pointer and, on the click, reports where it was dropped. It creates nothing
 * itself, by design — whether a token exists is the server's decision
 * (Constitution Principle I), asked for from here. The prop half of that
 * handshake was wired in `InteractionTool`, which ignores anything but props;
 * nothing listened for an actor's drop, so Place made a square follow the
 * pointer and then vanish.
 *
 * The token goes into the store under an id minted here, so it shows at once;
 * the token bridge asks the server for it under that same id and takes it
 * back off if refused. The world event the server records brings it to every
 * other client, and confirms it on this one.
 *
 * `enabled` is whoever runs the scene: the engine only starts a carry for a
 * Game Master, and the server refuses anyone else, so a player's tab has no
 * drop to answer.
 */
export function usePlaceActorTokens(
  worldStore: WorldStore,
  sceneId: string | null,
  enabled: boolean,
): void {
  useEffect(() => {
    if (!enabled || !sceneId) return;
    return onPlacementConfirmed((event) => {
      // A prop is `InteractionTool`'s; only an actor's drop is ours.
      if (event.kind !== "actor") return;
      worldStore.dispatch(
        {
          type: "upsert_token",
          token: {
            id: crypto.randomUUID(),
            sceneId,
            actorId: event.reference,
            x: event.x,
            y: event.y,
            z: 0,
          },
        },
        "ui",
      );
    });
  }, [worldStore, sceneId, enabled]);
}
