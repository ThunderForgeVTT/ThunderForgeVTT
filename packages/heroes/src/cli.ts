/**
 * Writes every preset hero and every bestiary creature to a directory:
 *
 *   node packages/heroes/src/cli.ts <out-dir>
 *
 * `<slug>-portrait.svg`, `<slug>-token.svg`, and `index.html`, a gallery for
 * looking at them side by side. Heroes and monsters share the page on
 * purpose: the only way to tell whether a goblin belongs beside Sir Pip is to
 * put them next to each other and look.
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import {
  BESTIARY,
  createHero,
  createMonster,
  PRESET_HEROES,
  type Hero,
} from "./index.ts";

const argument = process.argv[2];
if (argument === undefined) {
  process.stderr.write("usage: node src/cli.ts <out-dir>\n");
  process.exit(2);
}
const out: string = argument;

mkdirSync(out, { recursive: true });
const cards: string[] = [];

function write(slug: string, caption: string, drawn: Hero): void {
  writeFileSync(join(out, `${slug}-portrait.svg`), drawn.portrait());
  writeFileSync(join(out, `${slug}-token.svg`), drawn.token());
  cards.push(`<figure>
    <img src="${slug}-portrait.svg" width="160" height="160" alt="">
    <img src="${slug}-token.svg" width="96" height="96" alt="">
    <figcaption>${caption}</figcaption>
  </figure>`);
}

for (const { slug, spec } of PRESET_HEROES) {
  write(slug, slug, createHero(spec));
}
for (const { slug, source } of BESTIARY) {
  const monster = createMonster(source);
  write(
    slug,
    `${slug} · ${monster.spec.size} · ${monster.footprint}◻`,
    monster,
  );
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
  `wrote ${cards.length * 2} SVGs and index.html to ${out}\n`,
);
