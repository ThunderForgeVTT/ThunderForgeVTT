#!/usr/bin/env node
/**
 * Spec 066, FR-005: the shape of a system pack, written as a check.
 *
 * A pack is `system.json`, and at most a `server/` crate, a `web/` half,
 * `seed-content/`, a README and data files its own crate reads. That is the
 * contract in `packs/systems/README.md`, and Roll for Shoes is the pack that
 * has exactly it.
 *
 * Until 2026-10-04 seven packs also carried an `engine/` crate nothing
 * depended on, and five a `web/` package the host could find nothing in.
 * Nobody decided to keep them. Each pack was started by copying the one
 * before, and a copy keeps what it is given. Deleting them is worth little if
 * the ninth pack is copied from memory of the old shape, so this refuses the
 * shape in the commit that adds it.
 *
 * # Why it reads tracked files
 *
 * It asks git what the pack holds rather than the disk, so an ignored
 * `node_modules` or `target` left behind by a build is not mistaken for part
 * of the pack. The rule is about what is committed.
 */

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const SYSTEMS = "packs/systems/";

/**
 * Where each pack's server crate is linked: the application, and the server
 * library's own test binary. A crate missing from the first contributes
 * nothing to the product; one missing from the second contributes nothing to
 * the tests that would have noticed.
 */
export const LINKAGES = [
  "apps/server/src/system_packs.rs",
  "crates/thunderforge-server/src/test_packs.rs",
];

/**
 * What the host looks for in a pack's `web/`, as the contract writes them.
 *
 * These mirror the build-time globs in `apps/web/src`: `systemActorSheets.ts`,
 * `systemStatBlocks.ts` and `panels/systemPanels.ts`. A `web/` with none of
 * them is code nothing mounts.
 */
export const DISCOVERED_WEB_ENTRIES = [
  "web/src/ActorSheet.tsx",
  "web/src/StatBlocks.ts",
  "web/src/panels/<slot>.tsx",
];

const DISCOVERED = [
  /^web\/src\/ActorSheet\.tsx$/,
  /^web\/src\/StatBlocks\.ts$/,
  /^web\/src\/panels\/[^/]+\.tsx$/,
];

/**
 * Where the host declares its panel slots. The list is read from there rather
 * than repeated here, so a slot added to the host is one a pack may fill the
 * moment it exists, and one removed is refused the moment it is gone.
 */
export const SLOT_SOURCE = "apps/web/src/host/index.ts";

/** The members of `export type PanelSlot = | "a" | "b";` in the host's source. */
export function panelSlots(source) {
  const declared = /export type PanelSlot =([^;]*);/.exec(source);
  return declared
    ? [...declared[1].matchAll(/"([^"]+)"/g)].map((match) => match[1])
    : [];
}

/**
 * The contract, which carries the list of keys a manifest may have. Read from
 * there rather than repeated here, so the document an author reads and the
 * list they are held to cannot drift apart.
 */
export const CONTRACT = "packs/systems/README.md";

/** Every `key` between the contract's `<!-- manifest-keys -->` markers. */
export function manifestKeys(contract) {
  const listed =
    /<!-- manifest-keys -->([\s\S]*?)<!-- \/manifest-keys -->/.exec(contract);
  if (!listed) return [];
  return listed[1]
    .split("\n")
    .filter((line) => line.startsWith("|"))
    .flatMap((row) =>
      [...row.split("|")[1].matchAll(/`([^`]+)`/g)].map((match) => match[1]),
    );
}

/** Problems with one manifest's top-level keys. */
function keyProblems(manifest, source, keys) {
  let declared;
  try {
    declared = JSON.parse(source);
  } catch {
    declared = null;
  }
  if (declared === null || typeof declared !== "object") {
    return [`${manifest}: is not a JSON object, so nothing can read it`];
  }
  const unlisted = Object.keys(declared).filter((key) => !keys.includes(key));
  if (unlisted.length === 0) return [];
  if (keys.length === 0) {
    return [
      `${manifest}: cannot be checked, because no manifest-keys list was found in ${CONTRACT}`,
    ];
  }
  return unlisted.map(
    (key) =>
      `${manifest}: "${key}" is not a key the contract describes, so nothing reads it. ` +
      `A manifest carries what ${CONTRACT} lists under "Every key"; ` +
      `describe it there with what reads it, or take it out`,
  );
}

/** Top-level entries any pack may have. Anything else must earn its place. */
const LISTED = new Set([
  "system.json",
  "server",
  "web",
  "seed-content",
  "README.md",
]);

/** The name the pack's server crate is linked by: its lib name, else its package's. */
function crateName(manifest) {
  const section = (name) =>
    new RegExp(
      `^\\[${name}\\]\\s*$([\\s\\S]*?)(?=^\\[|(?![\\s\\S]))`,
      "m",
    ).exec(manifest);
  const nameIn = (block) =>
    block ? /^\s*name\s*=\s*"([^"]+)"/m.exec(block[1]) : null;
  const found = nameIn(section("lib")) ?? nameIn(section("package"));
  return found ? found[1].replaceAll("-", "_") : null;
}

/**
 * Problems with the shape of the system packs.
 *
 * `files` is every tracked path in the repository, repo-relative; `read`
 * returns the contents of one. Both are passed in so the tests can hand over
 * a tree without building it on disk.
 */
export function packProblems(files, read) {
  const packs = new Map();
  for (const file of files) {
    if (!file.startsWith(SYSTEMS)) continue;
    const [id, ...rest] = file.slice(SYSTEMS.length).split("/");
    // A file directly in `packs/systems/` — the contract itself — is not a pack.
    if (rest.length === 0) continue;
    if (!packs.has(id)) packs.set(id, []);
    packs.get(id).push(rest.join("/"));
  }

  const linked = LINKAGES.map((file) => [
    file,
    read(file)
      .split("\n")
      .map((line) => line.trim()),
  ]);
  const slots = panelSlots(read(SLOT_SOURCE));
  const keys = manifestKeys(read(CONTRACT));
  const problems = [];

  for (const [id, inside] of [...packs].sort(([a], [b]) =>
    a.localeCompare(b),
  )) {
    const pack = `${SYSTEMS}${id}`;
    const entries = new Set(inside.map((file) => file.split("/")[0]));

    if (!inside.includes("system.json")) {
      problems.push(
        `${pack}: a system pack must have a system.json; a directory without one is not a pack`,
      );
    } else {
      problems.push(
        ...keyProblems(
          `${pack}/system.json`,
          read(`${pack}/system.json`),
          keys,
        ),
      );
    }

    const serverSources = inside.filter((file) =>
      /^server\/src\/.*\.rs$/.test(file),
    );
    for (const entry of [...entries].sort()) {
      if (LISTED.has(entry)) continue;
      if (entry === "engine") {
        problems.push(
          `${pack}/engine: a pack has no engine crate. A pack extends the engine with data, ` +
            `not code (ADR-062), and nothing can load one into the browser`,
        );
        continue;
      }
      const isFile = inside.includes(entry);
      const isRead =
        isFile &&
        serverSources.some((source) =>
          read(`${pack}/${source}`).includes(entry),
        );
      if (!isRead) {
        problems.push(
          `${pack}/${entry}: not part of a pack. A pack holds ${[...LISTED].join(", ")}, ` +
            `and data files its own server crate reads by name`,
        );
      }
    }

    if (
      entries.has("web") &&
      !inside.some((file) => DISCOVERED.some((entry) => entry.test(file)))
    ) {
      problems.push(
        `${pack}/web: the host finds nothing in it. It looks for ${DISCOVERED_WEB_ENTRIES.join(", ")}; ` +
          `a pack with none of these has no web/ and its sheet is drawn from the manifest`,
      );
    }

    for (const file of inside) {
      const panel = /^web\/src\/panels\/([^/]+)\.tsx$/.exec(file);
      if (!panel || slots.includes(panel[1])) continue;
      problems.push(
        slots.length === 0
          ? `${pack}/${file}: cannot be checked, because no PanelSlot list was found in ${SLOT_SOURCE}`
          : `${pack}/${file}: "${panel[1]}" is not a panel slot, so the host never mounts it. ` +
              `The slots are ${slots.join(", ")}`,
      );
    }

    if (entries.has("server")) {
      if (!inside.includes("server/Cargo.toml")) {
        problems.push(
          `${pack}/server: has no Cargo.toml, so it is not a crate the application can link`,
        );
      } else {
        const name = crateName(read(`${pack}/server/Cargo.toml`));
        const line = `use ${name} as _;`;
        for (const [file, lines] of linked) {
          if (name && lines.includes(line)) continue;
          problems.push(
            `${pack}/server: ${file} does not link it. Add \`${line}\` there ` +
              `(and the dependency beside it), or what the crate contributes is never collected`,
          );
        }
      }
    }
  }
  return problems;
}

function main() {
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
  const files = execFileSync(
    "git",
    ["ls-files", "-z", "--", SYSTEMS, ...LINKAGES],
    {
      cwd: root,
      encoding: "utf8",
      maxBuffer: 64 * 1024 * 1024,
    },
  )
    .split("\0")
    .filter(Boolean);
  const read = (file) => {
    try {
      return readFileSync(path.join(root, file), "utf8");
    } catch {
      // Tracked but deleted in the working tree: the commit being made removes it.
      return "";
    }
  };
  // A file deleted in the working tree is still tracked until the commit lands.
  const present = files.filter((file) => {
    try {
      readFileSync(path.join(root, file));
      return true;
    } catch {
      return false;
    }
  });
  const problems = packProblems(present, read);
  if (problems.length > 0) {
    for (const problem of problems)
      process.stderr.write(`[packs] ${problem}\n`);
    process.stderr.write(
      `\n${problems.length} pack problem(s). The contract is packs/systems/README.md; ` +
        `see specs/066-one-way-to-be-a-pack/spec.md.\n`,
    );
    process.exit(1);
  }
  const packs = new Set(
    present
      .filter((file) => file.startsWith(SYSTEMS))
      .map((file) => file.split("/")[2]),
  );
  packs.delete("README.md");
  process.stdout.write(
    `packs: ${packs.size} system packs hold only what the contract lists\n`,
  );
}

if (process.argv[1] === fileURLToPath(import.meta.url)) main();
