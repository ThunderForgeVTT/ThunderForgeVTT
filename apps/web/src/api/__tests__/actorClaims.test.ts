import { describe, expect, it } from "vitest";
import {
  ALREADY_CLAIMED,
  CLAIM_CHANGED,
  isAlreadyClaimed,
  isClaimChanged,
} from "@/api/actorClaims";
import { GraphQLRequestError } from "@/api/graphqlClient";

/**
 * Spec 031 FR-034. Three surfaces write the claim relation and all three
 * have to read a refusal the same way. These assert that the reading is
 * done on the server's code and on nothing else — a message-sniffing
 * version of `isAlreadyClaimed` would pass a hand-written happy-path test
 * and then break the first time somebody reworded the refusal.
 */

function refusal(codes: string[], message = "refused"): GraphQLRequestError {
  return new GraphQLRequestError(message, { codes });
}

describe("claim refusal classification", () => {
  it("recognises a lost race by its code", () => {
    expect(isAlreadyClaimed(refusal([ALREADY_CLAIMED]))).toBe(true);
  });

  it("recognises a stale release by its code", () => {
    expect(isClaimChanged(refusal([CLAIM_CHANGED]))).toBe(true);
  });

  it("does not confuse the two refusals with each other", () => {
    expect(isClaimChanged(refusal([ALREADY_CLAIMED]))).toBe(false);
    expect(isAlreadyClaimed(refusal([CLAIM_CHANGED]))).toBe(false);
  });

  it("treats a refusal carrying no code as a real failure", () => {
    // Permission refusals and server faults arrive this way, and reporting
    // either as "somebody was quicker" would send a GM looking for a
    // player who does not exist.
    expect(isAlreadyClaimed(refusal([], "You may not do that"))).toBe(false);
    expect(isClaimChanged(refusal([]))).toBe(false);
  });

  it("ignores the message entirely", () => {
    expect(
      isAlreadyClaimed(refusal([], "That character is already played")),
    ).toBe(false);
  });

  it("is false for anything that is not a GraphQL refusal", () => {
    expect(isAlreadyClaimed(new Error(ALREADY_CLAIMED))).toBe(false);
    expect(isClaimChanged(null)).toBe(false);
    expect(isAlreadyClaimed(undefined)).toBe(false);
  });
});
