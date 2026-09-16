//! What Genie's character sheet is made of, declared rather than drawn.
//!
//! # Why this file exists
//!
//! `ActorSheet.tsx` was one card wrapping one tabbed component. On a
//! widescreen that read as a narrow column with four tabs hiding three
//! quarters of the character. The owner wanted Genie to be the page that
//! shows how flexible the configuration is, and a sheet you click through
//! shows one thing at a time by construction.
//!
//! So the sheet is a list of regions, and `ActorSheet.tsx` draws the list.
//! Moving Conditions beside the scores, or giving Notes the full width, is an
//! edit to a row here, not to JSX.
//!
//! # How this relates to the host's own layout format
//!
//! It is deliberately the same shape as `apps/web/src/sheet-layout/types.ts`:
//! a region's `kind` corresponds to that format's `badgeGrid` / `barStack` /
//! `rowList` / `block`, and `span` is the one thing that format lacks. It is
//! written in the pack because the host format cannot express this page yet;
//! `ActorSheet.tsx`'s header lists exactly what is missing.
//!
//! # Mirrored, not fetched
//!
//! The labels mirror `packs/systems/genie/system.json`'s `abilities` and
//! `resources` blocks, as `conditions.ts` mirrors `conditions` and
//! `derived-data.ts` mirrors `wishPoints`. That is this pack's convention for
//! manifest data the browser needs at render time. Keep the two in sync.

import type { GenieAbilityData, GenieResourceData } from './components/CharacterSheet';

/** One score, drawn as a badge. Mirrors `system.json`'s `abilities`. */
export interface GenieAttributeDeclaration {
  id: keyof GenieAbilityData;
  label: string;
  abbreviation: string;
}

export const GENIE_ATTRIBUTES: readonly GenieAttributeDeclaration[] = [
  { id: 'might', label: 'Might', abbreviation: 'MGT' },
  { id: 'cunning', label: 'Cunning', abbreviation: 'CUN' },
  { id: 'spirit', label: 'Spirit', abbreviation: 'SPI' },
];

/**
 * One pool, drawn as a bar. Mirrors `system.json`'s `resources`.
 *
 * `max` names a field rather than holding a number because both pools keep
 * their ceiling in `resource_data`, and Wish Points' ceiling moves with level.
 */
export interface GenieResourceDeclaration {
  id: string;
  label: string;
  current: keyof GenieResourceData;
  max: keyof GenieResourceData;
}

export const GENIE_RESOURCES: readonly GenieResourceDeclaration[] = [
  { id: 'health', label: 'Health', current: 'current_health', max: 'max_health' },
  {
    id: 'wishPoints',
    label: 'Wish Points',
    current: 'current_wish_points',
    max: 'max_wish_points',
  },
];

/** What a region draws. */
export type GenieRegionKind = 'identity' | 'scores' | 'pools' | 'conditions' | 'traits' | 'notes';

/**
 * A region of the sheet and how much of a row it wants.
 *
 * `span` is a number, not a class, because the renderer owns the breakpoints:
 * a declaration saying `lg:col-span-2` would be a pack reaching into the
 * host's grid. At 375px every region is one full-width column whatever this
 * says.
 */
export interface GenieSheetRegion {
  id: string;
  title: string;
  kind: GenieRegionKind;
  /** Columns out of three on a wide screen. */
  span: 1 | 2 | 3;
  /** One sentence saying what the region is, for someone who has not played. */
  blurb: string;
}

/**
 * Genie's sheet, in the order it is read at a table: who this is, what they
 * are good at, what they can spend, what is wrong with them now, the
 * slower-moving facts, and room to write.
 *
 * Rows of three: identity + scores, pools + conditions, traits + notes.
 *
 * No art region. The host page mounts the actor's imagery panel (portrait and
 * token) directly above this sheet on the edit route, for every system, so a
 * portrait here would be a second copy of the same picture.
 *
 * No skills region. `system.json` declares `skills: {}`, and an empty table is
 * not information. No scrolls or knacks either: the host draws those below the
 * sheet, and `@thunderforge/host` gives a pack no way to read them.
 */
export const GENIE_SHEET_REGIONS: readonly GenieSheetRegion[] = [
  {
    id: 'identity',
    title: 'Identity',
    kind: 'identity',
    span: 1,
    blurb: 'Who this is.',
  },
  {
    id: 'scores',
    title: 'Scores',
    kind: 'scores',
    span: 2,
    blurb: 'The three scores a Manifestation roll is built from.',
  },
  {
    id: 'pools',
    title: 'Resources',
    kind: 'pools',
    span: 2,
    blurb: 'What this character can spend, and what is left.',
  },
  {
    id: 'conditions',
    title: 'Conditions',
    kind: 'conditions',
    span: 1,
    blurb: 'What is true of them right now.',
  },
  {
    id: 'traits',
    title: 'Traits',
    kind: 'traits',
    span: 1,
    blurb: 'Level sets the Wish Point ceiling.',
  },
  {
    id: 'notes',
    title: 'Notes',
    kind: 'notes',
    span: 2,
    blurb: 'Anything the numbers do not say.',
  },
];
