import { describe, expect, it } from "vitest";
import {
  filterPlayers,
  matchesPlayerQuery,
  type SearchablePlayer,
} from "@/pages/world/players/playerFilter";

function player(
  username: string,
  role: string,
  character: string | null,
): SearchablePlayer {
  return {
    username,
    role,
    claimedActor: character === null ? null : { label: character },
  };
}

const ROSTER: SearchablePlayer[] = [
  player("sam", "Player", "Aria Nightbloom"),
  player("Robin", "GM", null),
  player("mira", "Player", "Bran the Quiet"),
];

describe("matchesPlayerQuery", () => {
  it("keeps everybody when nothing has been typed", () => {
    expect(filterPlayers(ROSTER, "")).toHaveLength(3);
    expect(filterPlayers(ROSTER, "   ")).toHaveLength(3);
  });

  it("finds a player by the character they are playing", () => {
    // The question a GM actually asks mid-session: not "where is mira" but
    // "who has Bran".
    expect(filterPlayers(ROSTER, "bran").map((p) => p.username)).toEqual([
      "mira",
    ]);
  });

  it("finds a player by name regardless of case", () => {
    expect(filterPlayers(ROSTER, "ROBIN").map((p) => p.username)).toEqual([
      "Robin",
    ]);
  });

  it("finds every player holding a role", () => {
    expect(filterPlayers(ROSTER, "gm").map((p) => p.username)).toEqual([
      "Robin",
    ]);
  });

  it("matches inside a word, not only at its start", () => {
    expect(matchesPlayerQuery(ROSTER[0], "night")).toBe(true);
  });

  it("does not crash on a player with no character", () => {
    expect(matchesPlayerQuery(ROSTER[1], "aria")).toBe(false);
  });

  it("returns nothing when nothing matches", () => {
    expect(filterPlayers(ROSTER, "zzz")).toEqual([]);
  });
});
