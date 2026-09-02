import { describe, expect, it } from "vitest";
import {
  describeArrival,
  type PartyArrival,
} from "@/pages/world/scenes/bringParty";

function arrival(arrived: number, present: number): PartyArrival {
  const ids = (n: number, prefix: string) =>
    Array.from({ length: n }, (_, i) => `${prefix}-${i}`);
  return {
    arrivedActorIds: ids(arrived, "new"),
    alreadyPresentActorIds: ids(present, "there"),
  };
}

describe("describeArrival", () => {
  it("counts what actually arrived", () => {
    expect(describeArrival(arrival(3, 0))).toBe("Brought 3 characters.");
    expect(describeArrival(arrival(1, 0))).toBe("Brought 1 character.");
  });

  it("says the party was already here rather than reporting nothing", () => {
    // The tavern -> cellar -> tavern -> cellar case (ADR-056 rule 2). A GM who
    // is told nothing happened would reasonably think the button is broken.
    expect(describeArrival(arrival(0, 2))).toBe(
      "The party was already here — 2 characters unchanged.",
    );
  });

  it("accounts for both halves when only some of the party was missing", () => {
    expect(describeArrival(arrival(2, 1))).toBe(
      "Brought 2 characters; 1 already here.",
    );
  });

  it("does not claim an arrival when there is no party at all", () => {
    expect(describeArrival(arrival(0, 0))).toBe(
      "No party characters to bring.",
    );
  });
});
