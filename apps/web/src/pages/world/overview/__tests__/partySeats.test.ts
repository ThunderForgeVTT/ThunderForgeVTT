import { describe, expect, it } from "vitest";
import type { WorldMemberRecord } from "@/api/worldMembers";
import type { WorldActorRecord } from "@/types/actor";
import { partySeats } from "@/pages/world/overview/partySeats";

function member(
  username: string,
  role: string,
  claimedActor: { id: string; label: string } | null = null,
): WorldMemberRecord {
  return {
    id: `m-${username}`,
    userId: `u-${username}`,
    username,
    role,
    joinedAt: "2026-01-01T00:00:00Z",
    claimedActor,
  };
}

function actor(
  id: string,
  over: Partial<WorldActorRecord> = {},
): WorldActorRecord {
  return {
    id,
    label: id,
    isNpc: false,
    availableForClaim: true,
    claimedBy: null,
    ...over,
  } as WorldActorRecord;
}

describe("partySeats", () => {
  it("names the Game Masters apart and seats the players", () => {
    const { gms, seats } = partySeats(
      [
        member("owner", "Owner"),
        member("gm", "GM"),
        member("ana", "Player", { id: "a1", label: "Brin" }),
        member("bo", "TrustedPlayer"),
      ],
      [actor("a1", { claimedBy: { id: "u-ana" } as never })],
    );
    expect(gms).toEqual(["owner", "gm"]);
    expect(seats).toEqual([
      { key: "m-ana", player: "ana", actorId: "a1", character: "Brin" },
      { key: "m-bo", player: "bo", actorId: null, character: null },
    ]);
  });

  it("shows an unclaimed player character as an open seat, never an NPC", () => {
    const { seats } = partySeats(
      [member("gm", "GM")],
      [
        actor("pc", { label: "Wren" }),
        actor("npc", { isNpc: true }),
        actor("kept", { availableForClaim: false }),
      ],
    );
    expect(seats).toEqual([
      { key: "pc", player: null, actorId: "pc", character: "Wren" },
    ]);
  });
});
