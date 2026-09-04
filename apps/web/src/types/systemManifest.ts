/**
 * The shape of a game system pack's manifest, as this application reads it.
 *
 * # Why this is types and nothing else
 *
 * These declarations used to sit in `contexts/game-system-context.ts` beside a
 * React context, a provider, an in-memory cache and a `loadManifest` that did:
 *
 * ```ts
 * await import(`@/systems/${systemId}/index`)
 * ```
 *
 * `apps/web/src/systems/` has never existed, so that call could only ever
 * throw. Nothing mounted the provider and nothing called the hook, which is
 * why four months passed without anyone noticing — the three files that
 * imported from it wanted `SystemManifest` and nothing more.
 *
 * It was deleted rather than repaired, for the same reason `useSystemHooks`
 * was (spec 032 T107): a per-system dynamic import is the wrong mechanism now,
 * not merely a broken one. A pack is discovered at **build time** — the linker
 * on the server via `inventory`, `import.meta.glob` in the browser — because
 * ADR-029 says the product runs only code it compiled, and a runtime import
 * keyed by a system id is what that decision rules out. Leaving a dead
 * implementation of the rejected approach in the tree is how it comes back.
 *
 * What survives is the part that was doing work: the manifest's shape.
 */

/**
 * Spec 016 (FR-001, contracts/manifest-legal-schema.md): a system pack's
 * required, structured legal/attribution metadata — the render-ready
 * expansion of the manifest's loose free-text `license` string. Required
 * on every manifest (FR-003/FR-007); a pack missing it fails server-side
 * validation before it ever reaches this type.
 */
export type SystemManifestLegal = {
  licenseName: string;
  attributionText: string;
  requiredNotice?: string | null;
  disclaimer?: string | null;
  trademarkRestrictions?: string[];
  requiredUiPlacement?: string | null;
  sourceUrl?: string | null;
};

/**
 * Each system pack exports its own manifest, so beyond the four keys every
 * pack must publish the contents are whatever that pack chose. The index
 * signature is `unknown` rather than `any` on purpose: a consumer reaching for
 * a pack-specific table (`sizeCategories`, `abilityFacets`, …) has to say what
 * shape it expects at the point it reads it, instead of every such read
 * silently type-checking.
 */
export type SystemManifest = {
  id: string;
  title: string;
  version: string;
  legal: SystemManifestLegal;
  [key: string]: unknown;
};
