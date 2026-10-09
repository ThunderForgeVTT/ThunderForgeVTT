/**
 * Opening a GraphQL socket to `/api/ws`.
 *
 * Every socket the web client opens goes through here, so every one of them
 * pings. The server closes a socket that has sent nothing for a minute: that
 * is how it lets go of a client that vanished without closing. A quiet world
 * gives a healthy client nothing to say, so without pings it would be closed
 * too; with them, only a client that is really gone goes quiet.
 */
import { createClient, type Client, type ClientOptions } from "graphql-ws";

/**
 * How often to ping. Four pings fit in the server's one-minute idle timeout
 * (`graphql::websocket::IDLE_TIMEOUT`), so one slow or dropped ping is not a
 * disconnect.
 */
export const SOCKET_KEEP_ALIVE_MS = 15_000;

/** A `graphql-ws` client for this page's host, pinging. */
export function createSocketClient(
  options: Omit<ClientOptions, "url" | "keepAlive">,
): Client {
  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  return createClient({
    ...options,
    url: `${protocol}//${window.location.host}/api/ws`,
    keepAlive: SOCKET_KEEP_ALIVE_MS,
  });
}
