/**
 * Getting a hero in and out as files (FR-013, FR-010, FR-026).
 *
 * A hero leaves as a spec or as drawings, and comes back only as a spec:
 * there is no SVG import, because a drawing is not a hero and any SVG can
 * claim to be one. The exported drawings carry the renderer's default ids, so
 * a preset exported here is byte for byte what `packages/heroes/src/cli.ts`
 * writes for it.
 */
import {
  minimalSpec,
  renderPortrait,
  renderToken,
  validateHero,
  type HeroSpec,
} from "@thunderforge/heroes";
import { toProblems, type HeroProblem } from "../problems.ts";

export interface HeroFile {
  name: string;
  type: string;
  text: string;
}

/** The three files a hero exports as, named after the hero. */
export function heroFiles(spec: HeroSpec): {
  portrait: HeroFile;
  token: HeroFile;
  json: HeroFile;
} {
  const stem = fileStem(spec.name);
  return {
    portrait: {
      name: `${stem}-portrait.svg`,
      type: "image/svg+xml",
      text: renderPortrait(spec),
    },
    token: {
      name: `${stem}-token.svg`,
      type: "image/svg+xml",
      text: renderToken(spec),
    },
    json: {
      name: `${stem}.hero.json`,
      type: "application/json",
      text: `${JSON.stringify(minimalSpec(spec), null, 2)}\n`,
    },
  };
}

/** "Sir Pip the Bold" → "sir-pip-the-bold"; never empty. */
export function fileStem(name: string): string {
  const stem = name
    .toLowerCase()
    .normalize("NFKD")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return stem === "" ? "hero" : stem;
}

/** A spec from pasted or loaded text, refused whole with every problem named
 * (FR-010). */
export function parseHeroText(
  text: string,
): { ok: true; spec: HeroSpec } | { ok: false; problems: HeroProblem[] } {
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    return {
      ok: false,
      problems: [{ field: "", message: "this is not a hero spec (not JSON)" }],
    };
  }
  const result = validateHero(parsed);
  if (!result.ok) return { ok: false, problems: toProblems(result.problems) };
  return { ok: true, spec: minimalSpec(parsed as HeroSpec) };
}

/** Hands a file to the browser to save. */
export function saveHeroFile(file: HeroFile): void {
  const url = URL.createObjectURL(new Blob([file.text], { type: file.type }));
  const link = document.createElement("a");
  link.href = url;
  link.download = file.name;
  link.hidden = true;
  document.body.append(link);
  link.click();
  // Removed and revoked a while later, not at once: a browser that has not
  // started the download by then saves nothing, and says nothing.
  setTimeout(() => {
    link.remove();
    URL.revokeObjectURL(url);
  }, 30_000);
}
