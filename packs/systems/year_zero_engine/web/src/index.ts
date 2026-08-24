/**
 * Year Zero Engine Web Package - System Manifest Export
 * Mirrors packs/systems/dnd5e/web/src/index.ts's structure.
 */

export interface YzeSystemManifest {
  id: string;
  title: string;
  version: string;
  components: {
    CharacterSheet: React.ComponentType<any>;
  };
}

import CharacterSheet from './components/CharacterSheet';

export const yzeSystemManifest: YzeSystemManifest = {
  id: 'year_zero_engine',
  title: 'Year Zero Engine',
  version: '0.1.0',
  components: {
    CharacterSheet,
  },
};

export default yzeSystemManifest;
