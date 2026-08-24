/**
 * Blades in the Dark Web Package - System Manifest Export
 * Mirrors packs/systems/dnd5e/web/src/index.ts's structure.
 */

export interface BladesSystemManifest {
  id: string;
  title: string;
  version: string;
  components: {
    CharacterSheet: React.ComponentType<any>;
  };
}

import CharacterSheet from './components/CharacterSheet';

export const bladesSystemManifest: BladesSystemManifest = {
  id: 'blades_in_the_dark',
  title: 'Blades in the Dark',
  version: '0.1.0',
  components: {
    CharacterSheet,
  },
};

export default bladesSystemManifest;
