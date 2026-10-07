/**
 * The cast's faces, drawn by `@thunderforge/heroes`.
 *
 * A real instance stores an actor's art as a file and serves it from
 * `/api/actor-assets/{id}`. The demo has no files to serve, so it keeps the
 * spec that draws each picture and draws it when it is asked for: an SVG for
 * the page's own `<img>` tags (`guard/images.ts`), and a PNG for the engine,
 * which decodes PNG and WebP and nothing else (`guard/rest.ts`). The same
 * spec draws the same face in every browser, so nothing here is random.
 *
 * The drawings are this project's own work and ship under its licence.
 */
import {
  createHero,
  matchRace,
  monsterSpec,
  randomHero,
  type HeroSpec,
} from "@thunderforge/heroes";

/** The two roles the web app renders (ADR-057). */
export type ArtRole = "token" | "portrait";

/** One stored picture: who it shows and which of their pictures it is. */
export interface ArtAsset {
  role: ArtRole;
  look: HeroSpec;
}

/**
 * A hero of the named race, rolled from their name so the roll never changes,
 * carrying what their class carries.
 */
export function heroLook(
  name: string,
  race: string,
  gear: Pick<HeroSpec, "headgear" | "prop">,
): HeroSpec {
  const rolled = randomHero(name, {
    race: matchRace(race),
    locked: { headgear: true, prop: true },
  });
  return { ...rolled, ...gear, name } as HeroSpec;
}

/** A creature as a stat block names it: "Goblin Warrior", "Small Fey". */
export function creatureLook(name: string, descriptor: string): HeroSpec {
  return monsterSpec({ name, descriptor });
}

const drawn = new Map<string, string>();

/** The picture as an SVG document, 256 units square. */
export function drawArt(asset: ArtAsset): string {
  const key = `${asset.role}|${JSON.stringify(asset.look)}`;
  let svg = drawn.get(key);
  if (svg === undefined) {
    const hero = createHero(asset.look);
    svg = asset.role === "token" ? hero.token() : hero.portrait();
    drawn.set(key, svg);
  }
  return svg;
}

/** Where the server would serve this asset; the demo answers the same. */
export function artUrl(assetId: string): string {
  return `/api/actor-assets/${assetId}`;
}

/**
 * The address a token's `photoUrl` carries. The engine picks its image loader
 * by extension, so the suffix is what it will decode: PNG, because that is
 * what a browser canvas can write.
 */
export function tokenPhotoUrl(assetId: string): string {
  return `${artUrl(assetId)}.png`;
}

/**
 * Reads an address the demo might have handed out for a picture. Accepts
 * every shape the server's own route does (`assets_serve::actor`): the bare
 * id, the id with an image suffix, its thumbnail, and Bevy's `.meta` probe.
 */
export function readArtPath(
  path: string,
): { assetId: string; meta: boolean } | null {
  const found =
    /^\/api\/actor-assets\/([0-9a-f-]{36})(?:\/thumb)?(?:\.png|\.webp)?(\.meta)?$/.exec(
      path,
    );
  if (!found) return null;
  return { assetId: found[1], meta: found[2] !== undefined };
}
