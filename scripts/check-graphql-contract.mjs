#!/usr/bin/env node
/**
 * Does every GraphQL operation the web sends still exist on the server?
 *
 * # Why this exists
 *
 * `apps/web` and the system packs' web code speak to the server in
 * hand-written query strings, and nothing compiled the two against each other.
 * Renaming a field or an argument in Rust compiled, passed every Rust test, and
 * broke a button — found by a person, or by an e2e lane twenty minutes into a
 * run, or not at all when no lane happened to press it.
 *
 * This is the contract check between them, in two halves:
 *
 * - `--schema` prints the merged schema from the app crate (the
 *   `thunderforge-schema` binary: no database, no server) and compares it with
 *   the committed `src/app/schema.graphql`. A schema change therefore shows up
 *   in the review diff, and a stale file fails. With `--fix` it rewrites the
 *   file. It compiles, so its cost is whatever `cargo build` costs today.
 *
 * - `--operations` (the default) finds every operation in the web sources and
 *   runs graphql-js `validate()` on each against the committed SDL. It reads
 *   files and exits, so it costs the same every time.
 *
 * Both halves also warn — without failing — about changes to the committed
 * SDL that `findBreakingChanges` calls breaking, measured against the copy on
 * `origin/main` (or the ref in `GRAPHQL_BASE_REF`). A browser tab opened before a deploy still sends the old
 * operations, so a breaking change is worth a sentence in review even when
 * every call site in the tree has been updated.
 *
 * # How operations are found
 *
 * There is no `gql` tag and no `.graphql` file here: operations are template
 * literals handed to `postGraphQL` or to `graphql-ws`'s `subscribe`, often
 * with a shared selection interpolated (`${TOKEN_FIELDS}`). So this parses
 * each file with the TypeScript compiler, takes every string whose text begins
 * with `query`, `mutation`, `subscription` or `fragment`, and substitutes each
 * `${NAME}` with the constant it names — in the same file or imported from
 * another. An interpolation it cannot resolve is an error, not a skip: a check
 * that quietly stops checking is the failure it exists to prevent.
 *
 * The pattern match alone could miss an operation spelled some other way, so
 * every transport call site is checked as well: the first argument of
 * `postGraphQL`/`postGraphQLMultipart` and the `query` of a `subscribe({...})`
 * must resolve to an operation this script validated.
 *
 * # Scope
 *
 * The shipped web code: `apps/web/src` and `packs/systems/<pack>/web/src`.
 * Tests are left out — some send malformed operations on purpose, to prove the
 * server refuses them.
 *
 * Not affected by `--fix` for operations: an invalid call site is a decision
 * between changing the caller and changing the server, and that needs a
 * person.
 */

import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import path from "node:path";

const started = performance.now();
const repoRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);

// graphql-js and TypeScript are the web app's dependencies; the root has none.
const webRequire = createRequire(path.join(repoRoot, "apps/web/package.json"));
const ts = webRequire("typescript");
const { buildSchema, findBreakingChanges, parse, Source, validate } =
  webRequire("graphql");

const SDL_PATH = "src/app/schema.graphql";
const args = process.argv.slice(2);
const fix = args.includes("--fix");
const schemaMode = args.includes("--schema");

const BASE_REF = process.env.GRAPHQL_BASE_REF || "origin/main";

const relative = (file) => path.relative(repoRoot, file);

function done(failed) {
  const ms = Math.round(performance.now() - started);
  process.stdout.write(`(${ms} ms)\n`);
  process.exit(failed ? 1 : 0);
}

// ─── breaking changes ──────────────────────────────────────────────────────

/**
 * Warns, never fails. The copy on `origin/main` is the one deployed clients
 * were built against; with no remote ref or no committed SDL there, there is
 * nothing to compare and it says so once.
 */
function warnBreakingChanges(currentSdl) {
  const base = spawnSync("git", ["show", `${BASE_REF}:${SDL_PATH}`], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  if (base.status !== 0) {
    process.stdout.write(
      `note: no ${SDL_PATH} on ${BASE_REF} to compare against; breaking-change warnings skipped.\n`,
    );
    return;
  }
  if (base.stdout === currentSdl) return;
  const changes = findBreakingChanges(
    buildSchema(base.stdout),
    buildSchema(currentSdl),
  );
  if (changes.length === 0) return;
  process.stdout.write(
    `\nwarning: ${changes.length} breaking schema change(s) against ${BASE_REF}.\n` +
      "Browser tabs opened before a deploy still send the old operations:\n",
  );
  for (const change of changes) {
    process.stdout.write(`  ${change.type}: ${change.description}\n`);
  }
}

// ─── --schema ──────────────────────────────────────────────────────────────

if (schemaMode) {
  const run = spawnSync(
    "cargo",
    [
      "run",
      "--quiet",
      "-p",
      "thunderforge",
      "--bin",
      "thunderforge-schema",
      "--features",
      "schema-sdl",
    ],
    {
      cwd: repoRoot,
      encoding: "utf8",
      maxBuffer: 64 * 1024 * 1024,
      stdio: ["ignore", "pipe", "inherit"],
    },
  );
  if (run.error !== undefined || run.status !== 0) {
    process.stdout.write(
      `could not print the schema: ${run.error?.message ?? `exit ${run.status}`}\n`,
    );
    done(true);
  }
  const printed = run.stdout;
  const sdlFile = path.join(repoRoot, SDL_PATH);
  const committed = existsSync(sdlFile) ? readFileSync(sdlFile, "utf8") : null;

  if (committed === printed) {
    process.stdout.write(`${SDL_PATH} matches the server.\n`);
    warnBreakingChanges(printed);
    done(false);
  }
  if (fix) {
    writeFileSync(sdlFile, printed);
    process.stdout.write(`rewrote ${SDL_PATH} from the server.\n`);
    warnBreakingChanges(printed);
    done(false);
  }
  // What changed, so the failure says which field without a second command.
  const diff = spawnSync("diff", ["-u", sdlFile, "-"], {
    input: printed,
    encoding: "utf8",
  });
  const lines = (diff.stdout ?? "").split("\n").slice(2);
  process.stdout.write(
    `${lines.slice(0, 40).join("\n")}${lines.length > 40 ? "\n…" : ""}\n`,
  );
  process.stdout.write(
    `${SDL_PATH} is stale: the server's schema has changed since it was written.\n` +
      "Run `pnpm verify:fix` (or this script with --schema --fix) and commit the result.\n",
  );
  done(true);
}

// ─── --operations: find the sources ────────────────────────────────────────

function sourceRoots() {
  const roots = [path.join(repoRoot, "apps/web/src")];
  const packs = path.join(repoRoot, "packs/systems");
  for (const pack of readdirSync(packs, { withFileTypes: true })) {
    const src = path.join(packs, pack.name, "web/src");
    if (pack.isDirectory() && existsSync(src)) roots.push(src);
  }
  return roots;
}

const TEST_FILE = /\.(test|spec)\.tsx?$|[\\/]__tests__[\\/]/;

function walk(dir, out) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    if (entry.name === "node_modules") continue;
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(full, out);
    else if (/\.(ts|tsx|mts)$/.test(entry.name) && !TEST_FILE.test(full)) {
      out.push(full);
    }
  }
  return out;
}

// ─── parsing and constant resolution ───────────────────────────────────────

/** file → { sf, consts: Map<name, node>, imports: Map<local, {from, name}> } */
const parsed = new Map();

function load(file) {
  let entry = parsed.get(file);
  if (entry) return entry;
  const sf = ts.createSourceFile(
    file,
    readFileSync(file, "utf8"),
    ts.ScriptTarget.Latest,
    true,
    file.endsWith(".tsx") ? ts.ScriptKind.TSX : ts.ScriptKind.TS,
  );
  const consts = new Map();
  const imports = new Map();
  const visit = (node) => {
    if (
      ts.isVariableDeclaration(node) &&
      ts.isIdentifier(node.name) &&
      node.initializer &&
      !consts.has(node.name.text)
    ) {
      consts.set(node.name.text, unwrap(node.initializer));
    }
    if (
      ts.isImportDeclaration(node) &&
      node.importClause?.namedBindings &&
      ts.isNamedImports(node.importClause.namedBindings)
    ) {
      for (const el of node.importClause.namedBindings.elements) {
        imports.set(el.name.text, {
          from: node.moduleSpecifier.text,
          name: (el.propertyName ?? el.name).text,
        });
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(sf);
  entry = { sf, consts, imports };
  parsed.set(file, entry);
  return entry;
}

function unwrap(node) {
  while (
    ts.isParenthesizedExpression(node) ||
    ts.isAsExpression(node) ||
    ts.isSatisfiesExpression?.(node)
  ) {
    node = node.expression;
  }
  return node;
}

/** `@/x` is `apps/web/src/x` in the web app; everything else is relative. */
function resolveModule(fromFile, specifier) {
  let base;
  if (specifier.startsWith("."))
    base = path.resolve(path.dirname(fromFile), specifier);
  else if (specifier.startsWith("@/"))
    base = path.join(repoRoot, "apps/web/src", specifier.slice(2));
  else return null;
  for (const candidate of [
    base,
    `${base}.ts`,
    `${base}.tsx`,
    path.join(base, "index.ts"),
  ]) {
    if (existsSync(candidate) && candidate.match(/\.tsx?$/)) return candidate;
  }
  return null;
}

/** The literal a name refers to, as `{ file, node }`, or null. */
function resolveName(file, name, seen = new Set()) {
  const key = `${file}#${name}`;
  if (seen.has(key)) return null;
  seen.add(key);
  const { consts, imports } = load(file);
  if (consts.has(name)) {
    const node = consts.get(name);
    if (ts.isIdentifier(node)) return resolveName(file, node.text, seen);
    return { file, node };
  }
  const imported = imports.get(name);
  if (imported) {
    const target = resolveModule(file, imported.from);
    if (target) return resolveName(target, imported.name, seen);
  }
  return null;
}

const isStringish = (node) =>
  ts.isStringLiteral(node) ||
  ts.isNoSubstitutionTemplateLiteral(node) ||
  ts.isTemplateExpression(node);

/**
 * Renders a string literal to its text, with a source map: `pieces` maps each
 * run of output back to the file and offset it came from, so an error inside
 * an interpolated selection can name the constant's own line.
 */
function render(file, node, via = null, depth = 0) {
  const pieces = [];
  const unresolved = [];
  let text = "";
  const push = (chunk, pos, from = file) => {
    pieces.push({ start: text.length, file: from, pos, via });
    text += chunk;
  };
  if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) {
    push(node.text, node.getStart() + 1);
  } else {
    push(node.head.text, node.head.getStart() + 1);
    for (const span of node.templateSpans) {
      const expr = unwrap(span.expression);
      const target = ts.isIdentifier(expr)
        ? resolveName(file, expr.text)
        : null;
      if (target && isStringish(target.node) && depth < 16) {
        const inner = render(
          target.file,
          target.node,
          via ?? expr.text,
          depth + 1,
        );
        for (const piece of inner.pieces) {
          pieces.push({ ...piece, start: piece.start + text.length });
        }
        text += inner.text;
        unresolved.push(...inner.unresolved);
      } else {
        unresolved.push({
          file,
          pos: span.expression.getStart(),
          expr: span.expression.getText(),
        });
        // A placeholder that parses where a selection or argument would, so
        // the rest of the operation is still checked.
        text += " __unresolved__ ";
      }
      push(span.literal.text, span.literal.getStart() + 1);
    }
  }
  return { text, pieces, unresolved };
}

function locate(file, pos) {
  const { sf } = load(file);
  const { line, character } = sf.getLineAndCharacterOfPosition(pos);
  return { file, line: line + 1, column: character + 1 };
}

/** Maps a graphql-js location inside `rendered.text` back to the source. */
function mapLocation(rendered, gqlLine, gqlColumn) {
  const lines = rendered.text.split("\n");
  let offset = gqlColumn - 1;
  for (let i = 0; i < gqlLine - 1; i += 1) offset += lines[i].length + 1;
  let piece = rendered.pieces[0];
  for (const candidate of rendered.pieces) {
    if (candidate.start <= offset) piece = candidate;
  }
  return {
    ...locate(piece.file, piece.pos + (offset - piece.start)),
    via: piece.via,
  };
}

// ─── --operations: find and validate ───────────────────────────────────────

const OPERATION_HEAD =
  /^\s*(?:#[^\n]*\n\s*)*(?:(?:query|mutation|subscription)\s*(?:[_A-Za-z]\w*)?\s*[({@]|fragment\s+[_A-Za-z]\w*\s+on\s)/;

const TRANSPORTS = new Set(["postGraphQL", "postGraphQLMultipart"]);

const sdlFile = path.join(repoRoot, SDL_PATH);
if (!existsSync(sdlFile)) {
  process.stdout.write(
    `${SDL_PATH} does not exist. Run this script with --schema --fix.\n`,
  );
  done(true);
}
const sdl = readFileSync(sdlFile, "utf8");
const schema = buildSchema(sdl);

const files = sourceRoots().flatMap((root) => walk(root, []));
const failures = [];
/** `${file}:${pos}` of every literal validated as an operation. */
const validated = new Set();
let operationCount = 0;

const fmt = (loc) => `${relative(loc.file)}:${loc.line}:${loc.column}`;

function checkLiteral(file, node) {
  const rendered = render(file, node);
  if (!OPERATION_HEAD.test(rendered.text)) return;
  validated.add(`${file}:${node.getStart()}`);
  operationCount += 1;
  const site = locate(file, node.getStart());
  const nameMatch = rendered.text.match(
    /^\s*(?:#[^\n]*\n\s*)*(?:query|mutation|subscription|fragment)\s+([_A-Za-z]\w*)/,
  );
  const name = nameMatch ? nameMatch[1] : "(anonymous)";

  for (const miss of rendered.unresolved) {
    failures.push({
      site,
      name,
      message: `cannot resolve \`\${${miss.expr}}\` to a string constant (at ${fmt(locate(miss.file, miss.pos))})`,
    });
  }
  let document;
  try {
    document = parse(new Source(rendered.text, relative(file)));
  } catch (error) {
    const at = error.locations?.[0];
    const where = at ? mapLocation(rendered, at.line, at.column) : site;
    failures.push({ site: where, name, message: `syntax: ${error.message}` });
    return;
  }
  if (rendered.unresolved.length > 0) return;
  for (const error of validate(schema, document)) {
    const at = error.locations?.[0];
    const where = at ? mapLocation(rendered, at.line, at.column) : site;
    failures.push({
      site: where,
      name,
      operationSite: site,
      via: where.via,
      message: error.message,
    });
  }
}

/** A transport's query argument must be an operation validated above. */
function checkTransportArgument(file, callNode, argument) {
  let target = unwrap(argument);
  let targetFile = file;
  if (ts.isIdentifier(target)) {
    const resolved = resolveName(file, target.text);
    if (resolved) {
      target = resolved.node;
      targetFile = resolved.file;
    }
  }
  if (
    isStringish(target) &&
    validated.has(`${targetFile}:${target.getStart()}`)
  )
    return;
  // A parameter being forwarded (the transport's own wrappers) is checked at
  // the wrapper's call sites, not here.
  if (ts.isIdentifier(target)) return;
  failures.push({
    site: locate(file, callNode.getStart()),
    name: "(unrecognised)",
    message: `this GraphQL call's query is not an operation the check could read: \`${argument.getText().slice(0, 60)}\``,
  });
}

for (const file of files) {
  const { sf } = load(file);
  const visit = (node) => {
    if (isStringish(node)) checkLiteral(file, node);
    ts.forEachChild(node, visit);
  };
  visit(sf);
}
for (const file of files) {
  const { sf } = load(file);
  const visit = (node) => {
    if (ts.isCallExpression(node)) {
      const callee = node.expression;
      const calleeName = ts.isIdentifier(callee)
        ? callee.text
        : ts.isPropertyAccessExpression(callee)
          ? callee.name.text
          : null;
      if (TRANSPORTS.has(calleeName) && node.arguments[0]) {
        checkTransportArgument(file, node, node.arguments[0]);
      }
      const first = node.arguments[0] && unwrap(node.arguments[0]);
      if (
        calleeName === "subscribe" &&
        first &&
        ts.isObjectLiteralExpression(first)
      ) {
        const query = first.properties.find(
          (p) => ts.isPropertyAssignment(p) && p.name.getText() === "query",
        );
        if (query) checkTransportArgument(file, node, query.initializer);
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(sf);
}

// ─── report ────────────────────────────────────────────────────────────────

failures.sort((a, b) =>
  fmt(a.site).localeCompare(fmt(b.site), "en", { numeric: true }),
);
for (const failure of failures) {
  let line = `${fmt(failure.site)}  ${failure.name}: ${failure.message}`;
  if (failure.via)
    line += `\n    (in \`\${${failure.via}}\`, used by ${failure.name} at ${fmt(failure.operationSite)})`;
  process.stdout.write(`${line}\n`);
}

warnBreakingChanges(sdl);

if (failures.length > 0) {
  process.stdout.write(
    `\n${failures.length} problem(s) in ${operationCount} operations across ${files.length} files, against ${SDL_PATH}.\n`,
  );
  done(true);
}
process.stdout.write(
  `${operationCount} operations across ${files.length} files are valid against ${SDL_PATH}.\n`,
);
done(false);
