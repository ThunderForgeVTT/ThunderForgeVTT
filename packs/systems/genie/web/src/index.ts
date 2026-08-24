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
