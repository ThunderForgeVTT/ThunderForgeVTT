import { describe, expect, it } from "vitest";

import { describeStanding, standingTitle } from "../playerStanding";

describe("what a player card says a person is", () => {
  it("names an Owner as the Game Master too", () => {
    expect(standingTitle("Owner")).toBe("Owner / Game Master");
    expect(standingTitle("GM")).toBe("Game Master");
    expect(standingTitle("TrustedPlayer")).toBe("Trusted Player");
    expect(standingTitle("Player")).toBe("Player");
  });

  it("shows a role this build cannot read as the server spelled it", () => {
    expect(standingTitle("Spectator")).toBe("Spectator");
    expect(
      describeStanding({ role: "Spectator", claimedActor: null }).runsTheTable,
    ).toBe(false);
  });
});

describe("what a player card says a person holds", () => {
  it("says a Game Master plays every character", () => {
    const owner = describeStanding({ role: "Owner", claimedActor: null });
    expect(owner.runsTheTable).toBe(true);
    expect(owner.holding).toBe("Playing all characters.");

    const gm = describeStanding({ role: "GM", claimedActor: null });
    expect(gm.holding).toBe("Playing all characters.");
  });

  it("keeps saying so when a Game Master holds a character of their own", () => {
    // The bound character is beside "all characters", never instead of it.
    const standing = describeStanding({
      role: "Owner",
      claimedActor: { label: "Zephyr" },
    });
    expect(standing.holding).toBe(
      "Playing all characters, with Zephyr as their own.",
    );
  });

  it("names a player's one character, or says there is none", () => {
    expect(
      describeStanding({ role: "Player", claimedActor: { label: "Aria" } })
        .holding,
    ).toBe("Playing Aria.");
    expect(
      describeStanding({ role: "TrustedPlayer", claimedActor: null }).holding,
    ).toBe("No character yet.");
  });
});
