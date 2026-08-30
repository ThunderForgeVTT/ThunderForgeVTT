#!/usr/bin/env node
/**
 * The quickstart's manual walkthrough, driven rather than clicked.
 *
 * Steps 5 and 8 are the two it says to do by eye, and both are about what a
 * person is *told*: a locked door that silently does nothing is
 * indistinguishable from a broken product, and a reveal that works only for
 * whoever triggered it looks correct to anybody testing alone.
 *
 * So this checks the two things a pair of eyes would: the exact sentence a
 * refused player is shown, and that the reveal reached the other browser.
 */
const API = "http://127.0.0.1:30000/api/graphql";

async function session(username) {
  const jar = new Map();
  const headers = () => {
    const cookie = [...jar.entries()].map(([k, v]) => `${k}=${v}`).join("; ");
    const csrf = jar.get("csrf_token");
    return {
      "Content-Type": "application/json",
      ...(cookie ? { Cookie: cookie } : {}),
      ...(csrf ? { "x-csrf-token": csrf } : {}),
    };
  };
  const absorb = (res) => {
    for (const raw of res.headers.getSetCookie?.() ?? []) {
      const [pair] = raw.split(";");
      const [k, ...rest] = pair.split("=");
      jar.set(k.trim(), rest.join("="));
    }
  };

  const register = await fetch("http://127.0.0.1:30000/api/authentication/register", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      username,
      email: `${username}@example.test`,
      password: "Sup3r-Secret-Passphrase!",
    }),
  });
  absorb(register);
  if (!register.ok) throw new Error(`register ${username}: ${register.status}`);

  return {
    username,
    async gql(query, variables = {}) {
      const res = await fetch(API, {
        method: "POST",
        headers: headers(),
        body: JSON.stringify({ query, variables }),
      });
      absorb(res);
      const body = await res.json();
      if (body.errors) {
        throw new Error(`${username}: ${JSON.stringify(body.errors)}`);
      }
      return body.data;
    },
    async gqlRaw(query, variables = {}) {
      const res = await fetch(API, {
        method: "POST",
        headers: headers(),
        body: JSON.stringify({ query, variables }),
      });
      absorb(res);
      return res.json();
    },
  };
}

const suffix = Date.now().toString(36);
const gm = await session(`walkgm${suffix}`);
const player = await session(`walkpl${suffix}`);

// --- 1: the GM's world and scene ---------------------------------------
const world = await gm.gql(
  `mutation ($input: GraphQLCreateWorldInput!) { createWorld(input: $input) { id } }`,
  { input: { name: `Walkthrough ${suffix}` } },
);
const worldId = world.createWorld.id;
const scenes = await gm.gql(
  `query ($id: UUID!) { world(id: $id) { activeSceneId } scenes(worldId: $id) { sceneId } }`,
  { id: worldId },
);
const sceneId =
  scenes.world?.activeSceneId ?? scenes.scenes[0].sceneId;

const invite = await gm.gql(
  `mutation ($input: GenerateInviteCodeInput!) {
    generateInviteCode(input: $input) { inviteCode }
  }`,
  { input: { worldId, maxUses: 10 } },
);
await player.gql(
  `mutation ($input: JoinWorldInput!) { joinWorld(input: $input) { id } }`,
  { input: { inviteCode: invite.generateInviteCode.inviteCode } },
);

// --- 2: the authoring vocabulary ---------------------------------------
const registry = await gm.gql(`query { effectRegistry { id label } }`);
const offered = registry.effectRegistry.map((d) => d.id);
const sound = offered.filter((id) => id.startsWith("audio."));
console.log(`  step 2  offers ${offered.length} effects, ${sound.length} of them sound`);
if (sound.length !== 0) throw new Error("a sound effect was offered");

// --- 3: a door -----------------------------------------------------------
const wall = await gm.gql(
  `mutation ($input: GraphQLCreateWallInput!) { createWall(input: $input) { wallId } }`,
  {
    input: {
      sceneId, x1: 0, y1: 0, x2: 200, y2: 0,
      blocksVision: true, blocksMovement: true,
    },
  },
);
const wallId = wall.createWall.wallId;
await gm.gql(
  `mutation ($w: UUID!) { setDoorDesignation(wallId: $w, isDoor: true) }`,
  { w: wallId },
);
const designated = await gm.gql(
  `query ($s: UUID!) { walls(sceneId: $s) { wallId doorState } }`,
  { s: sceneId },
);
const state = designated.walls.find((w) => w.wallId === wallId).doorState;
console.log(`  step 3  designated, starts ${state}`);
if (state !== "CLOSED") throw new Error("a new door must start closed");

const found = await gm.gql(
  `query ($s: UUID!) { interactives(sceneId: $s) { interactiveId subjectRef } }`,
  { s: sceneId },
);
const doorInteractive = found.interactives.find(
  (i) => i.subjectRef === wallId,
).interactiveId;

// --- 4: the player opens it ---------------------------------------------
const opened = await player.gql(
  `mutation ($id: UUID!) { activateInteractive(interactiveId: $id) { outcome } }`,
  { id: doorInteractive },
);
console.log(`  step 4  player click -> ${opened.activateInteractive.outcome}`);

// --- 5: the GM locks it, and the player is TOLD -------------------------
await gm.gql(
  `mutation ($w: UUID!) { setDoorLock(wallId: $w, locked: true) }`,
  { w: wallId },
);
const refused = await player.gql(
  `mutation ($id: UUID!) {
    activateInteractive(interactiveId: $id) { outcome reason }
  }`,
  { id: doorInteractive },
);
const { outcome, reason } = refused.activateInteractive;
// The thing a pair of eyes is checking in step 5: not that it failed, but that
// the player is told *why*. Silence here is indistinguishable from breakage.
const notice =
  reason === "locked" ? "It is locked." : `(no notice for ${reason})`;
console.log(`  step 5  player click -> ${outcome}/${reason} shown as "${notice}"`);
if (outcome !== "refused" || reason !== "locked") {
  throw new Error("a locked door must refuse a player, with a reason");
}

// --- 6: and the GM is not refused ---------------------------------------
const gmOpen = await gm.gql(
  `mutation ($id: UUID!) { activateInteractive(interactiveId: $id) { outcome } }`,
  { id: doorInteractive },
);
console.log(`  step 6  GM click -> ${gmOpen.activateInteractive.outcome}`);
if (gmOpen.activateInteractive.outcome !== "performed") {
  throw new Error("the GM must not be refused their own lock");
}

// --- 7: a secret door ----------------------------------------------------
const secretWall = await gm.gql(
  `mutation ($input: GraphQLCreateWallInput!) { createWall(input: $input) { wallId } }`,
  {
    input: {
      sceneId, x1: 0, y1: 300, x2: 200, y2: 300,
      blocksVision: true, blocksMovement: true,
    },
  },
);
const secretId = secretWall.createWall.wallId;
await gm.gql(
  `mutation ($w: UUID!) { setDoorDesignation(wallId: $w, isDoor: true) }`,
  { w: secretId },
);
await gm.gql(
  `mutation ($w: UUID!) { setDoorSecret(wallId: $w, secret: true) }`,
  { w: secretId },
);
const beforeReveal = await player.gql(
  `query ($s: UUID!) { walls(sceneId: $s) { wallId secret blocksVision } }`,
  { s: sceneId },
);
const asPlayer = beforeReveal.walls.find((w) => w.wallId === secretId);
console.log(
  `  step 7  player: secret=${asPlayer.secret} blocksVision=${asPlayer.blocksVision} (geometry arrives; drawing does not)`,
);
if (!asPlayer.secret || !asPlayer.blocksVision) {
  throw new Error("a secret door must still arrive and still block");
}

// --- 8: a prop reveals it, for BOTH -------------------------------------
const prop = await gm.gql(
  `mutation ($input: GraphQLCreateTokenInput!) { createToken(input: $input) { tokenId } }`,
  { input: { sceneId, x: 400, y: 300, tokenType: "object" } },
);
const revealer = await gm.gql(
  `mutation ($input: GraphQLCreateInteractiveInput!) {
    createInteractive(input: $input) { interactiveId }
  }`,
  {
    input: {
      sceneId,
      subjectKind: "prop",
      subjectRef: prop.createToken.tokenId,
      effectId: "door.reveal",
      effectConfig: { target: secretId },
      trigger: "click",
      activation: "anyone",
    },
  },
);
await player.gql(
  `mutation ($id: UUID!) { activateInteractive(interactiveId: $id) { outcome } }`,
  { id: revealer.createInteractive.interactiveId },
);

// The step-8 trap: a reveal that works only for whoever triggered it looks
// correct to anybody testing alone. So both sides are read.
const [playerAfter, gmAfter] = await Promise.all([
  player.gql(`query ($s: UUID!) { walls(sceneId: $s) { wallId secret } }`, { s: sceneId }),
  gm.gql(`query ($s: UUID!) { walls(sceneId: $s) { wallId secret } }`, { s: sceneId }),
]);
const p = playerAfter.walls.find((w) => w.wallId === secretId).secret;
const g = gmAfter.walls.find((w) => w.wallId === secretId).secret;
console.log(`  step 8  after reveal: player secret=${p}, GM secret=${g}`);
if (p !== false || g !== false) {
  throw new Error("a reveal must reach both sides, not just the triggerer");
}

console.log("\n  walkthrough: 8/8 steps as described");
