import { describe, expect, it } from "vitest";
import { GraphQLRequestError } from "@/api/graphqlClient";
import { isRefusal, joinRead, patchChrome } from "./world-chrome-store";

describe("patchChrome", () => {
  it("returns the same object when the patch says nothing new", () => {
    const current = { w1: { resumeScene: "Crypt", claimCleared: true } };
    expect(patchChrome(current, "w1", { resumeScene: "Crypt" })).toBe(current);
  });

  it("treats an equal record read again as nothing new", () => {
    const current = { w1: { playState: { paused: false, reason: null } } };
    const again = { paused: false, reason: null };
    expect(
      patchChrome(current as never, "w1", { playState: again } as never),
    ).toBe(current);
  });

  it("keeps the other keys and the other worlds", () => {
    const current = { w1: { resumeScene: "Crypt" }, w2: { resumeScene: null } };
    const next = patchChrome(current, "w1", { claimCleared: true });
    expect(next.w1).toEqual({
      resumeScene: "Crypt",
      claimCleared: true,
    });
    expect(next.w2).toBe(current.w2);
  });

  it("replaces the member list whole", () => {
    const a = { id: "a" };
    const b = { id: "b" };
    const current = { w1: { members: [a, b] } };
    const next = patchChrome(current as never, "w1", { members: [b] } as never);
    expect(next.w1?.members).toEqual([b]);
  });
});

describe("joinRead", () => {
  it("hands a second caller the first caller's read", async () => {
    const inFlight = new Map<string, Promise<unknown>>();
    let calls = 0;
    const load = () => {
      calls += 1;
      return Promise.resolve(calls);
    };
    const first = joinRead(inFlight, "w1:world", load);
    const second = joinRead(inFlight, "w1:world", load);
    expect(second).toBe(first);
    expect(await second).toBe(1);
    expect(calls).toBe(1);
  });

  it("forgets a read once it settles, answered or not", async () => {
    const inFlight = new Map<string, Promise<unknown>>();
    await joinRead(inFlight, "w1:world", () => Promise.resolve(1));
    expect(inFlight.size).toBe(0);
    await joinRead(inFlight, "w1:world", () =>
      Promise.reject(new Error("x")),
    ).catch(() => undefined);
    expect(inFlight.size).toBe(0);
  });
});

describe("isRefusal", () => {
  it("is a server that answered with a code", () => {
    expect(
      isRefusal(new GraphQLRequestError("no", { codes: ["FORBIDDEN"] })),
    ).toBe(true);
  });

  it("is not a failure that carried no code", () => {
    expect(
      isRefusal(new GraphQLRequestError("offline", { transport: true })),
    ).toBe(false);
    expect(isRefusal(new TypeError("Failed to fetch"))).toBe(false);
  });
});
