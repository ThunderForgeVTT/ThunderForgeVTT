/**
 * Writes every preset hero to a directory:
 *
 *   node packages/heroes/src/cli.ts <out-dir>
 *
 * `<slug>-portrait.svg`, `<slug>-token.svg`, and `index.html`, a gallery for
 * looking at them side by side.
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { createHero, PRESET_HEROES } from "./index.ts";

const out = process.argv[2];
if (out === undefined) {
  process.stderr.write("usage: node src/cli.ts <out-dir>\n");
  process.exit(2);
}

mkdirSync(out, { recursive: true });
const cards: string[] = [];
for (const { slug, spec } of PRESET_HEROES) {
  const hero = createHero(spec);
  writeFileSync(join(out, `${slug}-portrait.svg`), hero.portrait());
  writeFileSync(join(out, `${slug}-token.svg`), hero.token());
  cards.push(`<figure>
    <img src="${slug}-portrait.svg" width="160" height="160" alt="">
    <img src="${slug}-token.svg" width="96" height="96" alt="">
    <figcaption>${slug}</figcaption>
  </figure>`);
}

writeFileSync(
  join(out, "index.html"),
  `<!doctype html><meta charset="utf-8"><title>ThunderForge heroes</title>
<style>
  body { font-family: system-ui, sans-serif; background: #1d1a24; color: #f2e8d8; margin: 24px; }
  main { display: grid; grid-template-columns: repeat(auto-fill, minmax(290px, 1fr)); gap: 20px; }
  figure { display: flex; align-items: center; gap: 12px; margin: 0; padding: 12px; background: #2a2533; border-radius: 12px; }
</style>
<main>
${cards.join("\n")}
</main>
`,
);
process.stdout.write(
  `wrote ${PRESET_HEROES.length * 2} SVGs and index.html to ${out}\n`,
);
