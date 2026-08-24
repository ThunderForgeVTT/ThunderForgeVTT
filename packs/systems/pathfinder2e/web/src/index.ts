/**
 * Pathfinder Second Edition (Remaster) Web Package - System Manifest Export
 * Mirrors packs/systems/dnd5e/web/src/index.ts's structure.
 */

export interface Pathfinder2eSystemManifest {
  id: string;
  title: string;
  version: string;
  components: {
    CharacterSheet: React.ComponentType<any>;
  };
}

import CharacterSheet from './components/CharacterSheet';

export const pathfinder2eSystemManifest: Pathfinder2eSystemManifest = {
  id: 'pathfinder2e',
  title: 'Pathfinder Second Edition (Remaster)',
  version: '0.1.0',
  components: {
    CharacterSheet,
  },
};

export default pathfinder2eSystemManifest;
