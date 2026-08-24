/**
 * Fate Core Web Package - System Manifest Export
 * Mirrors packs/systems/dnd5e/web/src/index.ts's structure.
 */

export interface FateSystemManifest {
  id: string;
  title: string;
  version: string;
  components: {
    CharacterSheet: React.ComponentType<any>;
  };
}

import CharacterSheet from './components/CharacterSheet';

export const fateSystemManifest: FateSystemManifest = {
  id: 'fate_core',
  title: 'Fate Core',
  version: '0.1.0',
  components: {
    CharacterSheet,
  },
};

export default fateSystemManifest;
