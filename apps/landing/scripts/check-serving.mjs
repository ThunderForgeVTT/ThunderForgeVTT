// Spec 090 FR-025: stock nginx, with the landing's template, sends the brotli
// twin to a browser that takes br, and the plain file to one that does not.
//
// Starts `nginx:stable-alpine` in Docker with nginx.conf.template, the
// landing's dist/ and the demo's data/demo/ mounted read-only, on a free
// port. Asks for the engine and a landing script in each encoding, checks
// headers and lengths against engine.json, and removes the container.
//
// Needs both builds and compress.mjs to have run (the slice command does).
import { execFileSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { request } from "node:http";
import { createServer } from "node:net";
import { join, resolve } from "node:path";

const HERE = new URL(".", import.meta.url).pathname;
const ROOT = resolve(HERE, "../../..");
const DIST = resolve(HERE, "../dist");
const DEMO = join(ROOT, "data/demo");
const TEMPLATE = resolve(HERE, "../nginx.conf.template");
const GITHUB_AUTH = resolve(HERE, "../github-auth.sh");
const IMAGE = "nginx:stable-alpine";

for (const need of [join(DIST, "index.html"), join(DEMO, "engine.json")]) {
  if (!existsSync(need)) throw new Error(`check-serving: ${need} is missing; build first`);
}
const manifest = JSON.parse(readFileSync(join(DEMO, "engine.json"), "utf8"));
if (!manifest.bytes.br) throw new Error("check-serving: engine.json has no bytes.br; run compress.mjs");
const landingJs = readdirSync(join(DIST, "assets")).find((f) => /^index-.*\.js$/.test(f));

const port = await new Promise((done) => {
  const s = createServer().listen(0, "127.0.0.1", () => {
    const { port } = s.address();
    s.close(() => done(port));
  });
});
const name = `tf-check-serving-${process.pid}`;
const docker = (...args) => execFileSync("docker", args, { encoding: "utf8" }).trim();

/** One raw GET: no decoding, so lengths are what went over the wire. */
const get = (path, encoding) =>
  new Promise((done, fail) => {
    const req = request(
      {
        host: "127.0.0.1",
        port,
        path,
        headers: encoding ? { "Accept-Encoding": encoding } : {},
      },
      (res) => {
        let bytes = 0;
        res.on("data", (c) => (bytes += c.length));
        res.on("end", () => done({ status: res.statusCode, headers: res.headers, bytes }));
      },
    );
    req.on("error", fail);
    req.end();
  });

const problems = [];
const expect = (what, actual, wanted) => {
  const ok = wanted instanceof RegExp ? wanted.test(String(actual)) : actual === wanted;
  console.log(`${ok ? "ok  " : "FAIL"} ${what}: ${actual}${ok ? "" : ` (wanted ${wanted})`}`);
  if (!ok) problems.push(what);
};

docker(
  "run", "-d", "--rm", "--name", name,
  "-p", `127.0.0.1:${port}:8080`,
  "-e", "GITHUB_TOKEN=",
  "-e", "TELEMETRY=false",
  "-e", "THUNDERFORGE_BROWSER_TELEMETRY_ENDPOINT=https://telemetry.thunderforge.dev",
  "-e", "THUNDERFORGE_BROWSER_TELEMETRY_SAMPLE_RATE=1.0",
  "-v", `${TEMPLATE}:/etc/nginx/templates/default.conf.template:ro`,
  "-v", `${GITHUB_AUTH}:/docker-entrypoint.d/40-github-auth.sh:ro`,
  // A bind cannot be made inside a read-only bind, so both trees go under
  // /srv and the web root links to them, as the image's two COPYs lay out.
  "-v", `${DIST}:/srv/landing:ro`,
  "-v", `${DEMO}:/srv/demo:ro`,
  "--entrypoint", "sh",
  IMAGE,
  "-c",
  'rm -rf /usr/share/nginx/html/* && ln -s /srv/landing/* /usr/share/nginx/html/ ' +
    '&& ln -s /srv/demo /usr/share/nginx/html/demo ' +
    '&& exec /docker-entrypoint.sh nginx -g "daemon off;"',
);
try {
  let up = false;
  for (let i = 0; i < 100 && !up; i++) {
    up = await get("/healthz").then((r) => r.status === 200, () => false);
    if (!up) await new Promise((r) => setTimeout(r, 100));
  }
  if (!up) throw new Error(`check-serving: nginx did not come up\n${docker("logs", name)}`);

  // With no App and no token, github-auth.sh still writes the include, empty.
  expect("github auth: no App, no token", docker("exec", name, "cat", "/etc/nginx/github-auth.conf").trim(),
    'map "" $github_auth {\n  default "";\n}');

  const wasm = `/demo/${manifest.wasm}`;
  const br = await get(wasm, "gzip, deflate, br, zstd");
  expect("engine wasm, br: status", br.status, 200);
  expect("engine wasm, br: Content-Encoding", br.headers["content-encoding"], "br");
  expect("engine wasm, br: Content-Type", br.headers["content-type"], "application/wasm");
  expect("engine wasm, br: bytes", br.bytes, manifest.bytes.br);
  expect("engine wasm, br: Vary", br.headers.vary, /Accept-Encoding/i);
  expect("engine wasm, br: Cache-Control", br.headers["cache-control"], /immutable/);
  expect("engine wasm, br: nosniff", br.headers["x-content-type-options"], "nosniff");

  const gz = await get(wasm, "gzip");
  expect("engine wasm, gzip: Content-Encoding", gz.headers["content-encoding"], "gzip");
  expect("engine wasm, gzip: Content-Type", gz.headers["content-type"], "application/wasm");
  expect("engine wasm, gzip: Vary", gz.headers.vary, /Accept-Encoding/i);

  const plain = await get(wasm);
  expect("engine wasm, plain: Content-Encoding", plain.headers["content-encoding"], undefined);
  expect("engine wasm, plain: bytes", plain.bytes, manifest.bytes.raw);

  const js = await get(`/demo/${manifest.js}`, "br");
  expect("engine js, br: Content-Encoding", js.headers["content-encoding"], "br");
  expect("engine js, br: Content-Type", js.headers["content-type"], /^text\/javascript|^application\/javascript/);
  expect("engine js, br: bytes", js.bytes, statSync(join(DEMO, `${manifest.js}.br`)).size);

  const page = await get(`/assets/${landingJs}`, "br");
  expect("landing js, br: Content-Encoding", page.headers["content-encoding"], "br");
  expect("landing js, br: Content-Type", page.headers["content-type"], /javascript/);

  const missing = await get("/assets/nothing-here.js", "br");
  expect("missing asset: status", missing.status, 404);

  const json = await get("/demo/engine.json", "br");
  expect("engine.json: status", json.status, 200);
  expect("engine.json: Cache-Control", json.headers["cache-control"], "no-cache");

  expect("the page: status", (await get("/")).status, 200);
} finally {
  try {
    docker("rm", "-f", name);
  } catch {
    // Already gone.
  }
}

if (problems.length) {
  console.error(`check-serving: ${problems.length} failed`);
  process.exit(1);
}
console.log("check-serving: nginx serves the brotli twins");
