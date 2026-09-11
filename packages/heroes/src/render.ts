/**
 * The factory: a hero spec in, a portrait and a token out.
 *
 * Both are self-contained SVG strings, 256×256. The server rasterises them
 * to WebP on upload; a hero builder can drop them straight into the page,
 * which is why every id in them is prefixed — two heroes inlined side by side
 * must not share a gradient.
 */
import { shade } from "./color.ts";
import { figure, INK } from "./parts.ts";
import { resolveHero, type HeroSpec, type ResolvedHero } from "./spec.ts";

export interface RenderOptions {
  /** Prefix for the ids inside the SVG. Defaults to one derived from the
   * hero itself, so different heroes never clash and identical ones clash
   * harmlessly. */
  idPrefix?: string;
}

export interface Hero {
  /** The hero with every default filled in. */
  readonly spec: ResolvedHero;
  /** A square card: the hero on its backdrop, with a rounded frame. */
  portrait(options?: RenderOptions): string;
  /** A round token with a coloured rim, for the map. */
  token(options?: RenderOptions): string;
}

/** The hero `spec` describes. Throws `HeroSpecError` for a spec that is not
 * one; use `validateHero` first where the spec came from outside. */
export function createHero(spec: HeroSpec): Hero {
  const hero = resolveHero(spec);
  return Object.freeze({
    spec: hero,
    portrait: (options?: RenderOptions) => portraitSvg(hero, options),
    token: (options?: RenderOptions) => tokenSvg(hero, options),
  });
}

export function renderPortrait(
  spec: HeroSpec,
  options?: RenderOptions,
): string {
  return createHero(spec).portrait(options);
}

export function renderToken(spec: HeroSpec, options?: RenderOptions): string {
  return createHero(spec).token(options);
}

const ID_PREFIX = /^[A-Za-z][\w-]{0,63}$/;

function idPrefix(hero: ResolvedHero, options?: RenderOptions): string {
  const prefix = options?.idPrefix ?? `tfh-${fnv1a(JSON.stringify(hero))}`;
  if (!ID_PREFIX.test(prefix)) {
    throw new Error(`idPrefix must be a letter then letters, digits, - or _`);
  }
  return prefix;
}

/** 32-bit FNV-1a, as hex: short, stable, and plenty to tell heroes apart. */
function fnv1a(text: string): string {
  let hash = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) {
    hash ^= text.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193);
  }
  return (hash >>> 0).toString(16).padStart(8, "0");
}

function escapeXml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&apos;");
}

function label(hero: ResolvedHero): string {
  return escapeXml(hero.title ? `${hero.name}, ${hero.title}` : hero.name);
}

function backdrop(id: string, glow: string, radius: string): string {
  return `<radialGradient id="${id}" cx="50%" cy="38%" r="${radius}">
      <stop offset="0%" stop-color="${glow}"/>
      <stop offset="100%" stop-color="${shade(glow, 0.45)}"/>
    </radialGradient>`;
}

function portraitSvg(hero: ResolvedHero, options?: RenderOptions): string {
  const id = `${idPrefix(hero, options)}-portrait`;
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" width="256" height="256" role="img" aria-label="${label(hero)}">
  <title>${label(hero)}</title>
  <defs>
    ${backdrop(`${id}-bg`, hero.glow, "75%")}
    <clipPath id="${id}-card"><rect x="0" y="0" width="256" height="256" rx="28"/></clipPath>
  </defs>
  <g clip-path="url(#${id}-card)">
    <rect width="256" height="256" fill="url(#${id}-bg)"/>
    <circle cx="36" cy="40" r="3" fill="#ffffff" opacity="0.6"/>
    <circle cx="222" cy="30" r="2" fill="#ffffff" opacity="0.6"/>
    <circle cx="30" cy="190" r="2.5" fill="#ffffff" opacity="0.5"/>
${figure(hero)}
  </g>
  <rect x="1.5" y="1.5" width="253" height="253" rx="27" fill="none" stroke="${INK}" stroke-width="3"/>
</svg>
`;
}

function tokenSvg(hero: ResolvedHero, options?: RenderOptions): string {
  const id = `${idPrefix(hero, options)}-token`;
  const name = escapeXml(hero.name);
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" width="256" height="256" role="img" aria-label="${name}">
  <title>${name}</title>
  <defs>
    ${backdrop(`${id}-bg`, hero.glow, "70%")}
    <clipPath id="${id}-disc"><circle cx="128" cy="128" r="112"/></clipPath>
  </defs>
  <circle cx="128" cy="132" r="120" fill="#000000" opacity="0.25"/>
  <g clip-path="url(#${id}-disc)">
    <circle cx="128" cy="128" r="112" fill="url(#${id}-bg)"/>
${figure(hero)}
  </g>
  <circle cx="128" cy="128" r="114" fill="none" stroke="${hero.ring}" stroke-width="12"/>
  <circle cx="128" cy="128" r="120" fill="none" stroke="${INK}" stroke-width="3"/>
  <circle cx="128" cy="128" r="108" fill="none" stroke="${INK}" stroke-width="2"/>
</svg>
`;
}
