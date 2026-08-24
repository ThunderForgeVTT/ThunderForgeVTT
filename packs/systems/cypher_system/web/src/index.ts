/**
 * Cypher System Web Package - System Manifest Export
 * Mirrors packs/systems/dnd5e/web/src/index.ts's structure.
 */

export interface CypherSystemManifest {
  id: string;
  title: string;
  version: string;
  components: {
    CharacterSheet: React.ComponentType<any>;
  };
}

import CharacterSheet from './components/CharacterSheet';

export const cypherSystemManifest: CypherSystemManifest = {
  id: 'cypher_system',
  title: 'Cypher System',
  version: '0.1.0',
  components: {
    CharacterSheet,
  },
};

export default cypherSystemManifest;
