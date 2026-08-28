import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

/**
 * The canvas readout's visibility setting (spec 028 diagnostics).
 *
 * Re-imported under `vi.resetModules()` with a stubbed `localStorage`
 * rather than driven through `resetEngineMonitorForTests`, because what
 * these tests are actually about is what happens across a page reload —
 * and the reset helper restores the default, which would let a module that
 * never reads storage at all pass every persistence test.
 */

const store = new Map<string, string>();

beforeEach(() => {
  store.clear();
  vi.resetModules();
  vi.stubGlobal("window", {
    localStorage: {
      getItem: (key: string) => store.get(key) ?? null,
      setItem: (key: string, value: string) => void store.set(key, value),
    },
  });
});

afterEach(() => {
  vi.unstubAllGlobals();
});

async function load() {
  return await import("../engineMonitor");
}

describe("the performance readout setting", () => {
  it("is off for someone who has never asked for it", async () => {
    const { isEngineMonitorVisible } = await load();
    expect(
      isEngineMonitorVisible(),
      "diagnostics nobody asked for must not start on top of the map",
    ).toBe(false);
  });

  it("is still on after a reload once it has been turned on", async () => {
    const first = await load();
    first.setEngineMonitorVisible(true);

    vi.resetModules();
    const { isEngineMonitorVisible } = await load();
    expect(isEngineMonitorVisible()).toBe(true);
  });

  it("is off again after a reload once it has been turned back off", async () => {
    const first = await load();
    first.setEngineMonitorVisible(true);
    first.setEngineMonitorVisible(false);

    vi.resetModules();
    const { isEngineMonitorVisible } = await load();
    expect(isEngineMonitorVisible()).toBe(false);
  });

  /**
   * A browser with site data blocked throws on every storage access. The
   * readout is a convenience; taking the canvas down with it would not be.
   */
  it("still answers when the browser refuses storage entirely", async () => {
    vi.stubGlobal("window", {
      localStorage: {
        getItem: () => {
          throw new Error("blocked");
        },
        setItem: () => {
          throw new Error("blocked");
        },
      },
    });
    vi.resetModules();
    const { isEngineMonitorVisible, setEngineMonitorVisible } = await load();

    expect(isEngineMonitorVisible()).toBe(false);
    expect(() => setEngineMonitorVisible(true)).not.toThrow();
    expect(
      isEngineMonitorVisible(),
      "the setting must still apply to this session even if it cannot persist",
    ).toBe(true);
  });

  it("tells every subscriber, and stops once unsubscribed", async () => {
    const { setEngineMonitorVisible, subscribeToEngineMonitor } = await load();
    const seen: boolean[] = [];
    const unsubscribe = subscribeToEngineMonitor((visible) => seen.push(visible));

    // Subscribing reports the current value at once, so a caller does not
    // have to render a wrong state until the first change.
    expect(seen).toEqual([false]);

    setEngineMonitorVisible(true);
    expect(seen).toEqual([false, true]);

    unsubscribe();
    setEngineMonitorVisible(false);
    expect(seen).toEqual([false, true]);
  });

  it("says nothing when set to the value it already has", async () => {
    const { setEngineMonitorVisible, subscribeToEngineMonitor } = await load();
    const seen: boolean[] = [];
    subscribeToEngineMonitor((visible) => seen.push(visible));

    setEngineMonitorVisible(false);
    expect(seen, "a no-op write must not wake every subscriber").toEqual([false]);
  });
});
