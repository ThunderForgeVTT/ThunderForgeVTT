import {
  expect,
  type Browser,
  type Page,
  type TestInfo,
} from "@playwright/test";
import {
  freshCredentials,
  graphql,
  launchSceneByName,
  register,
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "../e2e/fixtures/helpers";
import {
  dragToken,
  serverTokenPosition,
  tokenPosition,
} from "../e2e/fixtures/offline";
import { createScene } from "../e2e/fixtures/world-cache";

/**
 * A table: one Game Master, some players, one world, one scene — driven the
 * way a session is, and recorded so a person can watch it back.
 *
 * Everything here goes through what the product already exposes: GraphQL for
 * what a Game Master builds, the keyboard for how a player walks, and the dev
 * probes (`__worldProbe`, `__engineProbe`) for what each client ended up
 * believing. Nothing is faked on the server and nothing is injected into the
 * engine, so what a playtest finds is what a table would find.
 *
 * The Game Master is the scenario's author, not a stand-in for one. A script
 * decides what the Game Master builds and when; nothing here decides anything
 * a Game Master would decide at a real table.
 */

export type SystemId = "genie" | "dnd5e";
export type Point = { x: number; y: number };
export type Step = "north" | "south" | "east" | "west";
export type Ambient = "bright" | "dim" | "dark";

/** WASD walks the token a player owns by one cell (`token_move.rs`). */
const KEY: Record<Step, string> = {
  north: "w",
  south: "s",
  east: "d",
  west: "a",
};

const VIEWPORT = { width: 1280, height: 800 };

export interface Seat {
  name: string;
  page: Page;
  userId: string;
}

export interface Table {
  gm: Page;
  worldId: string;
  sceneId: string;
  system: SystemId;
  players: Seat[];
  testInfo: TestInfo;
  /** The clients that have opened Play, in the order they sat down. */
  seated: Page[];
}

type Counts = { tokens: number; walls: number; lights: number; shapes: number };

/** A GraphQL call that fails loudly, naming the operation that was refused. */
export async function must<T>(
  page: Page,
  query: string,
  variables: Record<string, unknown>,
): Promise<T> {
  const result = await graphql<{ data?: T; errors?: { message: string }[] }>(
    page,
    query,
    variables,
  );
  if (result.errors?.length || !result.data) {
    const operation = query.trim().split("\n")[0];
    throw new Error(
      `${operation} was refused: ${JSON.stringify(result.errors ?? result)}`,
    );
  }
  return result.data;
}

/**
 * Registers a Game Master, builds a world of `system` with one visible scene,
 * launches it, and seats each named player in a context of their own.
 *
 * Players are opened here rather than with `inviteAndJoinAsPlayer` because
 * that helper's contexts are not recorded, and a playtest nobody can watch is
 * half a playtest. The Game Master is the test's own `page`, which the config
 * records.
 */
export async function openTable(options: {
  browser: Browser;
  gm: Page;
  testInfo: TestInfo;
  system: SystemId;
  players: string[];
  sceneName?: string;
}): Promise<Table> {
  const { browser, gm, testInfo, system } = options;
  const sceneName = options.sceneName ?? "The Crypt";

  const worldId = await registerAndCreateWorld(
    gm,
    `Playtest ${system} ${uniqueSuffix()}`,
    "ptgm",
  );
  await must(
    gm,
    `mutation ($input: UpdateWorldGameSystemInput!) {
      updateWorldGameSystem(input: $input) { id }
    }`,
    { input: { worldId, gameSystemId: system } },
  );
  const sceneId = await createScene(gm, worldId, sceneName);
  await must(
    gm,
    `mutation ($sceneId: UUID!) {
      updateSceneHidden(sceneId: $sceneId, hidden: false) { sceneId }
    }`,
    { sceneId },
  );
  await launchSceneByName(gm, worldId, sceneName);

  const { generateInviteCode } = await must<{
    generateInviteCode: { inviteCode: string };
  }>(
    gm,
    `mutation ($input: GenerateInviteCodeInput!) {
      generateInviteCode(input: $input) { inviteCode }
    }`,
    { input: { worldId, maxUses: 10 } },
  );

  const players: Seat[] = [];
  for (const name of options.players) {
    const context = await browser.newContext({
      viewport: VIEWPORT,
      recordVideo: {
        dir: testInfo.outputPath(`video-${name}`),
        size: VIEWPORT,
      },
    });
    const page = await context.newPage();
    await register(page, freshCredentials(`pt${name.toLowerCase()}`));
    await must(
      page,
      `mutation ($input: JoinWorldInput!) { joinWorld(input: $input) { id } }`,
      { input: { inviteCode: generateInviteCode.inviteCode } },
    );
    const { me } = await must<{ me: { id: string } }>(
      page,
      `query { me { id } }`,
      {},
    );
    players.push({ name, page, userId: me.id });
  }

  return { gm, worldId, sceneId, system, players, testInfo, seated: [] };
}

/**
 * An actor of the table's game system and a token for it. Given a seat, the
 * token is that player's own and their primary, which is what makes it both
 * the token their keyboard walks and the eyes their canvas sees through.
 */
export async function placeCharacter(
  table: Table,
  options: {
    label: string;
    at: Point;
    seat?: Seat;
    tokenType?: "character" | "npc";
  },
): Promise<string> {
  return (await placeCast(table, options)).tokenId;
}

/**
 * The same, returning the actor as well as the token.
 *
 * Anything about a character rather than a piece — hit points, abilities, a
 * game system's data — hangs off the actor, so a scenario that touches those
 * needs both ids. `placeCharacter` is this with the actor dropped.
 */
export async function placeCast(
  table: Table,
  options: {
    label: string;
    at: Point;
    seat?: Seat;
    tokenType?: "character" | "npc";
    /** Sizes the piece: 2 is an ogre, filling four squares of the grid. */
    scale?: number;
    /**
     * The creature's sheet, written before its token is placed.
     *
     * Spec 046 (ADR-102): an NPC's token is an unlinked copy, and a copy
     * starts from its NPC's sheet *as it is when placed* — so a goblin whose
     * hit points are written after it is on the board is a goblin with none.
     * That is the rule, not a harness quirk: a Game Master fills in the
     * goblin, then drops goblins.
     */
    sheet?: {
      scores: Record<string, number>;
      hitPoints: { current: number; max: number };
    };
  },
): Promise<{ actorId: string; tokenId: string }> {
  const { createActor } = await must<{ createActor: { id: string } }>(
    table.gm,
    `mutation ($input: CreateActorInput!) { createActor(input: $input) { id } }`,
    {
      input: {
        worldId: table.worldId,
        label: options.label,
        isNpc: !options.seat,
        gameSystemId: table.system,
      },
    },
  );
  if (options.sheet) {
    // Scores first: the 5e pack refuses anything else on a sheet without them.
    for (const [dataType, data] of [
      ["ability_data", options.sheet.scores],
      [
        "resource_data",
        {
          current_hp: options.sheet.hitPoints.current,
          max_hp: options.sheet.hitPoints.max,
          temporary_hp: 0,
        },
      ],
    ] as const) {
      await must(
        table.gm,
        `mutation ($input: GraphQLUpdateActorSystemDataInput!) {
          updateActorSystemData(input: $input) { id }
        }`,
        {
          input: {
            actorId: createActor.id,
            gameSystemId: table.system,
            dataType,
            data,
          },
        },
      );
    }
  }
  const { tokenId } = await placeToken(table, createActor.id, {
    at: options.at,
    tokenType: options.tokenType ?? (options.seat ? "character" : "npc"),
    scale: options.scale,
  });
  if (options.seat) {
    await must(
      table.gm,
      `mutation ($tokenId: UUID!, $input: GraphQLUpdateTokenInput!) {
        updateToken(tokenId: $tokenId, input: $input) { tokenId }
      }`,
      {
        tokenId,
        input: { ownerUserId: options.seat.userId, isPrimary: true },
      },
    );
  }
  return { actorId: createActor.id, tokenId };
}

/**
 * Place another token of an actor already in the world — another goblin of the
 * goblin NPC. The server decides whether it is linked or a copy (spec 046
 * FR-016) unless `linked` says otherwise.
 */
export async function placeToken(
  table: Table,
  actorId: string,
  options: {
    at: Point;
    tokenType?: "character" | "npc";
    scale?: number;
    label?: string;
    linked?: boolean;
  },
): Promise<{ tokenId: string; linked: boolean }> {
  const { createToken } = await must<{
    createToken: { tokenId: string; linked: boolean };
  }>(
    table.gm,
    `mutation ($input: GraphQLCreateTokenInput!) {
      createToken(input: $input) { tokenId linked }
    }`,
    {
      input: {
        sceneId: table.sceneId,
        actorId,
        x: options.at.x,
        y: options.at.y,
        ...(options.tokenType === undefined
          ? {}
          : { tokenType: options.tokenType }),
        ...(options.scale === undefined ? {} : { scale: options.scale }),
        ...(options.label === undefined
          ? {}
          : { metadata: { label: options.label } }),
        ...(options.linked === undefined ? {} : { linked: options.linked }),
      },
    },
  );
  return createToken;
}

export async function addWall(
  table: Table,
  from: Point,
  to: Point,
  { blocksVision = true, blocksMovement = true } = {},
): Promise<string> {
  const { createWall } = await must<{ createWall: { wallId: string } }>(
    table.gm,
    `mutation ($input: GraphQLCreateWallInput!) {
      createWall(input: $input) { wallId }
    }`,
    {
      input: {
        sceneId: table.sceneId,
        x1: from.x,
        y1: from.y,
        x2: to.x,
        y2: to.y,
        blocksVision,
        blocksMovement,
      },
    },
  );
  return createWall.wallId;
}

export async function addLight(
  table: Table,
  at: Point,
  radius: number,
  { attachedTokenId }: { attachedTokenId?: string } = {},
): Promise<string> {
  const { createLightSource } = await must<{
    createLightSource: { lightId: string };
  }>(
    table.gm,
    `mutation ($input: GraphQLCreateLightSourceInput!) {
      createLightSource(input: $input) { lightId }
    }`,
    {
      input: {
        sceneId: table.sceneId,
        x: at.x,
        y: at.y,
        radius,
        intensity: 1,
        castsShadows: true,
        ...(attachedTokenId ? { attachedTokenId } : {}),
      },
    },
  );
  return createLightSource.lightId;
}

export async function setAmbient(table: Table, level: Ambient): Promise<void> {
  await must(
    table.gm,
    `mutation ($sceneId: UUID!, $level: String!) {
      updateSceneAmbientLight(sceneId: $sceneId, ambientLight: $level) { sceneId }
    }`,
    { sceneId: table.sceneId, level },
  );
}

/** Makes a wall a door (it starts closed) and returns its interactive's id. */
export async function makeDoor(table: Table, wallId: string): Promise<string> {
  await must(
    table.gm,
    `mutation ($wallId: UUID!, $isDoor: Boolean!) {
      setDoorDesignation(wallId: $wallId, isDoor: $isDoor)
    }`,
    { wallId, isDoor: true },
  );
  const { interactives } = await must<{
    interactives: { interactiveId: string; subjectRef: string }[];
  }>(
    table.gm,
    `query ($sceneId: UUID!) {
      interactives(sceneId: $sceneId) { interactiveId subjectRef }
    }`,
    { sceneId: table.sceneId },
  );
  const door = interactives.find((i) => i.subjectRef === wallId);
  if (!door)
    throw new Error(`wall ${wallId} became a door with no interactive`);
  return door.interactiveId;
}

/** A player (or the Game Master) uses an interactive; returns its outcome. */
export async function activate(
  page: Page,
  interactiveId: string,
): Promise<string> {
  const { activateInteractive } = await must<{
    activateInteractive: { outcome: string };
  }>(
    page,
    `mutation ($id: UUID!) {
      activateInteractive(interactiveId: $id) { outcome }
    }`,
    { id: interactiveId },
  );
  return activateInteractive.outcome;
}

/**
 * Uses an interactive the way a click on the canvas does: through the
 * client's own `activateAndApply`, which asks the server and, when it
 * performs, applies the effect to that client's engine at once.
 *
 * `activate` above goes to the server directly, which is what a *different*
 * client's view of the change looks like. The two together are how a
 * playtest tells "the person who opened the door sees it open" apart from
 * "everyone sees it open".
 */
export async function activateAsPlayer(
  page: Page,
  interactiveId: string,
): Promise<string> {
  return page.evaluate(async (id) => {
    const sync = (await import(
      /* @vite-ignore */ "/src/engine/world/sync/interactives.ts"
    )) as typeof import("../src/engine/world/sync/interactives");
    const bevy = (await import(
      /* @vite-ignore */ "/src/engine/bevy/index.ts"
    )) as typeof import("../src/engine/bevy/index");
    const store = bevy.getBoundWorldStore();
    if (!store) throw new Error("this client has no world store bound");
    const result = await sync.activateAndApply(store, id);
    return result.outcome;
  }, interactiveId);
}

/**
 * What a client's own board believes about a door: `"open"`, `"closed"`, or
 * `null` when it holds no such wall. Read from the store the engine is fed
 * from, without re-reading anything from the server — re-reading is exactly
 * the step whose absence this is meant to catch.
 */
export async function doorStateOn(
  page: Page,
  wallId: string,
): Promise<string | null> {
  return page.evaluate(async (id) => {
    const bevy = (await import(
      /* @vite-ignore */ "/src/engine/bevy/index.ts"
    )) as typeof import("../src/engine/bevy/index");
    const wall = bevy.getBoundWorldStore()?.getState().walls[id] as
      | { doorState?: string }
      | undefined;
    return wall?.doorState ?? null;
  }, wallId);
}

/** Opens Play for a client and waits for its engine to be up. */
export async function sitDown(table: Table, page: Page): Promise<void> {
  await page.goto(`/world/${table.worldId}/play`);
  await waitForEngineReady(page);
  if (!table.seated.includes(page)) table.seated.push(page);
}

/**
 * Another player arrives after the session has started, with no token.
 *
 * Their own browser, registered and joined like everyone else — the point is
 * that they have nothing to see through, not that they are a stand-in. Added
 * to `table.players` so `clients()` includes them in snapshots and
 * `closeTable` tidies their context and recording away.
 */
export async function joinLate(table: Table, name: string): Promise<Seat> {
  const { generateInviteCode } = await must<{
    generateInviteCode: { inviteCode: string };
  }>(
    table.gm,
    `mutation ($input: GenerateInviteCodeInput!) {
      generateInviteCode(input: $input) { inviteCode }
    }`,
    { input: { worldId: table.worldId, maxUses: 10 } },
  );

  const context = await table.gm
    .context()
    .browser()!
    .newContext({
      viewport: VIEWPORT,
      recordVideo: {
        dir: table.testInfo.outputPath(`video-${name}`),
        size: VIEWPORT,
      },
    });
  const page = await context.newPage();
  await register(page, freshCredentials(`pt${name.toLowerCase()}`));
  await must(
    page,
    `mutation ($input: JoinWorldInput!) { joinWorld(input: $input) { id } }`,
    { input: { inviteCode: generateInviteCode.inviteCode } },
  );
  const { me } = await must<{ me: { id: string } }>(
    page,
    `query { me { id } }`,
    {},
  );

  const seat: Seat = { name, page, userId: me.id };
  table.players.push(seat);
  return seat;
}

/** Zooms a client's camera out a few notches, so a screenshot shows the room. */
export async function overview(page: Page, notches = 4): Promise<void> {
  const canvas = page.locator("canvas");
  const box = await canvas.boundingBox();
  if (!box) return;
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  for (let notch = 0; notch < notches; notch += 1) {
    await page.mouse.wheel(0, 120);
    await page.waitForTimeout(120);
  }
}

/**
 * What this client's engine believes about moving: the token named as its
 * player's, whether it has been found, how many are tagged, and whether the
 * scene has a grid. Worth naming in a failure — a key that moves nothing has
 * several causes and they look identical from outside.
 */
export async function movementStateOn(page: Page): Promise<string> {
  const state = await page.evaluate(
    () =>
      (
        window as unknown as {
          __engineProbe?: { movementState?: () => unknown };
        }
      ).__engineProbe?.movementState?.() ?? null,
  );
  return JSON.stringify(state);
}

/** The tokens this client's canvas is hiding from its viewer. */
export async function hiddenTokens(page: Page): Promise<string[]> {
  return page.evaluate(
    () =>
      (
        window as unknown as {
          __engineProbe?: { hiddenTokens?: () => string[] };
        }
      ).__engineProbe?.hiddenTokens?.() ?? [],
  );
}

/**
 * The tokens a Game Master's board marks because at least one player cannot
 * see them (spec 045 FR-033).
 *
 * Always empty on a player's board: a player is told what their own token can
 * see, never what anybody else's can.
 */
export async function markedTokens(page: Page): Promise<string[]> {
  return page.evaluate(
    () =>
      (
        window as unknown as {
          __engineProbe?: { markedTokens?: () => string[] };
        }
      ).__engineProbe?.markedTokens?.() ?? [],
  );
}

export async function storeCounts(page: Page): Promise<Counts | null> {
  return page.evaluate(
    () => (window.__worldProbe?.state()?.counts ?? null) as Counts | null,
  );
}

/** Every client at the table has loaded at least this much of the scene. */
export async function expectEveryoneLoaded(
  table: Table,
  expected: Partial<Counts>,
): Promise<void> {
  for (const [who, page] of clients(table)) {
    await expect
      .poll(() => storeCounts(page), {
        timeout: 20_000,
        message: `${who}'s client loads the scene`,
      })
      .toMatchObject(expected);
  }
}

/**
 * The clients at the table that have sat down. A player still on the welcome
 * page has no world loaded, so waiting for them to agree would wait forever —
 * which is how the first run of this suite failed before anyone had moved.
 */
export function clients(table: Table): [string, Page][] {
  const everyone: [string, Page][] = [
    ["Game Master", table.gm],
    ...table.players.map((seat): [string, Page] => [seat.name, seat.page]),
  ];
  return everyone.filter(([, page]) => table.seated.includes(page));
}

/**
 * A player drags their own token by a world-space delta.
 *
 * `dragToken` aims at the default 1:1 camera centred on the origin, which is
 * why only the Game Master's view is ever zoomed out: a player who zoomed
 * would be pressing where their token is not.
 */
export async function drag(
  seat: Seat,
  tokenId: string,
  by: Point,
): Promise<void> {
  // Screen y points down; world y points up.
  await dragToken(seat.page, tokenId, { dx: by.x, dy: -by.y });
}

/**
 * A drag that is allowed to be refused, returning whether the token moved.
 *
 * `drag` insists the token move, because a press that grabbed nothing is the
 * worst failure this harness can have — every convergence check afterwards
 * would compare a position with itself and pass. But since spec 045 a drag
 * across a wall is *supposed* to leave the token where it was, and the two
 * outcomes look identical from outside. Use this where the question is
 * whether the move was allowed; use `drag` everywhere else, so a silently
 * missed grab still fails loudly.
 */
export async function tryDrag(
  seat: Seat,
  tokenId: string,
  by: Point,
): Promise<boolean> {
  return dragToken(
    seat.page,
    tokenId,
    { dx: by.x, dy: -by.y },
    { mayBeRefused: true },
  );
}

/** A player presses a movement key, one press per cell (`token_move.rs`). */
export async function walk(seat: Seat, step: Step, times = 1): Promise<void> {
  for (let press = 0; press < times; press += 1) {
    await seat.page.keyboard.press(KEY[step]);
    await seat.page.waitForTimeout(300);
  }
}

/**
 * Waits until every client and the server agree where a token is, and returns
 * where that is. Disagreement is reported as the whole table's view, so a
 * failure says who was left behind.
 */
export async function expectAgreed(
  table: Table,
  tokenId: string,
  message: string,
): Promise<Point> {
  let agreed: Point | null = null;
  await expect
    .poll(
      async () => {
        const views: [string, Point | null][] = [];
        for (const [who, page] of clients(table)) {
          views.push([who, await tokenPosition(page, tokenId)]);
        }
        views.push([
          "server",
          await serverTokenPosition(table.gm, table.sceneId, tokenId),
        ]);
        const [, first] = views[0];
        const same =
          first !== null &&
          views.every(
            ([, p]) =>
              p !== null &&
              Math.abs(p.x - first.x) < 0.5 &&
              Math.abs(p.y - first.y) < 0.5,
          );
        if (same) {
          agreed = { x: first.x, y: first.y };
          return "agreed";
        }
        return JSON.stringify(Object.fromEntries(views));
      },
      { timeout: 15_000, intervals: [250], message },
    )
    .toBe("agreed");
  return agreed!;
}

/** One screenshot per client, attached to the report under `label`. */
export async function snapshot(table: Table, label: string): Promise<void> {
  for (const [who, page] of clients(table)) {
    await table.testInfo.attach(`${label} — ${who}`, {
      body: await page.screenshot(),
      contentType: "image/png",
    });
  }
}

/** Closes the players' contexts and attaches their recordings. */
export async function closeTable(table: Table): Promise<void> {
  for (const seat of table.players) {
    const video = seat.page.video();
    await seat.page.context().close();
    if (video) {
      await table.testInfo.attach(`recording — ${seat.name}`, {
        path: await video.path(),
        contentType: "video/webm",
      });
    }
  }
}

/** A small seeded generator (mulberry32), so a wandering run can be replayed. */
export function seededRandom(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** Whether the move from `a` to `b` passes through the wall from `c` to `d`. */
export function crosses(a: Point, b: Point, c: Point, d: Point): boolean {
  const side = (p: Point, q: Point, r: Point) =>
    Math.sign((q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x));
  return side(a, b, c) * side(a, b, d) < 0 && side(c, d, a) * side(c, d, b) < 0;
}
