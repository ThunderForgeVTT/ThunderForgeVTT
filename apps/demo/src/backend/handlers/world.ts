/**
 * World-level settings the demo keeps, by the server's rules.
 *
 * Session notes (spec 017 FR-012, FR-013): the Game Master's alone to write,
 * an empty string is a real save, and no world event is recorded for them.
 * They are kept on the world in this browser, like everything else here.
 */
import { GraphQLError } from "graphql";
import { viewerIsGm } from "../actors";
import { demoState, markChanged } from "../state";

type Args = Record<string, any>; // eslint-disable-line @typescript-eslint/no-explicit-any
type Handler = (args: Args) => unknown;

export const worldMutations: Record<string, Handler> = {
  updateWorldSessionNotes: ({ input }) => {
    const state = demoState();
    if (!viewerIsGm(state)) {
      throw new GraphQLError(
        "Only the DM (Owner or GM) may update session notes",
      );
    }
    const world = state.world;
    world.sessionNotes = String(input.notes ?? "");
    markChanged();
    return world;
  },
};
