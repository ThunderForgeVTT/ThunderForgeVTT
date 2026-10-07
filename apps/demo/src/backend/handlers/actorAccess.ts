/**
 * An actor's ownership block (spec 010): who holds more than the default.
 *
 * Only the Game Master reads or changes it, and it lists explicit grants
 * only: the Game Master is Owner of everything without a row, and a member
 * with no row is a Viewer. The grants are kept on the actor (`permissions`).
 */
import { GraphQLError } from "graphql";
import { DEMO_PLAYER, DEMO_USER } from "../../seed/world";
import { actorGrants, findActor, viewerIsGm } from "../actors";
import { now } from "../events";
import { demoState, markChanged, type DemoState } from "../state";

type Args = Record<string, any>; // eslint-disable-line @typescript-eslint/no-explicit-any
type Handler = (args: Args) => unknown;

function needDm(state: DemoState): void {
  if (!viewerIsGm(state)) {
    throw new GraphQLError(
      "Only the DM (Owner or GM) may view or change an actor's ownership block",
    );
  }
}

export const actorAccessQueries: Record<string, Handler> = {
  actorPermissions: ({ actorId }) => {
    const state = demoState();
    needDm(state);
    return actorGrants(findActor(state, actorId));
  },
};

export const actorAccessMutations: Record<string, Handler> = {
  setActorPermission: ({ input }) => {
    const state = demoState();
    needDm(state);
    const actor = findActor(state, input.actorId);
    if (![DEMO_USER.id, DEMO_PLAYER.id].includes(input.userId)) {
      throw new GraphQLError("That user is not a member of this world");
    }
    const rows = actorGrants(actor);
    let row = rows.find((g) => g.userId === input.userId);
    if (!row) {
      row = { actorId: actor.id, userId: input.userId };
      rows.push(row);
    }
    Object.assign(row, { level: input.level, updatedAt: now() });
    markChanged();
    return row;
  },
  removeActorPermission: ({ actorId, userId }) => {
    const state = demoState();
    needDm(state);
    const rows = actorGrants(findActor(state, actorId));
    const index = rows.findIndex((g) => g.userId === userId);
    if (index < 0) return false;
    rows.splice(index, 1);
    markChanged();
    return true;
  },
};
