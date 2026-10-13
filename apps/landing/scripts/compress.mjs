// Spec 090 US4 (FR-020 to FR-023): thunderforge.dev serves its assets small,
// from stock nginx, which has no brotli module.
//
// Run after both builds (the landing's and the demo's). It writes:
//   - `<file>.br` (quality 11) beside every compressible file in the landing's
//     `dist/assets` and the demo's `data/demo/assets`, which nginx sends when
//     the browser accepts br (nginx.conf.template);
//   - `bytes.br` and `bytes.gz` into `data/demo/engine.json`, measured from
//     the engine wasm the demo build named there;
//   - with `--marketing`, `marketing/landing-engine.json`: the same numbers,
//     generated, never transcribed. Only from a release engine; a dev-profile
//     wasm is several times larger and would publish the wrong size.
//
// A `.br` newer than its source is kept, so a second run costs nothing.
// `BROTLI_QUALITY` lowers the quality for a quick local run.
import { existsSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join, relative, resolve } from "node:path";
import { promisify } from "node:util";
import { brotliCompress, constants, gzip } from "node:zlib";

const brotli = promisify(brotliCompress);
const gz = promisify(gzip);

const HERE = new URL(".", import.meta.url).pathname;
const ROOT = resolve(HERE, "../../..");
const DIRS = [resolve(HERE, "../dist/assets"), join(ROOT, "data/demo/assets")];
const MANIFEST = join(ROOT, "data/demo/engine.json");
const MARKETING = join(ROOT, "marketing/landing-engine.json");
const COMPRESSIBLE = /\.(js|mjs|css|wasm|svg|json|html|txt)$/;
const QUALITY = Number(process.env.BROTLI_QUALITY ?? 11);
// nginx's gzip_comp_level in nginx.conf.template; bytes.gz is what it sends.
const GZIP_LEVEL = 6;

const br = (bytes) =>
  brotli(bytes, {
    params: {
      [constants.BROTLI_PARAM_QUALITY]: QUALITY,
      [constants.BROTLI_PARAM_SIZE_HINT]: bytes.length,
    },
  });

async function compressDir(dir) {
  if (!existsSync(dir)) {
    console.log(`compress: ${relative(ROOT, dir)} is not built, skipped`);
    return { written: 0, kept: 0 };
  }
  let written = 0;
  let kept = 0;
  await Promise.all(
    readdirSync(dir)
      .filter((f) => COMPRESSIBLE.test(f))
      .map(async (f) => {
        const src = join(dir, f);
        const out = `${src}.br`;
        if (existsSync(out) && statSync(out).mtimeMs >= statSync(src).mtimeMs) {
          kept++;
          return;
        }
        writeFileSync(out, await br(readFileSync(src)));
        written++;
      }),
  );
  console.log(`compress: ${relative(ROOT, dir)}: ${written} written, ${kept} up to date`);
  return { written, kept };
}

for (const dir of DIRS) await compressDir(dir);

if (!existsSync(MANIFEST)) {
  console.log("compress: data/demo/engine.json is not built, no engine sizes");
  process.exit(0);
}
const manifest = JSON.parse(readFileSync(MANIFEST, "utf8"));
const wasm = readFileSync(join(ROOT, "data/demo", manifest.wasm));
if (wasm.length !== manifest.bytes.raw) {
  throw new Error(
    `compress: ${manifest.wasm} is ${wasm.length} bytes, engine.json says ${manifest.bytes.raw}`,
  );
}
const brPath = join(ROOT, "data/demo", `${manifest.wasm}.br`);
manifest.bytes.br = statSync(brPath).size;
manifest.bytes.gz = (await gz(wasm, { level: GZIP_LEVEL })).length;
writeFileSync(MANIFEST, `${JSON.stringify(manifest, null, 2)}\n`);

const mb = (n) => `${(n / 1e6).toFixed(1)} MB`;
console.log(
  `compress: engine ${manifest.version}: ${mb(manifest.bytes.raw)} raw, ` +
    `${mb(manifest.bytes.gz)} gzip, ${mb(manifest.bytes.br)} brotli (q${QUALITY})`,
);

if (process.argv.includes("--marketing")) {
  if (QUALITY !== 11) throw new Error("compress: --marketing needs BROTLI_QUALITY 11");
  writeFileSync(
    MARKETING,
    `${JSON.stringify(
      {
        generatedAt: new Date().toISOString(),
        generatedBy: "node apps/landing/scripts/compress.mjs --marketing",
        measures:
          "spec 090 FR-023: the engine a thunderforge.dev visitor downloads once, shared with the demo",
        engine: { version: manifest.version, wasm: manifest.wasm },
        bytes: manifest.bytes,
        budget: { brotliConcernBytes: 100e6 },
      },
      null,
      2,
    )}\n`,
  );
  console.log(`compress: wrote ${relative(ROOT, MARKETING)}`);
}
