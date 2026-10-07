/**
 * A client whose subscription dropped asks what it missed, and the Books tab
 * asks what the world holds. Both are answered, not refused.
 */
import { beforeEach, describe, expect, it } from "vitest";
import { freshWorld, must, refused } from "../testing/world";
import { EVENT, record } from "../events";
import { demoState } from "../state";

const SINCE = `query ($worldId: UUID!, $afterId: Int!) {
  worldEventsSince(worldId: $worldId, afterId: $afterId) {
    events { id eventCode tokenEvent } truncated latestId
  }
}`;
const BOOKS = `query ($worldId: UUID!) {
  worldBookList(worldId: $worldId) { compendiumId }
  compendiumsOfferedToWorld(worldId: $worldId) { id }
  worldAdditionsWithoutBook(worldId: $worldId) { id }
}`;

beforeEach(async () => {
  await freshWorld("gm");
});

describe("catching up", () => {
  it("returns what came after the cursor, oldest first, and the newest id", async () => {
    const worldId = demoState().world.id;
    const before = (await must(SINCE, { worldId, afterId: 0 })).worldEventsSince
      .latestId as number;
    record(EVENT.wall, { action: "created", n: 1 });
    record(EVENT.light, { action: "created", n: 2 });
    record(EVENT.token, { action: "updated", n: 3 });

    const { worldEventsSince } = await must(SINCE, {
      worldId,
      afterId: before + 1,
    });
    expect(worldEventsSince.truncated).toBe(false);
    expect(worldEventsSince.latestId).toBe(before + 3);
    expect(
      worldEventsSince.events.map((e: { eventCode: number }) => e.eventCode),
    ).toEqual([EVENT.light, EVENT.token]);
    expect(worldEventsSince.events[0].tokenEvent).toEqual({
      action: "created",
      n: 2,
    });

    const caughtUp = (await must(SINCE, { worldId, afterId: before + 3 }))
      .worldEventsSince;
    expect(caughtUp).toMatchObject({ events: [], truncated: false });
    expect(refused).toEqual([]);
  });

  it("says to resynchronise when the gap is more than a page", async () => {
    const worldId = demoState().world.id;
    const start = (await must(SINCE, { worldId, afterId: 0 })).worldEventsSince
      .latestId as number;
    for (let n = 0; n < 201; n += 1) record(EVENT.token, { n });
    const { worldEventsSince } = await must(SINCE, { worldId, afterId: start });
    expect(worldEventsSince.truncated).toBe(true);
    expect(worldEventsSince.events).toHaveLength(200);
    expect(worldEventsSince.latestId).toBe(start + 201);
  });
});

describe("the world's books", () => {
  it("are none, and none are offered", async () => {
    const answer = await must(BOOKS, { worldId: demoState().world.id });
    expect(answer).toEqual({
      worldBookList: [],
      compendiumsOfferedToWorld: [],
      worldAdditionsWithoutBook: [],
    });
    expect(refused).toEqual([]);
  });
});
