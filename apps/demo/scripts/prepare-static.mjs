// Writes the static files the demo answers `/api/systems`,
// `/api/interface-packs` and their manifests from, and the notice that ships
// beside the maps (spec 074 FR-018).
//
// The manifests are the packs' own files, copied byte for byte: the server
// serves `packs/systems/<id>/system.json` as `manifest.json`, and so does
// this. Only the two listings are made here, with the fields the server's
// listings carry.
import { cp, mkdir, readdir, readFile, rm, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { ensureDiceBuild } from "../../../scripts/shared.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(here, "../../..");
const out = path.resolve(here, "../public");

async function manifests(kind, file) {
  const root = path.join(repo, "packs", kind);
  const found = [];
  for (const entry of (await readdir(root, { withFileTypes: true })).sort(
    (a, b) => a.name.localeCompare(b.name),
  )) {
    const source = path.join(root, entry.name, file);
    if (!entry.isDirectory() || !existsSync(source)) continue;
    found.push({
      source,
      manifest: JSON.parse(await readFile(source, "utf8")),
    });
  }
  return found;
}

await rm(path.join(out, "packs"), { recursive: true, force: true });

const systems = await manifests("systems", "system.json");
for (const { source, manifest } of systems) {
  const dir = path.join(out, "packs/systems", manifest.id);
  await mkdir(dir, { recursive: true });
  await cp(source, path.join(dir, "manifest.json"));
}
await writeFile(
  path.join(out, "packs/systems.json"),
  JSON.stringify({
    systems: systems.map(({ manifest }) => ({
      id: manifest.id,
      title: manifest.title,
      description: manifest.description,
      version: manifest.version,
    })),
  }),
);

const interfaces = await manifests("interface", "interface.json");
for (const { source, manifest } of interfaces) {
  const dir = path.join(out, "packs/interface", manifest.id);
  await mkdir(dir, { recursive: true });
  await cp(source, path.join(dir, "manifest.json"));
}
await writeFile(
  path.join(out, "packs/interface-packs.json"),
  JSON.stringify(
    interfaces.map(({ manifest }) => ({
      id: manifest.id,
      title: manifest.title,
      version: manifest.version,
      description: manifest.description,
      targets: manifest.targets ?? [],
    })),
  ),
);

const credit = JSON.parse(
  await readFile(path.resolve(here, "../credit.json"), "utf8"),
);
await mkdir(path.join(out, "maps"), { recursive: true });
await writeFile(
  path.join(out, "maps/NOTICE.txt"),
  [
    `The maps in this directory are by ${credit.author},`,
    `licensed under ${credit.licence} (${credit.licenceUrl}).`,
    `Source: ${credit.source}`,
    `More of them: ${credit.catalog}`,
    "",
    "They were resized and re-encoded to fit a scene; nothing else was changed.",
    `These copies are offered under the same licence, ${credit.licence}.`,
    "",
  ].join("\n"),
);

// The dice the in-page backend rolls with: `crates/thunderforge-dice`, built
// for the browser. Skipped when the build is current.
await ensureDiceBuild();

if (!existsSync(path.join(out, "maps/maps.json"))) {
  console.error(
    "[demo] public/maps/maps.json is missing. Run `pnpm -F @thunderforge/demo run maps` first.",
  );
  process.exit(1);
}
console.log(
  `[demo] ${systems.length} systems, ${interfaces.length} interface packs, notice written`,
);

// Spec 079: the fight is the server's rules compiled to wasm, and the page
// imports them. Built here when missing or stale; a no-op when up to date.
const { ensureCombatBuild } = await import(
  path.join(repo, "scripts/shared.mjs")
);
await ensureCombatBuild();
