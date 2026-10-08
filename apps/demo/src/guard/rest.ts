/**
 * The handful of plain HTTP addresses the client reads outside GraphQL.
 *
 * Each is answered here from the page or from one of the demo's own static
 * files. An address that is not in this table is not part of the demo.
 */
import { demoState } from "../backend/state";
import { viewerUser } from "../backend/actors";
import { bytesOf } from "../backend/uploads";
import { drawArt, readArtPath } from "../seed/art";
import type { Viewer } from "../seed/world";

/** An SVG document drawn as PNG bytes (`rasterize.ts` in the page). */
export type DrawPng = (svg: string) => Promise<Blob>;

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

const SESSION_LENGTH_MS = 24 * 60 * 60 * 1000;

/**
 * `null` when the demo has no answer for this address. `viewer` is the asking
 * tab's (spec 081 R7): the world's own is whoever asked it last.
 */
export function answerRest(
  method: string,
  path: string,
  base: string,
  fetchStatic: typeof fetch,
  drawPng: DrawPng,
  viewer: Viewer,
): Promise<Response> | Response | null {
  const reads = method === "GET" || method === "HEAD";

  if (reads && path === "/api/authentication/session") {
    const state = demoState();
    const at = state.world.createdAt as string;
    const user = viewerUser({ ...state, viewer });
    return json({
      status: "authenticated",
      message: "This is the demo. Nobody is signed in to anything.",
      session: {
        authenticated: true,
        user: {
          id: user.id,
          username: user.username,
          email: user.email,
          role: "user",
          is_admin: false,
          created_at: at,
          updated_at: at,
        },
        session_expires_at: new Date(
          Date.now() + SESSION_LENGTH_MS,
        ).toISOString(),
        account_disabled: false,
      },
      login_two_factor_challenge_id: null,
      requires_email_verification: false,
    });
  }

  if (reads && path === "/api/authentication/setup/status") {
    return json({
      setup_required: false,
      setup_completed: true,
      configured_oauth_providers: [],
      access_policy: "open",
      accepting_access_requests: false,
      required_settings: [],
      second_factor_confirmed: true,
      support_email: null,
    });
  }

  if (!reads) return null;

  // Game systems and interface packs: static files, laid out by
  // `scripts/prepare-static.mjs`.
  if (path === "/api/systems") {
    return fetchStatic(`${base}packs/systems.json`);
  }
  if (path === "/api/interface-packs") {
    return fetchStatic(`${base}packs/interface-packs.json`);
  }
  const manifest =
    /^\/api\/(systems|interface-packs)\/([a-z0-9][a-z0-9_-]*)\/manifest\.json$/.exec(
      path,
    );
  if (manifest) {
    const folder = manifest[1] === "systems" ? "systems" : "interface";
    return fetchStatic(`${base}packs/${folder}/${manifest[2]}/manifest.json`);
  }

  // A scene's background: one of the maps shipped beside the page, or a
  // picture the visitor gave it, kept in this browser.
  const asset = /^\/api\/canvas-assets\/([0-9a-f-]{36})\.webp(\.meta)?$/.exec(
    path,
  );
  if (asset) {
    const held = demoState().assets[asset[1]];
    if (!held || asset[2]) return json({ error: "asset not found" }, 404);
    if (held.file) return fetchStatic(`${base}maps/${held.file}`, { method });
    return bytesOf(asset[1]).then((bytes) =>
      bytes
        ? new Response(method === "HEAD" ? null : bytes, {
            headers: {
              "content-type": "image/webp",
              "content-length": String(bytes.size),
            },
          })
        : json({ error: "asset not found" }, 404),
    );
  }

  // An actor's picture, drawn from its spec. The engine reads a token's
  // `photoUrl` through this `fetch` (Bevy's wasm asset reader calls
  // `window.fetch`), and asks for `.meta` first; there is none.
  const art = readArtPath(path);
  if (art) {
    const held = demoState().art[art.assetId];
    if (!held || art.meta) return json({ error: "asset not found" }, 404);
    return drawPng(drawArt(held)).then(
      (png) =>
        new Response(method === "HEAD" ? null : png, {
          headers: { "content-type": "image/png" },
        }),
    );
  }

  return null;
}
