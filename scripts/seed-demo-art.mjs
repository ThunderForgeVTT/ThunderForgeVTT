#!/usr/bin/env node
/**
 * Portraits and tokens for the demo world's heroes — playtest 2026-09-10 P8.
 *
 * `make seed` writes the demo party as rows, but images live in object
 * storage, which SQL cannot reach. So this signs in as the demo Game Master
 * and uploads each hero's art through the same `uploadActorImage` mutation
 * the NPC editor uses: SVG drawn by `packages/heroes`, rasterised to WebP by
 * the server on the way in, exactly as anyone's upload would be.
 *
 * Idempotent: an actor that already has a portrait or a token keeps it, so a
 * Game Master who replaced one keeps their choice. A stack that was never
 * seeded — no demo login, or no demo world — is skipped with a note, not an
 * error.
 *
 *   node scripts/seed-demo-art.mjs [api-url]   # default http://127.0.0.1:30000/api
 *
 * `make dev` runs it once the backend is ready.
 */
import { pathToFileURL } from "node:url";
import { createHero, PRESET_HEROES } from "../packages/heroes/src/index.ts";

const DEFAULT_API = "http://127.0.0.1:30000/api";
const DEMO_WORLD = "00000000-0000-0000-0000-0000000000b0";
const DEMO_GM = { identifier: "user1", password: "user1" };

/** Which seeded actor wears which hero — see `src/server/seeds/demo_accounts.sql`. */
const PARTY = {
  "00000000-0000-0000-0000-0000000000c1": "sir-pip",
  "00000000-0000-0000-0000-0000000000c2": "mira-starweave",
  "00000000-0000-0000-0000-00000000f001": "nettle",
  "00000000-0000-0000-0000-00000000f002": "brother-oak",
  "00000000-0000-0000-0000-00000000f003": "fenna-swiftbow",
  "00000000-0000-0000-0000-00000000f004": "lark",
  "00000000-0000-0000-0000-00000000f005": "grom",
  "00000000-0000-0000-0000-00000000f006": "willow",
  "00000000-0000-0000-0000-00000000f007": "dame-aurora",
  "00000000-0000-0000-0000-00000000f008": "kai",
  "00000000-0000-0000-0000-00000000f009": "ember",
  "00000000-0000-0000-0000-00000000f010": "tink-cogsworth",
};

const UPLOAD = `mutation ($actorId: UUID!, $role: String!, $file: Upload!) {
  uploadActorImage(actorId: $actorId, role: $role, file: $file) { assetId }
}`;

/** A session as a script holds one: the cookies login set, and the CSRF
 * token every GraphQL POST has to echo back (the double-submit check). */
async function signIn(api) {
  const response = await fetch(`${api}/authentication/login`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(DEMO_GM),
  });
  if (!response.ok) return null;
  const cookies = new Map();
  for (const header of response.headers.getSetCookie()) {
    const [pair] = header.split(";");
    const eq = pair.indexOf("=");
    cookies.set(pair.slice(0, eq).trim(), pair.slice(eq + 1).trim());
  }
  const csrf = cookies.get("csrf_token");
  if (!csrf) throw new Error("signed in, but no CSRF token came with the session");
  return {
    headers: {
      cookie: [...cookies].map(([name, value]) => `${name}=${value}`).join("; "),
      "x-csrf-token": csrf,
    },
  };
}

async function answer(response) {
  const body = await response.json();
  if (body.errors?.length) {
    throw new Error(body.errors.map((error) => error.message).join("; "));
  }
  return body.data;
}

async function graphql(api, session, query, variables) {
  const response = await fetch(`${api}/graphql`, {
    method: "POST",
    headers: { ...session.headers, "Content-Type": "application/json" },
    body: JSON.stringify({ query, variables }),
  });
  return answer(response);
}

/** The GraphQL multipart request spec, as the web's own upload sends it. */
async function uploadSvg(api, session, actorId, role, svg, filename) {
  const form = new FormData();
  form.append(
    "operations",
    JSON.stringify({ query: UPLOAD, variables: { actorId, role, file: null } }),
  );
  form.append("map", JSON.stringify({ 0: ["variables.file"] }));
  form.append("0", new Blob([svg], { type: "image/svg+xml" }), filename);
  const response = await fetch(`${api}/graphql`, {
    method: "POST",
    headers: session.headers,
    body: form,
  });
  return answer(response);
}

export async function seedDemoArt({
  api = DEFAULT_API,
  log = (message) => console.log(message),
} = {}) {
  const session = await signIn(api);
  if (!session) {
    log("no demo login to sign in with (run `make seed`); skipping demo art");
    return { uploaded: 0 };
  }

  let actors;
  try {
    const data = await graphql(
      api,
      session,
      `query ($worldId: UUID!) { worldActors(worldId: $worldId) { id images { role } } }`,
      { worldId: DEMO_WORLD },
    );
    actors = data.worldActors;
  } catch (error) {
    log(`no demo world (${error.message}); skipping demo art`);
    return { uploaded: 0 };
  }

  const specs = new Map(PRESET_HEROES.map((preset) => [preset.slug, preset.spec]));
  let uploaded = 0;
  for (const actor of actors) {
    const slug = PARTY[actor.id];
    const spec = slug && specs.get(slug);
    if (!spec) continue;
    const hero = createHero(spec);
    const has = new Set(actor.images.map((image) => image.role));
    for (const role of ["portrait", "token"]) {
      if (has.has(role)) continue;
      const svg = role === "portrait" ? hero.portrait() : hero.token();
      try {
        await uploadSvg(api, session, actor.id, role, svg, `${slug}-${role}.svg`);
        uploaded += 1;
      } catch (error) {
        log(`${slug} ${role}: ${error.message}`);
      }
    }
  }

  log(
    uploaded > 0
      ? `uploaded ${uploaded} demo portraits and tokens`
      : "demo art already in place",
  );
  return { uploaded };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  seedDemoArt({ api: process.argv[2] ?? DEFAULT_API }).catch((error) => {
    console.error(error);
    process.exit(1);
  });
}
