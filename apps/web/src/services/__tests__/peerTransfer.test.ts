import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

/**
 * The user's say over peer transfer (spec 028 FR-049, T092).
 *
 * Everything here is about the *setting*, because the setting is the part
 * with a privacy consequence: peer transfer is on by default and reveals
 * network addresses between participants, so "off" has to mean off — across
 * a reload, and against a late report from a connection that was already
 * closing. The panel's rendering is not covered: this project's vitest
 * environment is `node` and there are no component tests, so the panel is an
 * e2e concern.
 *
 * The module reads its stored value once, at import, which is what the engine
 * needs (it asks `isPeerTransferEnabled()` before opening a connection, on a
 * page that may never render a panel). So each test re-imports it against a
 * freshly stubbed `window` rather than calling a reset — the reset restores
 * the default, which would hide exactly the "did it read storage at all?"
 * failure these tests exist to catch.
 */

const STORAGE_KEY = "thunderforge:peer-transfer-enabled";

/** A minimal `localStorage`, since the vitest environment is `node`. */
function stubWindow(initial: Record<string, string> = {}) {
  const store = new Map(Object.entries(initial));
  const localStorage = {
    getItem: (key: string) => store.get(key) ?? null,
    setItem: (key: string, value: string) => void store.set(key, value),
    removeItem: (key: string) => void store.delete(key),
  };
  (globalThis as { window?: unknown }).window = { localStorage };
  return store;
}

/** Fresh module state, reading whatever the stubbed storage currently holds. */
async function loadModule() {
  vi.resetModules();
  return import("../peerTransfer");
}

beforeEach(() => {
  stubWindow();
});

afterEach(() => {
  delete (globalThis as { window?: unknown }).window;
});

describe("the peer transfer setting", () => {
  /**
   * FR-049 decides the default, and it is the opposite of the cautious
   * choice: peer-to-peer is the intended model, disclosed rather than
   * avoided. A "safe" default of off would quietly disable server-isolated
   * play (FR-057) for everyone who never opened settings.
   */
  it("is on for someone who has never touched it", async () => {
    const peer = await loadModule();

    expect(peer.isPeerTransferEnabled()).toBe(true);
    expect(peer.getPeerTransferState().enabled).toBe(true);
  });

  it("stays off across a reload once it has been turned off", async () => {
    const store = stubWindow();
    const first = await loadModule();

    first.setPeerTransferEnabled(false);
    expect(store.get(STORAGE_KEY)).toBe("false");

    // A second import against the same storage is what a page reload is.
    // Failing this would mean the connection the user refused is opened on
    // the very next visit.
    const second = await loadModule();
    expect(second.isPeerTransferEnabled()).toBe(false);
  });

  it("comes back on across a reload once it has been turned back on", async () => {
    stubWindow({ [STORAGE_KEY]: "false" });
    const first = await loadModule();
    expect(first.isPeerTransferEnabled()).toBe(false);

    first.setPeerTransferEnabled(true);

    const second = await loadModule();
    expect(second.isPeerTransferEnabled()).toBe(true);
  });

  /**
   * A profile with site data blocked throws on every storage access. The
   * engine still asks whether it may connect, on a page that only wanted to
   * draw a canvas, so a throw here would take the world down over a setting.
   */
  it("still answers when the browser refuses storage entirely", async () => {
    (globalThis as { window?: unknown }).window = {
      localStorage: {
        getItem: () => {
          throw new Error("site data blocked");
        },
        setItem: () => {
          throw new Error("site data blocked");
        },
      },
    };
    const peer = await loadModule();

    expect(peer.isPeerTransferEnabled()).toBe(true);
    expect(() => peer.setPeerTransferEnabled(false)).not.toThrow();
    // It could not be persisted, but it must still apply to this session —
    // otherwise the switch visibly does nothing.
    expect(peer.isPeerTransferEnabled()).toBe(false);
  });
});

describe("activity reported by the engine", () => {
  it("reaches the indicator while peer transfer is on", async () => {
    const peer = await loadModule();
    const seen: number[] = [];
    peer.subscribeToPeerTransfer((state) => seen.push(state.connectedPeers));

    peer.reportPeerTransferActivity({
      connectedPeers: 2,
      bytesFromPeers: 4096,
    });

    expect(peer.getPeerTransferState()).toMatchObject({
      enabled: true,
      connectedPeers: 2,
      bytesFromPeers: 4096,
    });
    // Subscribers get the current state on subscribe, then each change.
    expect(seen).toEqual([0, 2]);
  });

  /**
   * The failure this catches: a report already in flight as the user turns
   * the setting off lands afterwards and repopulates the counters, so the
   * indicator says "2 peers connected" next to a switch reading Off. The
   * user's reasonable conclusion is that the switch did not work.
   */
  it("is ignored entirely while peer transfer is off", async () => {
    const peer = await loadModule();
    peer.reportPeerTransferActivity({ connectedPeers: 3, bytesFromPeers: 100 });

    peer.setPeerTransferEnabled(false);
    peer.reportPeerTransferActivity({ connectedPeers: 3, bytesFromPeers: 200 });

    expect(peer.getPeerTransferState()).toEqual({
      enabled: false,
      connectedPeers: 0,
      bytesFromPeers: 0,
      verificationFailures: 0,
    });
  });

  it("starts from zero again when peer transfer is turned back on", async () => {
    const peer = await loadModule();
    peer.reportPeerTransferActivity({
      connectedPeers: 3,
      verificationFailures: 1,
    });
    peer.setPeerTransferEnabled(false);

    peer.setPeerTransferEnabled(true);

    // Counting a session that has not started yet would misreport the very
    // thing the indicator exists to disclose.
    expect(peer.getPeerTransferState()).toEqual({
      enabled: true,
      connectedPeers: 0,
      bytesFromPeers: 0,
      verificationFailures: 0,
    });
  });

  it("tells every subscriber about a change, and stops on unsubscribe", async () => {
    const peer = await loadModule();
    const a: boolean[] = [];
    const b: boolean[] = [];
    peer.subscribeToPeerTransfer((state) => a.push(state.enabled));
    const stop = peer.subscribeToPeerTransfer((state) => b.push(state.enabled));

    peer.setPeerTransferEnabled(false);
    stop();
    peer.setPeerTransferEnabled(true);

    expect(a).toEqual([true, false, true]);
    expect(b).toEqual([true, false]);
  });
});
