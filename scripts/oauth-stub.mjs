/**
 * A provider that is not GitHub, Google, Discord or anybody else.
 *
 * Spec 036 US6, `contracts/e2e-fixtures.md` § "The OAuth provider stub".
 *
 * # The gap this closes
 *
 * External sign-in had never been driven end to end. The ~40 lines between
 * "the provider redirects back" and "an account exists" — the state check, the
 * code exchange, the userinfo read, the provisioning decision, the admission
 * policy — were covered by unit tests on either side of them and by nothing
 * that put an HTTP request on a wire. Spec 035's T056 and spec 032's deferred
 * manual pass both carry the same note for the same reason: without a
 * provider, the flow was reachable only by hand.
 *
 * # Why this needs no product code
 *
 * `oauth_providers` already carries `authorization_url`, `token_url` and
 * `userinfo_url` **per row**, because an operator wiring up their own provider
 * sets exactly those. So the server talks to this stub through the code path
 * it uses in production. There is no `#[cfg(test)]` branch, no flag, no fake
 * transport — a test that exercises a test-only branch proves the branch
 * (FR-023).
 *
 * # What it does not do
 *
 * It does not verify the client secret, the PKCE verifier or the state token.
 * Those are the *product's* to check on the way back, and the tests that
 * matter assert the product's behaviour, not this file's. What it does do is
 * let a scenario choose what `/userinfo` returns — a verified address, an
 * unverified one, or none at all — because that choice is the input to three
 * of the four scenarios US6 asks for.
 */

import { createServer } from "node:http";

/** What `/userinfo` answers with until a test says otherwise. */
const DEFAULT_IDENTITY = {
  sub: "stub-user-1",
  email: "stub-user-1@example.org",
  email_verified: true,
  name: "Stub User",
};

export function startOAuthStub(port) {
  /** The identity the next `/userinfo` returns. `null` means answer `{}`. */
  let identity = { ...DEFAULT_IDENTITY };
  const calls = [];

  const server = createServer((req, res) => {
    const url = new URL(req.url ?? "/", `http://127.0.0.1:${port}`);
    const path = url.pathname;
    const method = req.method ?? "GET";
    calls.push(`${method} ${path}`);

    let raw = "";
    req.on("data", (chunk) => {
      raw += chunk;
    });
    req.on("end", () => {
      const send = (status, payload) => {
        const text = JSON.stringify(payload);
        res.writeHead(status, {
          "Content-Type": "application/json",
          "Content-Length": Buffer.byteLength(text),
        });
        res.end(text);
      };

      // ---- controls -------------------------------------------------------
      //
      // Under `/_control`, which no product code knows about, so a request
      // that reached here by accident could not be mistaken for a real one.
      if (path === "/_control/identity" && method === "POST") {
        const body = raw ? JSON.parse(raw) : {};
        // `null` is meaningful and distinct from absent: it is the
        // "the provider told us nothing" case.
        identity =
          body.identity === undefined ? { ...DEFAULT_IDENTITY } : body.identity;
        return send(200, { ok: true });
      }
      if (path === "/_control/calls") {
        return send(200, { calls });
      }
      if (path === "/_control/reset" && method === "POST") {
        identity = { ...DEFAULT_IDENTITY };
        calls.length = 0;
        return send(200, { ok: true });
      }

      // ---- the provider ---------------------------------------------------

      /**
       * Straight back to whatever `redirect_uri` was asked for, carrying a
       * fixed code and **the caller's own `state` verbatim**.
       *
       * Echoing the state rather than inventing one is the whole point: the
       * product generated it, stored it against the authorization session, and
       * checks it on the way back. A stub that made one up would turn every
       * scenario into an assertion that the state check fails.
       */
      if (path === "/authorize") {
        const redirect = url.searchParams.get("redirect_uri");
        const state = url.searchParams.get("state") ?? "";
        if (!redirect) {
          return send(400, {
            error: "invalid_request",
            detail: "no redirect_uri",
          });
        }
        // A relative `redirect_uri` is the caller's mistake, not this
        // stub's crash: a real provider requires an absolute one, and
        // `new URL` on a path throws a bare `TypeError: Invalid URL` that
        // says nothing about which request caused it.
        let back;
        try {
          back = new URL(redirect);
        } catch {
          return send(400, {
            error: "invalid_request",
            detail: `redirect_uri must be absolute, got ${redirect}`,
          });
        }
        back.searchParams.set("code", "stub-authorization-code");
        back.searchParams.set("state", state);
        res.writeHead(302, { Location: back.toString() });
        return res.end();
      }

      if (path === "/token" && method === "POST") {
        return send(200, {
          access_token: "stub-access-token",
          token_type: "Bearer",
          expires_in: 3600,
          refresh_token: "stub-refresh-token",
          scope: "openid email profile",
        });
      }

      if (path === "/userinfo") {
        // An identity of `null` is a provider that returned a body with no
        // identity in it — which is how "no verified email" is expressed
        // without the stub having an opinion about what the product does
        // with it.
        return send(200, identity ?? {});
      }

      return send(404, { error: "not_found", path });
    });
  });

  return new Promise((resolve) => {
    server.listen(port, "127.0.0.1", () => {
      resolve({
        url: `http://127.0.0.1:${port}`,
        stop: () =>
          new Promise((done) => {
            server.close(() => done());
          }),
      });
    });
  });
}

// Run directly: `node scripts/oauth-stub.mjs 31600`.
const port = Number(process.argv[2]);
if (!Number.isInteger(port) || port <= 0) {
  process.stderr.write("usage: node scripts/oauth-stub.mjs <port>\n");
  process.exit(1);
}
startOAuthStub(port).then((stub) => {
  process.stdout.write(`oauth stub listening on ${stub.url}\n`);
});
