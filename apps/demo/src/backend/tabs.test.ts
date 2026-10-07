/**
 * Spec 081 R6: one world across tabs, with a lock and a channel faked the
 * way a browser behaves — the lock goes to one tab at a time and passes to
 * the longest waiting on release, and a message reaches every other tab.
 */
import type { ExecutionResult } from "graphql";
import { describe, expect, it } from "vitest";

import type { Viewer } from "../seed/world";
import type { WorldEvent } from "./events";
import type { OperationRequest } from "./execute";
import {
  connectTabs,
  oneAtATime,
  type Channel,
  type Locks,
  type TabMessage,
} from "./tabs";

/** A browser's lock manager, for one lock name. */
function fakeLocks() {
  let holder: number | null = null;
  const waiting: Array<{ tab: number; grant: () => void }> = [];
  return {
    holder: () => holder,
    /** The lock manager as tab `tab` sees it. */
    forTab(tab: number): Locks {
      return {
        request(_name, options, callback) {
          if (holder === null) {
            holder = tab;
            void callback({});
          } else if (options.ifAvailable) {
            void callback(null);
          } else {
            waiting.push({ tab, grant: () => void callback({}) });
          }
          return Promise.resolve();
        },
      };
    },
    /** Tab `tab` closes: its lock goes to the longest waiting. */
    close(tab: number) {
      const at = waiting.findIndex((w) => w.tab === tab);
      if (at !== -1) waiting.splice(at, 1);
      if (holder !== tab) return;
      const next = waiting.shift();
      holder = next ? next.tab : null;
      next?.grant();
    },
  };
}

/** One `BroadcastChannel` name: a post reaches every other open tab. */
function fakeChannels() {
  const listeners = new Map<number, Array<(m: { data: TabMessage }) => void>>();
  const posted: Array<{ from: number; message: TabMessage }> = [];
  return {
    posted,
    forTab(tab: number): Channel {
      listeners.set(tab, []);
      return {
        postMessage(message) {
          posted.push({ from: tab, message });
          // Structured clone, and later than the post, as a browser does.
          const copy = structuredClone(message);
          for (const [other, heard] of listeners) {
            if (other === tab) continue;
            queueMicrotask(() => heard.forEach((l) => l({ data: copy })));
          }
        },
        addEventListener(_type, listener) {
          listeners.get(tab)!.push(listener);
        },
      };
    },
    close(tab: number) {
      listeners.delete(tab);
    },
  };
}

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

const event = (
  id: number,
  eventCode: number,
  visibility?: string,
): WorldEvent => ({
  id,
  eventCode,
  tokenEvent: visibility ? { rollId: `r${id}`, visibility } : {},
});

/** A table of tabs over one world, each with its own viewer. */
function table() {
  const locks = fakeLocks();
  const channels = fakeChannels();
  /** Who ran each operation, in the tab that ran it. */
  const ran: Array<{ tab: number; viewer: Viewer; query: string }> = [];
  const releasers = new Map<number, (event: WorldEvent) => void>();
  const tookOver: number[] = [];

  function open(tab: number, viewer: Viewer, available = true) {
    const delivered: WorldEvent[] = [];
    const tabs = connectTabs({
      locks: available ? locks.forTab(tab) : undefined,
      channel: available ? channels.forTab(tab) : undefined,
      viewer: () => viewer,
      run: async (as: Viewer, operation: OperationRequest) => {
        ran.push({ tab, viewer: as, query: operation.query });
        return { data: { ranOn: tab, as } } as ExecutionResult;
      },
      takeOver: () => tookOver.push(tab),
      deliver: (e) => delivered.push(e),
      onReleased: (post) => releasers.set(tab, post),
    });
    /** This tab, holding the world, releases an event. */
    const release = (e: WorldEvent) => {
      delivered.push(e);
      releasers.get(tab)?.(e);
    };
    return { tabs, delivered, release };
  }

  return { locks, channels, ran, tookOver, open };
}

describe("tabs", () => {
  it("runs a guest's request on the holder, as the guest's viewer", async () => {
    const t = table();
    const gm = t.open(1, "gm");
    const player = t.open(2, "player");
    await Promise.all([gm.tabs.ready, player.tabs.ready]);
    expect(gm.tabs.holds()).toBe(true);
    expect(player.tabs.holds()).toBe(false);

    const answer = await player.tabs.ask({ query: "{ session }" });
    expect(answer.data).toEqual({ ranOn: 1, as: "player" });
    expect(t.ran).toEqual([{ tab: 1, viewer: "player", query: "{ session }" }]);

    expect((await gm.tabs.ask({ query: "{ me }" })).data).toEqual({
      ranOn: 1,
      as: "gm",
    });
  });

  it("posts every released event, unfiltered, to every tab", async () => {
    const t = table();
    const gm = t.open(1, "gm");
    const player = t.open(2, "player");
    const other = t.open(3, "gm");
    await Promise.all([gm.tabs.ready, player.tabs.ready, other.tabs.ready]);

    gm.release(event(1, 36, "gm_only"));
    gm.release(event(2, 14));
    await settle();
    // Each tab's socket judges what its viewer may see; the channel does not.
    for (const tab of [gm, player, other]) {
      expect(tab.delivered.map((e) => e.id)).toEqual([1, 2]);
    }
  });

  it("hands the world to a waiting guest when the holder goes, which resends what was open", async () => {
    const t = table();
    const first = t.open(1, "gm");
    const second = t.open(2, "player");
    const third = t.open(3, "gm");
    await Promise.all([first.tabs.ready, second.tabs.ready, third.tabs.ready]);

    // The holder closes with a guest's request unanswered.
    t.channels.close(1);
    const asked = third.tabs.ask({ query: "{ worldRolls }" });
    t.locks.close(1);
    await settle();

    expect(t.tookOver).toEqual([2]);
    expect(second.tabs.holds()).toBe(true);
    expect((await asked).data).toEqual({ ranOn: 2, as: "gm" });
    // Its own requests are answered where it now holds the world.
    expect((await second.tabs.ask({ query: "{ x }" })).data).toEqual({
      ranOn: 2,
      as: "player",
    });

    second.release(event(9, 37, "gm_eyes"));
    await settle();
    expect(third.delivered.map((e) => e.id)).toEqual([9]);
  });

  it("answers its own waiting requests on taking over", async () => {
    const t = table();
    const first = t.open(1, "gm");
    const second = t.open(2, "player");
    await Promise.all([first.tabs.ready, second.tabs.ready]);
    t.channels.close(1);
    const asked = second.tabs.ask({ query: "{ y }" });
    await settle();
    t.locks.close(1);
    expect((await asked).data).toEqual({ ranOn: 2, as: "player" });
  });

  it("without locks or a channel, holds its own world", async () => {
    const t = table();
    const alone = t.open(1, "player", false);
    await alone.tabs.ready;
    expect(alone.tabs.holds()).toBe(true);
    expect((await alone.tabs.ask({ query: "{ z }" })).data).toEqual({
      ranOn: 1,
      as: "player",
    });
    expect(t.channels.posted).toEqual([]);
  });
});

describe("oneAtATime", () => {
  it("finishes one operation before the next starts, a refusal included", async () => {
    const order: string[] = [];
    const run = oneAtATime(
      async (viewer: Viewer, operation: OperationRequest) => {
        order.push(`start ${viewer} ${operation.query}`);
        await settle();
        order.push(`end ${viewer}`);
        if (operation.query === "bad") throw new Error("refused");
        return viewer;
      },
    );
    const results = await Promise.allSettled([
      run("gm", { query: "bad" }),
      run("player", { query: "ok" }),
    ]);
    expect(order).toEqual([
      "start gm bad",
      "end gm",
      "start player ok",
      "end player",
    ]);
    expect(results[1]).toEqual({ status: "fulfilled", value: "player" });
  });
});
