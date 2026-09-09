/**
 * A destination that is not GitHub.
 *
 * Spec 037, `contracts/e2e-harness.md` § 2. Feedback delivery had never run
 * against anything: the outbox, the backoff curve, the adoption path and the
 * "exactly once after an ambiguous failure" rule were all proven against
 * in-process fakes, and nothing had ever put an HTTP request on a wire.
 *
 * # Why this needs no product code
 *
 * `GitHubApp::with_bases` already existed for GitHub Enterprise and was wired
 * to `GITHUB_API_BASE` / `GITHUB_WEB_BASE` — a configuration value an operator
 * running their own GitHub sets too. So the server talks to this stub through
 * exactly the code path it uses in production. There is no `#[cfg(test)]`
 * branch, no flag and no fake transport, because a test that exercises a
 * test-only branch proves the branch.
 *
 * # What it does not do
 *
 * It does not verify the JWT or the installation token. Authentication is
 * GitHub's to enforce and asserting it here would be asserting this file's own
 * behaviour. What it *does* record is what arrived, so a test can say what the
 * server sent rather than that it sent something.
 */

import { createServer } from "node:http";

export function startGithubStub(port) {
  const issues = [];
  const files = [];
  const calls = [];
  // Branch state has to outlive one request: `ensure_branch` creates the
  // attachment branch on the first upload and expects to find it on the next.
  const branches = new Set(["main"]);
  let failNext = null;
  let nextNumber = 1;

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
      const body = raw ? JSON.parse(raw) : {};
      const send = (status, payload) => {
        const text = JSON.stringify(payload);
        res.writeHead(status, {
          "Content-Type": "application/json",
          "Content-Length": Buffer.byteLength(text),
        });
        res.end(text);
      };

      // ---- controls -----------------------------------------------------
      if (path === "/_control/fail-next" && method === "POST") {
        failNext = body ?? null;
        return send(200, { ok: true });
      }
      if (path.startsWith("/_control/close/") && method === "POST") {
        const number = Number(path.split("/").pop());
        const issue = issues.find((i) => i.number === number);
        if (issue) issue.state = "closed";
        return send(200, { ok: true });
      }
      if (path === "/_control/issues") {
        return send(200, { issues, files, calls });
      }
      if (path === "/_control/reset" && method === "POST") {
        issues.length = 0;
        files.length = 0;
        calls.length = 0;
        failNext = null;
        branches.clear();
        branches.add("main");
        return send(200, { ok: true });
      }

      // ---- the failure that matters -------------------------------------
      //
      // Applied *after* the request has been recorded, never before. That is
      // the whole point of US6: the one failure worth testing is the request
      // that succeeded at the host and whose answer never arrived, and a stub
      // that failed before recording would be simulating a different, easier
      // bug.
      const recordThenMaybeFail = (status, payload) => {
        if (failNext && "transport" in failNext) {
          failNext = null;
          req.socket.destroy();
          return;
        }
        if (failNext && "status" in failNext) {
          const code = failNext.status;
          failNext = null;
          return send(code, { message: "stub was told to fail" });
        }
        return send(status, payload);
      };

      // ---- the API ------------------------------------------------------
      const tokenMatch = /^\/app\/installations\/([^/]+)\/access_tokens$/.exec(path);
      if (tokenMatch && method === "POST") {
        return send(201, {
          token: "ghs_stubinstallationtoken",
          expires_at: new Date(Date.now() + 3_600_000).toISOString(),
        });
      }

      const issuesMatch = /^\/repos\/([^/]+)\/([^/]+)\/issues$/.exec(path);
      if (issuesMatch && method === "POST") {
        const number = nextNumber++;
        const issue = {
          number,
          title: String(body.title ?? ""),
          body: String(body.body ?? ""),
          labels: Array.isArray(body.labels) ? body.labels : [],
          state: "open",
        };
        issues.push(issue);
        return recordThenMaybeFail(201, {
          number,
          html_url: `http://127.0.0.1:${port}/${issuesMatch[1]}/${issuesMatch[2]}/issues/${number}`,
          state: "open",
        });
      }

      const issueMatch = /^\/repos\/([^/]+)\/([^/]+)\/issues\/(\d+)$/.exec(path);
      if (issueMatch && method === "GET") {
        const issue = issues.find((i) => i.number === Number(issueMatch[3]));
        if (!issue) return send(404, { message: "Not Found" });
        return send(200, {
          number: issue.number,
          state: issue.state,
          title: issue.title,
          html_url: `http://127.0.0.1:${port}/${issueMatch[1]}/${issueMatch[2]}/issues/${issue.number}`,
        });
      }

      // `put_file` calls `ensure_branch` first, which reads the branch ref,
      // falls back to the default branch's ref, and creates the branch from
      // it. Three endpoints, and omitting them made an attachment upload fail
      // as a 404 — which the delivery path classifies as
      // `destination_not_found`, so the symptom was "the repository is
      // missing" rather than "the stub has no route for a git ref".
      const refMatch = /^\/repos\/([^/]+)\/([^/]+)\/git\/ref\/heads\/(.+)$/.exec(path);
      if (refMatch && method === "GET") {
        if (!branches.has(decodeURIComponent(refMatch[3]))) {
          return send(404, { message: "No ref found" });
        }
        return send(200, {
          ref: `refs/heads/${refMatch[3]}`,
          object: { sha: "0".repeat(40), type: "commit" },
        });
      }
      const refsMatch = /^\/repos\/([^/]+)\/([^/]+)\/git\/refs$/.exec(path);
      if (refsMatch && method === "POST") {
        const created = String(body.ref ?? "").replace(/^refs\/heads\//, "");
        branches.add(created);
        return send(201, {
          ref: body.ref,
          object: { sha: String(body.sha ?? "0".repeat(40)), type: "commit" },
        });
      }

      const contentsMatch = /^\/repos\/([^/]+)\/([^/]+)\/contents\/(.+)$/.exec(path);
      if (contentsMatch && method === "PUT") {
        files.push({
          path: decodeURIComponent(contentsMatch[3]),
          message: String(body.message ?? ""),
          content: String(body.content ?? ""),
        });
        return recordThenMaybeFail(201, {
          content: { path: decodeURIComponent(contentsMatch[3]) },
        });
      }

      if (path === "/search/issues" && method === "GET") {
        // The delivery key is what makes adoption single-flight: the server
        // searches for the key it stamped into the body before creating a
        // second issue.
        const q = url.searchParams.get("q") ?? "";
        const key = /([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})/i.exec(q)?.[1];
        const found = key
          ? issues.filter((i) => i.body.toLowerCase().includes(key.toLowerCase()))
          : [];
        return send(200, {
          total_count: found.length,
          items: found.map((i) => ({
            number: i.number,
            title: i.title,
            state: i.state,
            html_url: `http://127.0.0.1:${port}/stub/issues/${i.number}`,
          })),
        });
      }

      const repoMatch = /^\/repos\/([^/]+)\/([^/]+)$/.exec(path);
      if (repoMatch && method === "GET") {
        return send(200, {
          full_name: `${repoMatch[1]}/${repoMatch[2]}`,
          private: false,
          default_branch: "main",
        });
      }

      if (path === "/app/installations" && method === "GET") {
        return send(200, [{ id: 1, account: { login: "stub" } }]);
      }
      if (path === "/installation/repositories" && method === "GET") {
        return send(200, { total_count: 1, repositories: [] });
      }

      send(404, { message: `stub has no route for ${method} ${path}` });
    });
  });

  return new Promise((resolve) => {
    server.listen(port, "127.0.0.1", () => {
      resolve({
        port,
        url: `http://127.0.0.1:${port}`,
        issues: () => issues,
        files: () => files,
        calls: () => calls,
        failNext: (mode) => {
          failNext = mode;
        },
        close: (number) => {
          const issue = issues.find((i) => i.number === number);
          if (issue) issue.state = "closed";
        },
        reset: () => {
          issues.length = 0;
          files.length = 0;
          calls.length = 0;
          failNext = null;
        },
        stop: () =>
          new Promise((done) => {
            server.close(() => done());
          }),
      });
    });
  });
}

// Run directly: `node scripts/github-stub.mjs 31500`.
const port = Number(process.argv[2]);
if (!Number.isInteger(port) || port <= 0) {
  process.stderr.write("usage: node scripts/github-stub.mjs <port>\n");
  process.exit(1);
}
startGithubStub(port).then((stub) => {
  process.stdout.write(`github stub listening on ${stub.url}\n`);
});
