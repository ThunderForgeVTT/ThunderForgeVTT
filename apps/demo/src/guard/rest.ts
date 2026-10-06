/**
 * The handful of plain HTTP addresses the client reads outside GraphQL.
 *
 * Each is answered here from the page or from one of the demo's own static
 * files. An address that is not in this table is not part of the demo.
 */
import { demoState } from "../backend/state";
import { viewerUser } from "../backend/actors";

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

const SESSION_LENGTH_MS = 24 * 60 * 60 * 1000;

/** `null` when the demo has no answer for this address. */
export function answerRest(
  method: string,
  path: string,
  base: string,
  fetchStatic: typeof fetch,
): Promise<Response> | Response | null {
  const reads = method === "GET" || method === "HEAD";

  if (reads && path === "/api/authentication/session") {
    const state = demoState();
    const at = state.world.createdAt as string;
    const user = viewerUser(state);
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

  // A scene's background: one of the maps shipped beside the page.
  const asset = /^\/api\/canvas-assets\/([0-9a-f-]{36})\.webp(\.meta)?$/.exec(
    path,
  );
  if (asset) {
    const held = demoState().assets[asset[1]];
    if (!held || asset[2]) return json({ error: "asset not found" }, 404);
    return fetchStatic(`${base}maps/${held.file}`, { method });
  }

  return null;
}
