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
  { cwd = ROOT_DIR, prefix, detached = true },
) {
  const child = spawn(command, {
    cwd,
    shell: true,
    detached,
    env: {
      ...process.env,
      FORCE_COLOR: process.env.FORCE_COLOR ?? "1",
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

function getEngineInputsHash() {
  const hash = createHash("sha256");

  if (existsSync(WORKSPACE_CARGO_TOML)) {
    hashFile(hash, WORKSPACE_CARGO_TOML);
  }

  if (existsSync(WORKSPACE_CARGO_LOCK)) {
    hashFile(hash, WORKSPACE_CARGO_LOCK);
  }

  hashFile(hash, ENGINE_CARGO_TOML);
  hashDirectoryRecursive(hash, ENGINE_SRC_DIR);
  return hash.digest("hex");
}

export async function buildEngine() {
  log("engine", "Building WebAssembly engine...");
  const child = spawnManaged(
    "wasm-pack build ./ --dev --target web --out-dir ../../dist/engine --scope thunderforge --out-name engine",
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

  const currentInputsHash = getEngineInputsHash();
  writeFileSync(ENGINE_PKG_SUM, currentInputsHash, "utf-8");
  log("engine", "Build complete and pkg.sum updated.");
}

export async function ensureEngineBuild({ force = false } = {}) {
  if (force) {
    log("engine", "Forcing build...", process.stderr);
    await buildEngine();
    return;
  }

  if (!existsSync(ENGINE_PKG_DIR)) {
    log("engine", "No pkg directory found, building engine...");
    await buildEngine();
    return;
  }

  if (!existsSync(ENGINE_PKG_SUM)) {
    log("engine", "No pkg.sum file found, building engine...");
    await buildEngine();
    return;
  }

  const pkgSum = readFileSync(ENGINE_PKG_SUM, "utf-8").trim();
  const currentInputsHash = getEngineInputsHash();
  if (pkgSum === currentInputsHash) {
    log("engine", "Engine is up to date, skipping build...");
    return;
  }

  log("engine", "Engine is out of date, building...");
  await buildEngine();
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

export async function runCommand(command, { name, cwd = ROOT_DIR, prefix }) {
  const child = spawnManaged(command, { cwd, prefix });
  const result = await waitForProcess(child, name);
  if (result.code !== 0) {
    throw new Error(`${name} exited with code ${result.code}`);
  }
}

export function parseArgs(argv = process.argv.slice(2), options = {}) {
  const { allowOnlyWasm = false } = options;
  const args = {
    force: false,
    onlyWasm: false,
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
      default:
        throw new Error(`Unknown argument: ${arg}`);
    }
  }

  return args;
}
