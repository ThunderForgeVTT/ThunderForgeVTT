// Spec 086 SC-007: telemetry costs the landing nothing up front, and little
// when it loads.
//
// Run at the end of `build`. Fails when
//   - the telemetry chunk is over 25 KB brotli, or
//   - the page's document asks for a different number of scripts than it did
//     before telemetry: one entry module and one shared preload. Telemetry is
//     imported only after the served config said yes, so it must never add
//     one.
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { brotliCompressSync, constants } from "node:zlib";

const DIST = new URL("../dist/", import.meta.url).pathname;
const LIMIT = 25 * 1024;
const EXPECTED = { scripts: 1, preloads: 1 };

const problems = [];

const chunks = readdirSync(join(DIST, "assets")).filter((f) =>
  /^telemetryChunk-.*\.js$/.test(f),
);
if (chunks.length !== 1) {
  problems.push(`expected one telemetry chunk in dist/assets, found ${chunks.length}`);
}
for (const chunk of chunks) {
  const raw = readFileSync(join(DIST, "assets", chunk));
  const size = brotliCompressSync(raw, {
    params: { [constants.BROTLI_PARAM_QUALITY]: 11 },
  }).length;
  const line = `${chunk}: ${(size / 1024).toFixed(2)} KB brotli (limit 25 KB)`;
  if (size > LIMIT) problems.push(line);
  else console.log(`check-telemetry-size: ${line}`);
}

const html = readFileSync(join(DIST, "index.html"), "utf8");
const scripts = html.match(/<script\b[^>]*\bsrc=/g)?.length ?? 0;
const preloads = html.match(/<link\b[^>]*rel="modulepreload"/g)?.length ?? 0;
if (scripts !== EXPECTED.scripts || preloads !== EXPECTED.preloads) {
  problems.push(
    `index.html loads ${scripts} script(s) and ${preloads} preload(s); ` +
      `expected ${EXPECTED.scripts} and ${EXPECTED.preloads}`,
  );
}
for (const name of chunks) {
  if (html.includes(name)) problems.push(`index.html names ${name}`);
}

if (problems.length) {
  for (const p of problems) console.error(`check-telemetry-size: ${p}`);
  process.exit(1);
}
console.log(
  `check-telemetry-size: index.html loads ${scripts} script and ${preloads} preload, as before`,
);
