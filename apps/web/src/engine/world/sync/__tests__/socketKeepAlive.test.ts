/**
 * Every GraphQL socket the web client opens pings the server.
 *
 * The server closes a socket that has sent nothing for a minute, which is how
 * it lets go of a client that vanished without closing (a laptop lid shut, a
 * network that dropped without a FIN). A quiet world sends the client nothing
 * and the client has nothing to say back, so without pings a healthy table on
 * a quiet world would be closed too. `graphql-ws` pings every `keepAlive` ms.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const createClient = vi.fn();

vi.mock("graphql-ws", () => ({ createClient }));

function fakeClient() {
  return {
    subscribe: vi.fn(() => () => undefined),
    on: vi.fn(() => () => undefined),
    dispose: vi.fn(),
    terminate: vi.fn(),
  };
}

beforeEach(() => {
  vi.resetModules();
  createClient.mockReset();
  createClient.mockImplementation(fakeClient);
  vi.stubGlobal("window", {
    location: { protocol: "http:", host: "localhost:3000" },
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
  });
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("the GraphQL socket's keepalive", () => {
  it("pings well inside the server's one-minute idle timeout", async () => {
    const { SOCKET_KEEP_ALIVE_MS } = await import("../socketClient");
    // Four pings to a timeout: one slow or dropped ping is not a disconnect.
    expect(SOCKET_KEEP_ALIVE_MS).toBe(15_000);
    expect(SOCKET_KEEP_ALIVE_MS * 4).toBeLessThanOrEqual(60_000);
  });

  it("is set on every socket the shared factory opens", async () => {
    const { createSocketClient } = await import("../socketClient");
    createSocketClient({ retryAttempts: Infinity });
    expect(createClient).toHaveBeenCalledWith(
      expect.objectContaining({
        url: expect.stringMatching(/^wss?:\/\/.+\/api\/ws$/),
        keepAlive: 15_000,
        retryAttempts: Infinity,
      }),
    );
  });

  it("is set on the world-event socket, which still retries forever", async () => {
    // After a timeout close (3008, which graphql-ws retries) the client
    // reconnects, and the reconnect runs the catch-up query.
    const { subscribeToWorldEvents } = await import("../subscriptionClient");
    subscribeToWorldEvents("world-1");
    expect(createClient).toHaveBeenCalledTimes(1);
    expect(createClient).toHaveBeenCalledWith(
      expect.objectContaining({ keepAlive: 15_000, retryAttempts: Infinity }),
    );
  });
});
