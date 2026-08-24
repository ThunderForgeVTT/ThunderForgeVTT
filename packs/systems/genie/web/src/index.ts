/**
 * Genie Web Package - System Manifest Export
 *
 * Exports the Genie system manifest for lazy-loading in the core web app.
 * Mirrors packs/systems/dnd5e/web/src/index.ts's structure (spec 018-genie-house-system).
 */

export interface GenieSystemManifest {
  id: string;
  title: string;
  version: string;
  components: {
    CharacterSheet: React.ComponentType<any>;
    ManifestationRollButton: React.ComponentType<any>;
    ConditionTrack: React.ComponentType<any>;
    SizeCategoryBadge: React.ComponentType<any>;
    SessionWishPool: React.ComponentType<any>;
    SessionClocks: React.ComponentType<any>;
    SessionResourceTrade: React.ComponentType<any>;
  };
  derivedDataCalculators: {
    maxWishPoints: (level: number) => number;
  };
}

import CharacterSheet from './components/CharacterSheet';
import ManifestationRollButton from './components/ManifestationRollButton';
import ConditionTrack from './components/ConditionTrack';
import SizeCategoryBadge from './components/SizeCategoryBadge';
import SessionWishPool from './components/SessionWishPool';
import SessionClocks from './components/SessionClocks';
import SessionResourceTrade from './components/SessionResourceTrade';
import { calculateMaxWishPoints } from './derived-data.ts';

// Named component + type exports, for apps/web (or any other consumer) to
// import individually rather than only through the bundled manifest below.
export {
  CharacterSheet,
  ManifestationRollButton,
  ConditionTrack,
  SizeCategoryBadge,
  SessionWishPool,
  SessionClocks,
  SessionResourceTrade,
};
export type {
  GenieCharacter,
  GenieAbilityData,
  GenieProficiencyData,
  GenieSkillDefinition,
  GenieResourceData,
} from './components/CharacterSheet';
export { calculateMaxWishPoints } from './derived-data.ts';
export type { ConditionTrackProps } from './components/ConditionTrack';
export type { SessionWishPoolProps } from './components/SessionWishPool';
export type { SessionClocksProps, GeniePuzzleClockData } from './components/SessionClocks';
export type {
  SessionResourceTradeProps,
  GenieResourceHoldingData,
  GeniePartyMemberOption,
  GenieIncomingTradeProposal,
} from './components/SessionResourceTrade';
export { GENIE_CONDITIONS, resolveCondition, resolveConditions } from './conditions';
export type { GenieConditionDefinition } from './conditions';

// Derived data calculators (spec 018-genie-house-system, US6)
// Mirrors packs/systems/dnd5e/web/src/index.ts's DerivedDataCalculators
// shape: `resource_data.max_wish_points` is recalculated on read from the
// character's current level via this pure function, the same way dnd5e's
// maxSpellSlots is recalculated on read rather than cached/stored.
export const DerivedDataCalculators = {
  maxWishPoints: calculateMaxWishPoints,
};

export const genieSystemManifest: GenieSystemManifest = {
  id: 'genie',
  title: 'Genie',
  version: '0.1.0',
  components: {
    CharacterSheet,
    ManifestationRollButton,
    ConditionTrack,
    SizeCategoryBadge,
    SessionWishPool,
    SessionClocks,
    SessionResourceTrade,
  },
  derivedDataCalculators: DerivedDataCalculators,
};

export default genieSystemManifest;
