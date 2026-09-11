import { useEffect } from "react";
import { createToken } from "@/api/tokens";
import { onPlacementConfirmed } from "@/engine/bevy";

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
 * Nothing is dispatched to the store here. The world event the server records
 * brings the new token to every client, this one included, by the same path
 * as any other token change — so what this tab sees is what everybody sees.
 *
 * `enabled` is whoever runs the scene: the engine only starts a carry for a
 * Game Master, and the server refuses anyone else, so a player's tab has no
 * drop to answer.
 */
export function usePlaceActorTokens(
  sceneId: string | null,
  enabled: boolean,
): void {
  useEffect(() => {
    if (!enabled || !sceneId) return;
    return onPlacementConfirmed((event) => {
      // A prop is `InteractionTool`'s; only an actor's drop is ours.
      if (event.kind !== "actor") return;
      createToken({
        sceneId,
        actorId: event.reference,
        x: event.x,
        y: event.y,
      }).catch((error: unknown) => {
        console.error("Placing the token failed:", error);
      });
    });
  }, [sceneId, enabled]);
}
