#!/usr/bin/env node
/**
 * Spec 068, FR-009: what a page costs, written as a check anyone can run.
 *
 * `React.lazy` splits a route off; one static import puts a library back.
 * Nothing about the app looks different when that happens — a scene's detail
 * page carried 178 kB of editor for months because one of four editors was
 * imported the plain way. So this reads a production build and answers the
 * only question that matters: when somebody opens this, what arrives without
 * their asking?
 *
 * # What it measures
 *
 * A chunk's *static closure*: itself and everything reachable by `import …
 * from`, never by `import()`. For the entry that is what every visitor
 * downloads. For a route it is counted beyond the entry's, since the entry
 * is already there.
 *
 * Budgets are brotli bytes, because that is what a deployment serves, and
 * live in `bundle-budget.json` beside this file. They are set from what a
 * build measures, with a little room; a split lowers the number in the same
 * commit.
 *
 * # What it forbids
 *
 * A budget catches growth; it does not say *what* grew. So a library that
 * must stay behind a dynamic import is named, with the chunks allowed to
 * reach it statically. Any other chunk that does is a failure that names the
 * path it took.
 *
 * # Why it reads the output and not the module graph
 *
 * The bundler decides what shares a chunk, and that decision is the thing
 * being checked. Source maps say which modules ended up where; the build
 * already writes them.
 */

import { existsSync, readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { brotliCompressSync } from "node:zlib";

const HERE = path.dirname(fileURLToPath(import.meta.url));

/** `SceneDetailPage-UCFdi3t5.js` is `SceneDetailPage`. */
export function chunkName(file) {
  const match = /^(.*)-[A-Za-z0-9_-]{8}\.js$/.exec(file);
  return match ? match[1] : file.replace(/\.js$/, "");
}

/**
 * The files a built chunk imports statically, by basename.
 *
 * `import("./x.js")` is left out by construction: nothing between `import`
 * and the specifier may be a parenthesis.
 */
export function staticImports(source) {
  const found = new Set();
  const pattern =
    /(?:^|[;}\n])\s*(?:import|export)\s*(?:[^"'()]*?from\s*)?["']([^"']+\.js)["']/g;
  for (const match of source.matchAll(pattern)) {
    found.add(match[1].split("/").pop());
  }
  return [...found];
}

/** Every `.js` file in a build's `assets/entry` and `assets/chunks`. */
export function readBuild(buildDir) {
  const chunks = new Map();
  for (const kind of ["entry", "chunks"]) {
    const dir = path.join(buildDir, "assets", kind);
    if (!existsSync(dir)) continue;
    for (const file of readdirSync(dir)) {
      if (!file.endsWith(".js")) continue;
      const full = path.join(dir, file);
      const bytes = readFileSync(full);
      const mapPath = `${full}.map`;
      const sources = existsSync(mapPath)
        ? (JSON.parse(readFileSync(mapPath, "utf8")).sources ?? [])
        : [];
      chunks.set(file, {
        file,
        name: chunkName(file),
        entry: kind === "entry",
        raw: bytes.length,
        brotli: brotliCompressSync(bytes).length,
        imports: staticImports(bytes.toString("utf8")),
        sources,
      });
    }
  }
  for (const chunk of chunks.values()) {
    chunk.imports = chunk.imports.filter((file) => chunks.has(file));
  }
  return chunks;
}

/** A chunk and everything it reaches statically, as file names. */
export function closure(chunks, start) {
  const seen = new Set();
  const queue = [start];
  while (queue.length > 0) {
    const file = queue.pop();
    if (seen.has(file)) continue;
    seen.add(file);
    queue.push(...(chunks.get(file)?.imports ?? []));
  }
  return seen;
}

function total(chunks, files) {
  let raw = 0;
  let brotli = 0;
  for (const file of files) {
    raw += chunks.get(file).raw;
    brotli += chunks.get(file).brotli;
  }
  return { raw, brotli, files: files.size ?? files.length };
}

/**
 * The entry's closure, and each named route's beyond it.
 *
 * A route is named by its chunk, without the hash. One that the build no
 * longer produces is reported as missing rather than as zero: a renamed page
 * must not pass by vanishing.
 */
export function measure(chunks, routeNames) {
  const entries = [...chunks.values()].filter((chunk) => chunk.entry);
  const entryFiles = new Set();
  for (const entry of entries) {
    for (const file of closure(chunks, entry.file)) entryFiles.add(file);
  }
  const routes = {};
  for (const name of routeNames) {
    const chunk = [...chunks.values()].find(
      (candidate) => !candidate.entry && candidate.name === name,
    );
    if (!chunk) {
      routes[name] = null;
      continue;
    }
    const beyond = new Set(
      [...closure(chunks, chunk.file)].filter((file) => !entryFiles.has(file)),
    );
    routes[name] = total(chunks, beyond);
  }
  return { entry: total(chunks, entryFiles), routes };
}

/**
 * Chunks that reach a forbidden library statically and are not allowed to.
 *
 * A chunk that *holds* the library is the library, not an offender; the
 * offender is whoever imported it the plain way.
 */
export function forbiddenReaches(chunks, forbidden) {
  const problems = [];
  for (const rule of forbidden) {
    const holds = (chunk) =>
      chunk.sources.some((source) => source.includes(rule.source));
    const holders = [...chunks.values()].filter(holds).map((c) => c.file);
    if (holders.length === 0) continue;
    for (const chunk of chunks.values()) {
      if (holds(chunk) || (rule.only ?? []).includes(chunk.name)) continue;
      // Reached only through an allowed chunk is the allowed chunk's doing.
      const direct = chunk.imports.find((file) => holders.includes(file));
      if (!direct) continue;
      problems.push(
        `${chunk.name} imports ${rule.source} statically (${chunk.file} → ${direct}). ${rule.why}`,
      );
    }
  }
  return problems;
}

/** Everything wrong with a build against a budget, as sentences. */
export function budgetProblems(measured, budget) {
  const problems = [];
  const over = (label, got, allowed) => {
    if (got.brotli > allowed) {
      problems.push(
        `${label} is ${got.brotli} bytes brotli across ${got.files} files; its budget is ${allowed}.`,
      );
    }
  };
  over("The entry's static closure", measured.entry, budget.entry.brotli);
  for (const [name, allowed] of Object.entries(budget.routes ?? {})) {
    const got = measured.routes[name];
    if (!got) {
      problems.push(
        `No chunk named ${name} in this build. If the page was renamed, rename its budget.`,
      );
      continue;
    }
    over(`${name} beyond the entry`, got, allowed.brotli);
  }
  return problems;
}

function main() {
  const buildDir = path.resolve(
    process.argv[2] ?? path.join(HERE, "..", "data", "client"),
  );
  const budget = JSON.parse(
    readFileSync(path.join(HERE, "bundle-budget.json"), "utf8"),
  );
  const chunks = readBuild(buildDir);
  if (chunks.size === 0) {
    console.error(
      `No build in ${buildDir}. Run \`pnpm --filter @thunderforge/web build\` first.`,
    );
    process.exit(2);
  }
  const measured = measure(chunks, Object.keys(budget.routes ?? {}));
  const row = (label, got, allowed) =>
    console.log(
      `${label.padEnd(34)} ${String(got?.raw ?? "-").padStart(9)} raw ${String(got?.brotli ?? "-").padStart(8)} br   budget ${allowed}`,
    );
  row("entry", measured.entry, budget.entry.brotli);
  for (const [name, allowed] of Object.entries(budget.routes ?? {})) {
    row(name, measured.routes[name], allowed.brotli);
  }
  const problems = [
    ...budgetProblems(measured, budget),
    ...forbiddenReaches(chunks, budget.forbidden ?? []),
  ];
  if (problems.length > 0) {
    console.error(`\n${problems.map((p) => `✗ ${p}`).join("\n")}`);
    process.exit(1);
  }
  console.log("\nEvery budget holds.");
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  main();
}
