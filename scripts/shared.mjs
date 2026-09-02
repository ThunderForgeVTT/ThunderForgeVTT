import { spawn } from "child_process";
import { fileURLToPath } from "url";
import { dirname, join } from "path";
import {
  existsSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "fs";
import { createHash } from "crypto";
import readline from "readline";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

export const ROOT_DIR = join(__dirname, "..");
const ENGINE_DIR = join(ROOT_DIR, "src/engine");
const ENGINE_SRC_DIR = join(ENGINE_DIR, "src");
const ENGINE_CARGO_TOML = join(ENGINE_DIR, "Cargo.toml");
const ENGINE_PKG_DIR = join(ROOT_DIR, "dist/engine");
const ENGINE_PKG_SUM = join(ENGINE_PKG_DIR, "pkg.sum");
const ENGINE_PKG_PACKAGE_JSON = join(ENGINE_PKG_DIR, "package.json");
const WORKSPACE_CARGO_TOML = join(ROOT_DIR, "Cargo.toml");
const WORKSPACE_CARGO_LOCK = join(ROOT_DIR, "Cargo.lock");

const ANSI_RESET = "\x1b[0m";
const PREFIX_COLORS = {
  dev: "\x1b[36m",
  build: "\x1b[34m",
  engine: "\x1b[35m",
  backend: "\x1b[31m",
  frontend: "\x1b[32m",
  tunnel: "\x1b[33m",
};

const runningChildren = new Set();
let shuttingDown = false;

const SHUTDOWN_GRACE_MS = 2500;
const SHUTDOWN_TERM_MS = 2000;
const SHUTDOWN_KILL_MS = 1000;

export function log(prefix, message, stream = process.stdout) {
  const color = PREFIX_COLORS[prefix] ?? "";
  stream.write(`${color}[${prefix}]${ANSI_RESET} ${message}\n`);
}

function pipeWithPrefix(stream, prefix, target) {
  const rl = readline.createInterface({ input: stream });
  rl.on("line", (line) => {
    if (line.length === 0) {
      target.write("\n");
      return;
    }
    const color = PREFIX_COLORS[prefix] ?? "";
    target.write(`${color}[${prefix}]${ANSI_RESET} ${line}\n`);
  });
  return rl;
}

export function spawnManaged(
  command,
  { cwd = ROOT_DIR, prefix, detached = true, env = {} },
) {
  const child = spawn(command, {
    cwd,
    shell: true,
    detached,
    env: {
      ...process.env,
      FORCE_COLOR: process.env.FORCE_COLOR ?? "1",
      // Last, so a caller can override anything inherited. `e2e-parallel.mjs`
      // runs several stacks at once and each needs its own database, bucket,
      // ports and data path — values that are otherwise inherited identically
      // from this process and would have every shard writing to one place.
      ...env,
    },
    stdio: ["inherit", "pipe", "pipe"],
  });

  runningChildren.add(child);

  const stdoutRl = pipeWithPrefix(child.stdout, prefix, process.stdout);
  const stderrRl = pipeWithPrefix(child.stderr, prefix, process.stderr);

  child.on("close", () => {
    stdoutRl.close();
    stderrRl.close();
    runningChildren.delete(child);
  });

  return child;
}

export function waitForProcess(child, name) {
  return new Promise((resolve, reject) => {
    child.once("error", (err) => {
      reject(new Error(`${name} failed to start: ${err.message}`));
    });

    child.once("exit", (code, signal) => {
      if (signal) {
        resolve({ code: 0, signal });
        return;
      }
      resolve({ code: code ?? 0, signal: null });
    });
  });
}

function hashFile(hash, filePath) {
  hash.update(filePath);
  hash.update(readFileSync(filePath));
}

function hashDirectoryRecursive(hash, dirPath) {
  const entries = readdirSync(dirPath).sort();
  for (const entry of entries) {
    const fullPath = join(dirPath, entry);
    const stat = statSync(fullPath);
    if (stat.isDirectory()) {
      hashDirectoryRecursive(hash, fullPath);
      continue;
    }
    hashFile(hash, fullPath);
  }
}

/**
 * The workspace crates the engine is built from, read out of its own
 * `Cargo.toml` rather than listed here.
 *
 * Derived on purpose: a hardcoded list is a list that goes stale the day
 * someone adds a `path` dependency, and the symptom of it being stale is a
 * silently skipped rebuild — the worst failure available here, because it
 * looks like it worked. Whatever the engine says it depends on is what gets
 * hashed.
 */
function engineLocalCrateDirs() {
  if (!existsSync(ENGINE_CARGO_TOML)) {
    return [];
  }
  const manifest = readFileSync(ENGINE_CARGO_TOML, "utf-8");
  const dirs = new Set();
  for (const match of manifest.matchAll(/path\s*=\s*"([^"]+)"/g)) {
    dirs.add(join(ENGINE_DIR, match[1]));
  }
  return [...dirs].sort();
}

function getEngineInputsHash(profile = engineProfile()) {
  const hash = createHash("sha256");

  if (existsSync(WORKSPACE_CARGO_TOML)) {
    hashFile(hash, WORKSPACE_CARGO_TOML);
  }

  if (existsSync(WORKSPACE_CARGO_LOCK)) {
    hashFile(hash, WORKSPACE_CARGO_LOCK);
  }

  hashFile(hash, ENGINE_CARGO_TOML);
  hashDirectoryRecursive(hash, ENGINE_SRC_DIR);

  // The engine is not only `src/engine`. It compiles `crates/thunderforge-*`
  // in, and editing one of those used to leave this hash unchanged: the dev
  // loop would report "Engine is up to date, skipping build" and serve the
  // previous wasm. Every cache change lives in those crates, so the whole of
  // spec 028 was invisible to the dev loop — and a test run against a stale
  // bundle does not fail, it passes for the wrong reason.
  //
  // `Cargo.lock` does not cover this either: a path dependency's *contents*
  // are not in it, only the fact of it.
  for (const crateDir of engineLocalCrateDirs()) {
    const crateManifest = join(crateDir, "Cargo.toml");
    if (existsSync(crateManifest)) {
      hashFile(hash, crateManifest);
    }
    const crateSrc = join(crateDir, "src");
    if (existsSync(crateSrc)) {
      hashDirectoryRecursive(hash, crateSrc);
    }
  }
  // The profile is part of the cache key. Without it, switching between dev
  // and release leaves pkg.sum matching and the rebuild is silently skipped —
  // you would get whichever bundle happened to be on disk, which is the worst
  // failure available here because it looks like it worked.
  hash.update(profile);
  return hash.digest("hex");
}

/**
 * Which wasm-pack profile to build the engine with.
 *
 * `--dev` is seconds to build and 220MB to ship; `--release` is ~7 minutes to
 * build and 29.8MB, of which 4.92MB after brotli (2026-09-01, after spec 031;
 * it was 24.7MB and 4.15MB before). The gap is not mostly code:
 * 71% of the dev bundle is the wasm `name` section — unmangled Rust and Bevy
 * symbols — which is why it gzips 10:1 and why stripping it is worth so much.
 *
 * So the default follows the caller rather than being one global choice. The
 * dev loop keeps `--dev`, because a seven-minute wait after every engine edit
 * would be intolerable and nobody is measuring load time there. Everything
 * else — production builds, and e2e runs where a 57MB unoptimized code
 * section is compiled by the browser on every page load — gets `--release`.
 *
 * Override with ENGINE_PROFILE=dev|release when you want the other one.
 */
export function engineProfile(defaultProfile = "release") {
  const requested = process.env.ENGINE_PROFILE;
  if (requested === "dev" || requested === "release") {
    return requested;
  }
  return defaultProfile;
}

/**
 * Whether to skip `wasm-opt` on a release build.
 *
 * Measured on this repo: `cargo build --release` takes about 35 seconds and
 * `wasm-opt` takes the rest of a multi-minute build. Neither scales with
 * cores — `[profile.release]` sets `codegen-units = 1` on purpose, and
 * `wasm-opt` is single-threaded over a ~25MB module — so a bigger machine does
 * not help. Skipping the optimiser is the only lever that moves the number.
 *
 * The cost is real: an unoptimised module is larger for the browser to fetch
 * and compile, and slower to run. That is why this is opt-in per run rather
 * than a default, and why `scripts/e2e-parallel.mjs` refuses it for the lane
 * that measures frame rates — `engine-limits` gates `fps > 20`, and a
 * deliberately slower build would turn that assertion into a lie in either
 * direction.
 *
 * Use it for targeted verification, where the question is "does this behave
 * correctly" and never "how fast is it".
 */
export function skipWasmOpt() {
  return process.env.ENGINE_SKIP_WASM_OPT === "1";
}

export async function buildEngine({
  profile = engineProfile(),
  noOpt = skipWasmOpt(),
} = {}) {
  const optNote = noOpt && profile === "release" ? ", wasm-opt skipped" : "";
  log("engine", `Building WebAssembly engine (${profile}${optNote})...`);
  // Bevy prints "<Enable the debug feature to see the name>" in place of every
  // system, component and resource name unless `bevy/debug` is on — see the
  // `debug-names` feature in `src/engine/Cargo.toml`. It rides the dev profile
  // and only the dev profile: the names are string data that survives the
  // symbol stripping the release build depends on, and the dev bundle is 71%
  // unmangled symbols already, so this is free exactly where it is useful.
  // After `--`: wasm-pack has no `--features` of its own, and takes trailing
  // positional EXTRA_OPTIONS to hand to `cargo build`.
  const features = profile === "dev" ? " -- --features debug-names" : "";
  // Only meaningful on release: the dev profile does not run `wasm-opt` in the
  // first place, so passing it there would advertise a saving that is not
  // there.
  const optFlag = noOpt && profile === "release" ? " --no-opt" : "";
  const child = spawnManaged(
    `wasm-pack build ./ --${profile}${optFlag} --target web --out-dir ../../dist/engine --scope thunderforge --out-name engine${features}`,
    {
      cwd: ENGINE_DIR,
      prefix: "engine",
    },
  );
  const result = await waitForProcess(child, "engine build");

  if (result.code !== 0) {
    throw new Error(`Engine build failed with exit code ${result.code}`);
  }

  // Set package name to the scoped package expected by the web app imports.
  const pkg = JSON.parse(readFileSync(ENGINE_PKG_PACKAGE_JSON, "utf-8"));
  pkg.name = "@thunderforge/engine";
  writeFileSync(ENGINE_PKG_PACKAGE_JSON, JSON.stringify(pkg, null, 2), "utf-8");

  const currentInputsHash = getEngineInputsHash(profile);
  writeFileSync(ENGINE_PKG_SUM, currentInputsHash, "utf-8");
  log("engine", "Build complete and pkg.sum updated.");
}

export async function ensureEngineBuild({
  force = false,
  profile = engineProfile(),
} = {}) {
  if (force) {
    log("engine", "Forcing build...", process.stderr);
    await buildEngine({ profile });
    return;
  }

  if (!existsSync(ENGINE_PKG_DIR)) {
    log("engine", "No pkg directory found, building engine...");
    await buildEngine({ profile });
    return;
  }

  if (!existsSync(ENGINE_PKG_SUM)) {
    log("engine", "No pkg.sum file found, building engine...");
    await buildEngine({ profile });
    return;
  }

  const pkgSum = readFileSync(ENGINE_PKG_SUM, "utf-8").trim();
  const currentInputsHash = getEngineInputsHash(profile);
  if (pkgSum === currentInputsHash) {
    log("engine", "Engine is up to date, skipping build...");
    return;
  }

  log("engine", "Engine is out of date, building...");
  await buildEngine({ profile });
}

export async function terminateChildren(signal = "SIGINT") {
  if (shuttingDown) {
    return;
  }
  shuttingDown = true;

  const children = [...runningChildren];
  if (children.length === 0) {
    return;
  }

  const sendSignal = (targetSignal) => {
    for (const child of children) {
      if (!runningChildren.has(child)) {
        continue;
      }

      if (typeof child.pid === "number") {
        try {
          process.kill(-child.pid, targetSignal);
          continue;
        } catch {
          // Fall through to direct child kill below.
        }
      }

      try {
        child.kill(targetSignal);
      } catch {
        // Ignore if process already exited between checks.
      }
    }
  };

  const waitForClose = async (timeoutMs) => {
    const pending = children.filter((child) => runningChildren.has(child));
    if (pending.length === 0) {
      return true;
    }

    const allClosedPromise = Promise.all(
      pending.map(
        (child) =>
          new Promise((resolve) => {
            child.once("close", resolve);
          }),
      ),
    ).then(() => true);

    const timedOut = await Promise.race([
      allClosedPromise,
      new Promise((resolve) => {
        setTimeout(() => resolve(false), timeoutMs);
      }),
    ]);

    return Boolean(timedOut);
  };

  log("dev", `Stopping ${children.length} process(es) with ${signal}...`);
  sendSignal(signal);
  if (await waitForClose(SHUTDOWN_GRACE_MS)) {
    return;
  }

  if (signal !== "SIGTERM") {
    log("dev", "Processes still running, escalating to SIGTERM...");
    sendSignal("SIGTERM");
    if (await waitForClose(SHUTDOWN_TERM_MS)) {
      return;
    }
  }

  log(
    "dev",
    "Processes still running, escalating to SIGKILL...",
    process.stderr,
  );
  sendSignal("SIGKILL");
  await waitForClose(SHUTDOWN_KILL_MS);
}

export async function runCommand(command, { name, cwd = ROOT_DIR, prefix, env = {} }) {
  const child = spawnManaged(command, { cwd, prefix, env });
  const result = await waitForProcess(child, name);
  if (result.code !== 0) {
    throw new Error(`${name} exited with code ${result.code}`);
  }
}

export function parseArgs(argv = process.argv.slice(2), options = {}) {
  const { allowOnlyWasm = false, allowTunnel = false } = options;
  const args = {
    force: false,
    onlyWasm: false,
    tunnel: false,
  };

  for (const arg of argv) {
    switch (arg) {
      case "--force":
        args.force = true;
        break;
      case "--only-wasm":
        if (!allowOnlyWasm) {
          throw new Error("--only-wasm is only supported by scripts/build.mjs");
        }
        args.onlyWasm = true;
        break;
      case "--tunnel":
        if (!allowTunnel) {
          throw new Error("--tunnel is only supported by scripts/dev.mjs");
        }
        args.tunnel = true;
        break;
      default:
        throw new Error(`Unknown argument: ${arg}`);
    }
  }

  return args;
}
