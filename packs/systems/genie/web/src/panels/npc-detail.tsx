/**
 * `npc-detail` — Genie's NPC shop, on an NPC's actor page.
 *
 * Spec 020 (User Story 2). The actor page used to ask
 * `actor.gameSystemId === "genie" && actor.isNpc`; only the first half of
 * that was a game-system decision. Whether an actor is an NPC is a fact
 * about the actor, so the host still decides it and only offers this slot on
 * an NPC — see `NpcDetailPanelProps`.
 *
 * Shown for any NPC so a GM can author listings before stocking anything,
 * and hidden from a non-GM viewer when it has none — that second rule is
 * `ShopPanel`'s own (spec 020 "Scenario 6"), and stays inside it.
 */
import type { NpcDetailPanelProps } from "@thunderforge/host";
import { GenieShopPanel } from "../components/ShopPanel";

export default function GenieNpcDetailPanel({
  worldId,
  actorId,
  currentUserId,
  isGm,
}: NpcDetailPanelProps) {
  return (
    <GenieShopPanel
      worldId={worldId}
      npcActorId={actorId}
      currentUserId={currentUserId}
      isGm={isGm}
    />
  );
}
