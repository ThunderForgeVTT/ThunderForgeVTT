/**
 * Two reads the demo answers honestly rather than refusing:
 *
 * - `worldEventsSince`, the catch-up a client asks for after its subscription
 *   drops, from the events this page recorded.
 * - The world's books. The demo world holds none and has none offered to it,
 *   which is the truth, so the Books tab says so instead of refusing.
 */
import { eventsSince } from "../events";

type Args = Record<string, any>; // eslint-disable-line @typescript-eslint/no-explicit-any
type Handler = (args: Args) => unknown;

export const catchUpQueries: Record<string, Handler> = {
  worldEventsSince: ({ afterId }) => eventsSince(Number(afterId)),
  worldBookList: () => [],
  compendiumsOfferedToWorld: () => [],
  // Additions kept beside a book the world switched off: with no books,
  // there are none.
  worldAdditionsWithoutBook: () => [],
};
