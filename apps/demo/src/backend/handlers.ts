/**
 * Spec 074 FR-009: what the demo answers, and nothing else.
 *
 * The keys of `queries` and `mutations` are the declared list. A root field
 * that is not a key here gets the one "not part of the demo" answer from
 * `execute.ts`.
 *
 * These are not the server's rules. The visitor is the Game Master and alone,
 * so there is nobody for a permission to refuse; a handler does what was
 * asked, records the event a server would have broadcast, and returns the row.
 */
import { GraphQLError } from "graphql";
import { DEMO_PLAYER, DEMO_USER, newToken, tokenOf } from "../seed/world";
import {
  CHECKS,
  CONDITIONS,
  actorRow,
  claim,
  claimRow,
  findActor,
  rollCheck,
  systemDataOf,
  tokenForViewer,
  updateSystemData,
  viewerIsGm,
  viewerUser,
  visibleActors,
} from "./actors";
import { EVENT, now, record } from "./events";
import {
  actorAccessMutations,
  actorAccessQueries,
} from "./handlers/actorAccess";
import { loreMutations, loreQueries } from "./handlers/lore";
import { catchUpQueries } from "./handlers/catchUp";
import { worldMutations } from "./handlers/world";
import { areaMutations, areaQueries } from "./handlers/index";
import { levelFor, onLevel, refuse } from "./handlers/common";
import { combatMutations, combatQueries } from "./handlers/combat";
import { demoState, markChanged, type DemoState, type Row } from "./state";

type Args = Record<string, any>; // eslint-disable-line @typescript-eslint/no-explicit-any
type Handler = (args: Args) => unknown;

const by = { createdBy: DEMO_USER.id, updatedBy: DEMO_USER.id };

const ALL_TOOLS = [
  "select",
  "walls",
  "lights",
  "shapes",
  "tokens",
  "interactions",
];
const PLAYER_TOOLS = ["select", "shapes"];

function notFound(what: string): never {
  throw new GraphQLError(`${what} not found`);
}

function scene(state: DemoState, sceneId: string): Row {
  return state.scenes.find((s) => s.sceneId === sceneId) ?? notFound("Scene");
}

/** The fields of `input` that were actually sent. */
function given(input: Args): Row {
  return Object.fromEntries(
    Object.entries(input).filter(([, value]) => value !== undefined),
  );
}

function withLevelCounts(state: DemoState, level: Row): Row {
  return {
    ...level,
    tokenCount: state.tokens.filter((t) => t.levelId === level.levelId).length,
  };
}

function worldRow(state: DemoState): Row {
  if (viewerIsGm(state)) return state.world;
  return {
    ...state.world,
    description:
      "A table of your own, seen as one of its players. Nothing you do here leaves this browser.",
  };
}

/** The two members: the Game Master who made the world, and one player. */
function members(state: DemoState): Row[] {
  const at = state.world.createdAt;
  const row = (user: typeof DEMO_USER, role: string, n: number): Row => ({
    id: `${state.world.id.slice(0, -1)}${n}`,
    worldId: state.world.id,
    userId: user.id,
    username: user.username,
    role,
    joinedAt: at,
    createdAt: at,
    updatedAt: at,
    claimedActor: null,
  });
  const player = row(DEMO_PLAYER, "Player", 2);
  player.claimedActor = claimRow(state)?.actor ?? null;
  return [row(DEMO_USER, "Owner", 1), player];
}

function viewerRole(state: DemoState): string {
  return viewerIsGm(state) ? "Owner" : "Player";
}

/**
 * One kind of thing on a scene: how it is found, what its identifier is
 * called in a row and in an event, and which event announces it.
 */
function kind(
  rows: (state: DemoState) => Row[],
  key: string,
  eventKey: string,
  eventCode: number,
) {
  const announce = (action: string, row: Row) =>
    record(eventCode, {
      action,
      [eventKey]: row[key],
      scene_id: row.sceneId,
      level_id: row.levelId,
    });
  return {
    create(row: Row): Row {
      rows(demoState()).push(row);
      announce("created", row);
      markChanged();
      return row;
    },
    update(id: string, changes: Row): Row {
      const row =
        rows(demoState()).find((candidate) => candidate[key] === id) ??
        notFound("That");
      Object.assign(row, changes, { updatedAt: now() });
      if ("updatedBy" in row) row.updatedBy = DEMO_USER.id;
      announce("updated", row);
      markChanged();
      return row;
    },
    remove(id: string): boolean {
      const list = rows(demoState());
      const index = list.findIndex((candidate) => candidate[key] === id);
      if (index < 0) return false;
      const [row] = list.splice(index, 1);
      announce("deleted", row);
      markChanged();
      return true;
    },
  };
}

const walls = kind((s) => s.walls, "wallId", "wall_id", EVENT.wall);
const lights = kind((s) => s.lights, "lightId", "light_id", EVENT.light);
const tokens = kind((s) => s.tokens, "tokenId", "token_id", EVENT.token);

/**
 * A door is a wall and something to click, and the second follows from the
 * first: a wall that is a door has one interactive, which toggles it. The
 * server keeps a row for that; here it is read off the wall, so the two can
 * never disagree.
 */
const DOOR_PREFIX = "d00d0000";

function doorInteractiveId(wallId: string): string {
  return DOOR_PREFIX + wallId.slice(DOOR_PREFIX.length);
}

/**
 * A door's interactive as `interactives.rs` answers it: the Game Master gets
 * the authoring view, a player only that it is there and whether they may
 * use it, which a lock forbids.
 */
function doorInteractive(wall: Row, gm: boolean): Row {
  const id = {
    interactiveId: doorInteractiveId(wall.wallId as string),
    sceneId: wall.sceneId,
    levelId: wall.levelId,
    subjectKind: "door",
    subjectRef: wall.wallId,
    geometry: null,
    trigger: "click",
  };
  if (!gm) {
    return {
      ...id,
      effectId: null,
      effectConfig: null,
      activation: null,
      fireMode: null,
      firedAt: null,
      available: null,
      canActivate: !wall.locked,
    };
  }
  return {
    ...id,
    effectId: "door.set_state",
    effectConfig: { target: wall.wallId, state: "toggle" },
    activation: "anyone",
    fireMode: "always",
    firedAt: null,
    available: true,
    canActivate: true,
  };
}

/** A door changed: told as a door, and as the wall change it also is. */
function changeDoor(wallId: string, changes: Row): Row {
  const wall = walls.update(wallId, changes);
  record(EVENT.door, {
    action: "changed",
    wall_id: wall.wallId,
    scene_id: wall.sceneId,
  });
  return wall;
}

/** `mutations_chat.rs`: a message's longest body, in characters. */
const MAX_CHAT_CHARS = 4000;

/** `validate_body`: trimmed, not empty, and not too long. */
function chatBody(body: string): string {
  const trimmed = body.trim();
  if (!trimmed) throw new GraphQLError("Message cannot be empty");
  if ([...trimmed].length > MAX_CHAT_CHARS) {
    throw new GraphQLError(
      `Message cannot exceed ${MAX_CHAT_CHARS} characters`,
    );
  }
  return trimmed;
}

/** `resolve_history_limit`: 100 unless asked for 1 to 500. */
function chatHistoryLimit(limit: unknown): number {
  if (typeof limit !== "number" || limit <= 0) return 100;
  return Math.min(limit, 500);
}

export const queries: Record<string, Handler> = {
  // The instance, as a signed-out visitor is told about it.
  featureFlags: () => [],
  instanceRepositoryIntegration: () => ({
    configured: false,
    operatorGuidance: "Repository sync is not part of the demo.",
  }),

  // The world and who is in it.
  myWorldsWithRole: () => {
    const state = demoState();
    return [{ role: viewerRole(state), world: worldRow(state) }];
  },
  myLibrary: () => [],
  world: ({ id }) =>
    id === demoState().world.id ? worldRow(demoState()) : null,
  worldMembers: () => members(demoState()),
  worldMember: ({ userId }) =>
    members(demoState()).find((m) => m.userId === userId) ?? null,
  worldInvites: () => [],
  worldPlayState: () => ({ paused: false, pausedAt: null, history: [] }),
  worldStatistics: () => {
    const state = demoState();
    return {
      scenes: state.scenes.length,
      members: 2,
      membersWithCharacter: 1,
      characters: state.actors.filter((a) => !a.isNpc).length,
      npcs: state.actors.filter((a) => a.isNpc).length,
      tokens: state.tokens.length,
      activeEncounter: null,
    };
  },
  worldActors: () => {
    const state = demoState();
    return visibleActors(state).map((actor) => actorRow(state, actor));
  },
  searchActors: ({ query }) => {
    const state = demoState();
    const needle = String(query ?? "").toLowerCase();
    return visibleActors(state)
      .filter((actor) => String(actor.label).toLowerCase().includes(needle))
      .map((actor) => actorRow(state, actor));
  },
  availableActors: () => {
    const state = demoState();
    return visibleActors(state)
      .filter(
        (a) => a.availableForClaim === true && a.id !== state.claimedActorId,
      )
      .map((a) => actorRow(state, a));
  },
  myActorClaim: () => {
    const state = demoState();
    return viewerIsGm(state) ? null : claimRow(state);
  },
  actorSystemData: ({ actorId }) => {
    const state = demoState();
    findActor(state, actorId);
    return systemDataOf(state, actorId);
  },
  actorAbilities: () => [],
  actorInventory: () => [],
  worldItems: () => [],
  worldAbilities: () => [],
  worldSystemConditions: () => CONDITIONS,
  systemChecks: () => CHECKS,
  // Spec 084: the demo is a 5e world, and 5e registers roll facets.
  rollsWithAdvantage: () => true,
  worldCollections: () => [],
  // `world_chat_messages_impl`: the newest `limit`, in reading order, and
  // never a GM-only message to a player.
  worldChatMessages: ({ limit }) => {
    const state = demoState();
    const seen = state.chat.filter(
      (message) => viewerIsGm(state) || !message.gmOnly,
    );
    return seen.slice(-chatHistoryLimit(limit));
  },
  worldSystemSettings: () => [
    {
      key: "inspiration",
      label: "Heroic Inspiration",
      description:
        "Show Inspiration on character sheets. Turn it off if your table does not award it.",
      kind: "boolean",
      options: [],
      min: null,
      max: null,
      maxLength: null,
      defaultValue: true,
      value: true,
      isDefault: true,
    },
  ],
  abilityVocabulary: () => ({
    umbrella: { label: "Ability", pluralLabel: "Abilities" },
    types: [
      {
        id: "spell",
        label: "Spell",
        pluralLabel: "Spells",
        order: 0,
        builtin: true,
        binds: "CHARACTER",
        grade: { label: "Level", min: 0, max: 9 },
      },
      {
        id: "feat",
        label: "Feat",
        pluralLabel: "Feats",
        order: 1,
        builtin: true,
        binds: "CHARACTER",
        grade: null,
      },
    ],
  }),
  // Spec 082 R10: the GM holds every tool, the player what players hold by
  // default. The demo takes nothing away, so the grants are those defaults.
  authoringToolGrants: () => {
    const state = demoState();
    if (!viewerIsGm(state)) {
      refuse("Only Owners and GMs can see this world's authoring tool grants");
    }
    return members(state)
      .filter((member) => member.role === "Player")
      .map((member) => ({
        worldMemberId: member.id,
        userId: member.userId,
        tools: PLAYER_TOOLS,
      }));
  },
  authoringTools: () => (viewerIsGm(demoState()) ? ALL_TOOLS : PLAYER_TOOLS),
  pendingOffers: () => [],
  peerSessions: () => [],
  worldSyncPlan: () => ({ fetch: [], evict: [], canonicalVersion: 1 }),

  // Scenes and what stands on them.
  scenes: () => demoState().scenes,
  scene: ({ sceneId }) =>
    demoState().scenes.find((s) => s.sceneId === sceneId) ?? null,
  sceneLevels: ({ sceneId }) => {
    const state = demoState();
    return state.levels
      .filter((level) => level.sceneId === sceneId)
      .map((level) => withLevelCounts(state, level));
  },
  sceneUnits: ({ sceneId }) => ({
    perCell: 5,
    label: "ft",
    gridSize: scene(demoState(), sceneId).gridSize,
  }),
  sceneAttacks: () => [],
  sceneExploration: () => ({ enabled: false, epoch: 0, mine: 0 }),
  walls: (args) => onLevel(demoState().walls, args),
  tokens: (args) => {
    const state = demoState();
    return onLevel(state.tokens, args).map((t) => tokenForViewer(state, t));
  },
  lightSources: (args) => onLevel(demoState().lights, args),
  interactives: (args) => {
    const state = demoState();
    const gm = viewerIsGm(state);
    return onLevel(state.walls, args)
      .filter((wall) => wall.doorState !== "NONE")
      .map((wall) => doorInteractive(wall, gm));
  },
  tokenStatus: () => [],
  tokenAttributes: () => [],
  tokenGrid: () => [],
  tokenVision: () => [],

  // Areas with files of their own, under `handlers/`.
  ...loreQueries,
  ...actorAccessQueries,
  ...catchUpQueries,
  ...combatQueries,
};

export const mutations: Record<string, Handler> = {
  heartbeat: () => true,

  launchScene: ({ sceneId }) => {
    const state = demoState();
    if (!viewerIsGm(state)) {
      throw new GraphQLError("Only the DM (Owner or GM) may launch a scene");
    }
    scene(state, sceneId);
    state.world.activeSceneId = sceneId;
    record(EVENT.sceneLaunched, { sceneId });
    markChanged();
    return state.world;
  },

  // `send_chat_message_impl`: the body's rules, a GM-only message from the
  // GM alone, and the sender's own name as its author.
  sendChatMessage: ({ input }) => {
    const state = demoState();
    const body = chatBody(input.body);
    const gmOnly = input.gmOnly ?? false;
    if (gmOnly && !viewerIsGm(state)) {
      throw new GraphQLError("Only the GM may send a GM-only message");
    }
    const author = viewerUser(state);
    const message: Row = {
      id: crypto.randomUUID(),
      worldId: input.worldId,
      sceneId: input.sceneId ?? null,
      authorUserId: author.id,
      authorLabel: author.username,
      body,
      gmOnly,
      createdAt: now(),
    };
    demoState().chat.push(message);
    record(EVENT.chat, { message_id: message.id, scene_id: message.sceneId });
    markChanged();
    return message;
  },

  createWall: ({ input }) => {
    const at = now();
    return walls.create({
      wallId: crypto.randomUUID(),
      sceneId: input.sceneId,
      levelId: levelFor(demoState(), input.sceneId, input.levelId),
      x1: input.x1,
      y1: input.y1,
      x2: input.x2,
      y2: input.y2,
      blocksVision: input.blocksVision ?? true,
      blocksMovement: input.blocksMovement ?? true,
      doorState: input.doorState ?? "NONE",
      locked: false,
      secret: false,
      metadata: input.metadata ?? null,
      ...by,
      createdAt: at,
      updatedAt: at,
    });
  },
  updateWall: ({ wallId, input }) => walls.update(wallId, given(input)),
  deleteWall: ({ wallId }) => walls.remove(wallId),
  setDoorDesignation: ({ wallId, isDoor }) => {
    // A newly designated door starts closed: a wall that became a hole the
    // moment it became a door would change what the room does.
    changeDoor(wallId, {
      doorState: isDoor ? "CLOSED" : "NONE",
      locked: false,
      secret: false,
    });
    return true;
  },
  setDoorLock: ({ wallId, locked }) => {
    changeDoor(wallId, { locked });
    return true;
  },
  setDoorSecret: ({ wallId, secret }) => {
    changeDoor(wallId, { secret });
    return true;
  },
  activateInteractive: ({ interactiveId }) => {
    const wall = demoState().walls.find(
      (candidate) =>
        candidate.doorState !== "NONE" &&
        doorInteractiveId(candidate.wallId as string) === interactiveId,
    );
    if (!wall) notFound("Interactive");
    // A lock stops a player and not the Game Master, as the server's
    // `activation_outcome` decides it (spec 071 FR-010).
    if (wall.locked && !viewerIsGm(demoState())) {
      return {
        outcome: "refused",
        reason: "locked",
        requestId: null,
        effectId: null,
        effectConfig: null,
        notices: [],
      };
    }
    const next = wall.doorState === "OPEN" ? "CLOSED" : "OPEN";
    changeDoor(wall.wallId as string, { doorState: next });
    return {
      outcome: "performed",
      reason: null,
      requestId: null,
      effectId: "door.set_state",
      effectConfig: { target: wall.wallId, state: next.toLowerCase() },
      notices: [],
    };
  },

  createLightSource: ({ input }) => {
    const at = now();
    return lights.create({
      lightId: crypto.randomUUID(),
      sceneId: input.sceneId,
      levelId: levelFor(demoState(), input.sceneId, input.levelId),
      x: input.x,
      y: input.y,
      radius: input.radius,
      brightRadius: input.brightRadius ?? input.radius * 0.5,
      intensity: input.intensity ?? 1,
      color: input.color ?? "#ffffff",
      attachedTokenId: input.attachedTokenId ?? null,
      castsShadows: input.castsShadows ?? true,
      metadata: input.metadata ?? null,
      ...by,
      createdAt: at,
      updatedAt: at,
    });
  },
  updateLightSource: ({ lightId, input }) =>
    lights.update(lightId, given(input)),
  deleteLightSource: ({ lightId }) => lights.remove(lightId),

  createToken: ({ input }) => {
    const state = demoState();
    const actor = input.actorId ? findActor(state, input.actorId) : null;
    return tokens.create(
      newToken({
        tokenId: crypto.randomUUID(),
        sceneId: input.sceneId,
        levelId: levelFor(state, input.sceneId, input.levelId),
        x: input.x,
        y: input.y,
        at: now(),
        rest: {
          ...(actor ? tokenOf(actor, input.metadata?.label) : {}),
          ...given({
            rotation: input.rotation,
            scale: input.scale,
            metadata: input.metadata,
            tokenType: input.tokenType,
            linked: input.linked,
          }),
        },
      }),
    );
  },
  updateToken: ({ tokenId, input }) => tokens.update(tokenId, given(input)),
  moveOwnToken: ({ tokenId, x, y }) => tokens.update(tokenId, { x, y }),
  deleteToken: ({ tokenId }) => tokens.remove(tokenId),
  setTokenNameVisibility: ({ tokenId, visible }) =>
    tokens.update(tokenId, { nameVisibleToPlayers: visible }),
  setTokenLink: ({ tokenId, linked }) => tokens.update(tokenId, { linked }),

  // The cast and their sheets. Only the data moves; no world event is
  // announced, as the real server announces none for a sheet edit either.
  updateActor: ({ input: { actorId, ...fields } }) => {
    const state = demoState();
    const actor = findActor(state, actorId);
    Object.assign(actor, given(fields), { updatedAt: now() });
    markChanged();
    return actorRow(state, actor);
  },
  setActorVisibleToPlayers: ({ actorId, visible }) => {
    const state = demoState();
    const actor = findActor(state, actorId);
    actor.visibleToPlayers = visible;
    actor.updatedAt = now();
    markChanged();
    return actorRow(state, actor);
  },
  setActorUnique: ({ actorId, unique }) => {
    const state = demoState();
    const actor = findActor(state, actorId);
    actor.isUnique = unique;
    actor.updatedAt = now();
    markChanged();
    return actorRow(state, actor);
  },
  updateActorSystemData: ({ input }) => updateSystemData(input),
  claimActor: ({ actorId }) => claim(demoState(), actorId),
  unclaimActor: ({ actorId }) => {
    const state = demoState();
    const actor = findActor(state, actorId);
    if (state.claimedActorId === actorId) state.claimedActorId = null;
    markChanged();
    return actorRow(state, actor);
  },
  rollCheck: (args) => rollCheck(args),

  ...loreMutations,
  ...worldMutations,
  ...actorAccessMutations,
  ...combatMutations,
};

// The areas kept in `handlers/`, one file each.
Object.assign(queries, areaQueries);
Object.assign(mutations, areaMutations);
