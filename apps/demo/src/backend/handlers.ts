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
import { DEMO_USER, newToken } from "../seed/world";
import { EVENT, now, record } from "./events";
import { demoState, markChanged, type DemoState, type Row } from "./state";

type Args = Record<string, any>; // eslint-disable-line @typescript-eslint/no-explicit-any
type Handler = (args: Args) => unknown;

const by = { createdBy: DEMO_USER.id, updatedBy: DEMO_USER.id };

function notFound(what: string): never {
  throw new GraphQLError(`${what} not found`);
}

function scene(state: DemoState, sceneId: string): Row {
  return state.scenes.find((s) => s.sceneId === sceneId) ?? notFound("Scene");
}

/** The level a thing lands on when the request names none: the entry level. */
function levelFor(state: DemoState, sceneId: string, levelId?: string): string {
  if (levelId) return levelId;
  const entry =
    state.levels.find((l) => l.sceneId === sceneId && l.isEntry) ??
    state.levels.find((l) => l.sceneId === sceneId);
  return (entry?.levelId as string | undefined) ?? notFound("Scene");
}

function onLevel(rows: Row[], args: Args): Row[] {
  return rows.filter(
    (row) =>
      row.sceneId === args.sceneId &&
      (args.levelId == null || row.levelId === args.levelId),
  );
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
  return state.world;
}

function member(state: DemoState): Row {
  return {
    id: state.world.id,
    worldId: state.world.id,
    userId: DEMO_USER.id,
    username: DEMO_USER.username,
    role: "Owner",
    joinedAt: state.world.createdAt,
    createdAt: state.world.createdAt,
    updatedAt: state.world.createdAt,
    claimedActor: null,
  };
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
const shapes = kind((s) => s.shapes, "shapeId", "shape_id", EVENT.shape);
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

function doorInteractive(wall: Row): Row {
  return {
    interactiveId: doorInteractiveId(wall.wallId as string),
    sceneId: wall.sceneId,
    levelId: wall.levelId,
    subjectKind: "door",
    subjectRef: wall.wallId,
    geometry: null,
    trigger: "click",
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

export const queries: Record<string, Handler> = {
  // The instance, as a signed-out visitor is told about it.
  featureFlags: () => [],
  instanceRepositoryIntegration: () => ({
    configured: false,
    operatorGuidance: "Repository sync is not part of the demo.",
  }),

  // The world and who is in it.
  myWorldsWithRole: () => [{ role: "Owner", world: worldRow(demoState()) }],
  myLibrary: () => [],
  world: ({ id }) =>
    id === demoState().world.id ? worldRow(demoState()) : null,
  worldMembers: () => [member(demoState())],
  worldInvites: () => [],
  worldPlayState: () => ({ paused: false, pausedAt: null, history: [] }),
  worldStatistics: () => {
    const state = demoState();
    return {
      scenes: state.scenes.length,
      members: 1,
      membersWithCharacter: 0,
      characters: 0,
      npcs: 0,
      tokens: state.tokens.length,
      activeEncounter: null,
    };
  },
  worldActors: () => [],
  worldCollections: () => [],
  worldChatMessages: () => demoState().chat,
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
  authoringToolGrants: () => [],
  authoringTools: () => [
    "select",
    "walls",
    "lights",
    "shapes",
    "tokens",
    "interactions",
  ],
  pendingOffers: () => [],
  myActorClaim: () => null,
  peerSessions: () => [],
  loreEntry: ({ slug }) =>
    demoState().lore.find((entry) => entry.slug === slug) ?? null,
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
  tokens: (args) => onLevel(demoState().tokens, args),
  lightSources: (args) => onLevel(demoState().lights, args),
  shapes: (args) => onLevel(demoState().shapes, args),
  interactives: (args) =>
    onLevel(demoState().walls, args)
      .filter((wall) => wall.doorState !== "NONE")
      .map(doorInteractive),
  tokenStatus: () => [],
  tokenAttributes: () => [],
  tokenGrid: () => [],
  tokenVision: () => [],
};

export const mutations: Record<string, Handler> = {
  heartbeat: () => true,

  createLoreEntry: ({ input }) => {
    const at = now();
    const content = (input.content as string | undefined) ?? "";
    const entry: Row = {
      id: crypto.randomUUID(),
      worldId: input.worldId,
      title: input.title,
      slug: String(input.title)
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, "-")
        .replace(/^-|-$/g, ""),
      content,
      // Text, escaped. The server renders markdown; the demo does not need to
      // for what it stores, and must never hand back markup it did not write.
      renderedHtml: `<p>${content.replace(/[&<>"]/g, (c) => `&#${c.charCodeAt(0)};`)}</p>\n`,
      currentRevisionId: crypto.randomUUID(),
      myPermissionLevel: "OWNER",
      moderated: false,
      moderationCaseId: null,
      createdBy: DEMO_USER.id,
      createdAt: at,
      updatedAt: at,
      parentId: null,
      tags: [],
      linkedFrom: [],
    };
    demoState().lore.push(entry);
    markChanged();
    return entry;
  },

  launchScene: ({ sceneId }) => {
    const state = demoState();
    scene(state, sceneId);
    state.world.activeSceneId = sceneId;
    record(EVENT.sceneLaunched, { sceneId });
    markChanged();
    return state.world;
  },

  sendChatMessage: ({ input }) => {
    const message: Row = {
      id: crypto.randomUUID(),
      worldId: input.worldId,
      sceneId: input.sceneId ?? null,
      authorUserId: DEMO_USER.id,
      authorLabel: "Game Master",
      body: input.body,
      gmOnly: input.gmOnly ?? false,
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
    // The Game Master's hand: a lock is for players, and there are none.
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

  createShape: ({ input }) => {
    const at = now();
    return shapes.create({
      shapeId: crypto.randomUUID(),
      sceneId: input.sceneId,
      levelId: levelFor(demoState(), input.sceneId, input.levelId),
      kind: input.kind,
      geometry: input.geometry,
      text: input.text ?? null,
      style: input.style ?? null,
      visibleToPlayers: input.visibleToPlayers ?? true,
      metadata: input.metadata ?? null,
      ...by,
      createdAt: at,
      updatedAt: at,
    });
  },
  updateShape: ({ shapeId, input }) => shapes.update(shapeId, given(input)),
  deleteShape: ({ shapeId }) => shapes.remove(shapeId),

  createToken: ({ input }) =>
    tokens.create(
      newToken({
        tokenId: crypto.randomUUID(),
        sceneId: input.sceneId,
        levelId: levelFor(demoState(), input.sceneId, input.levelId),
        x: input.x,
        y: input.y,
        at: now(),
        rest: given({
          rotation: input.rotation,
          scale: input.scale,
          metadata: input.metadata,
          tokenType: input.tokenType,
          linked: input.linked,
        }),
      }),
    ),
  updateToken: ({ tokenId, input }) => tokens.update(tokenId, given(input)),
  moveOwnToken: ({ tokenId, x, y }) => tokens.update(tokenId, { x, y }),
  deleteToken: ({ tokenId }) => tokens.remove(tokenId),
  setTokenNameVisibility: ({ tokenId, visible }) =>
    tokens.update(tokenId, { nameVisibleToPlayers: visible }),
  setTokenLink: ({ tokenId, linked }) => tokens.update(tokenId, { linked }),
};
