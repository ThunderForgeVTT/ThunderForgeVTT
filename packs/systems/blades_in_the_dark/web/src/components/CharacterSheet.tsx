import React from 'react';
import * as Tabs from '@radix-ui/react-tabs';

/** Blades in the Dark: the three attributes that aggregate action ratings (0-4 range). */
export interface BladesAttributes {
  insight: number;
  prowess: number;
  resolve: number;
}

/** Blades in the Dark: the twelve action ratings, each linked to one attribute. */
export interface BladesActionRatings {
  hunt: number;
  study: number;
  survey: number;
  tinker: number;
  finesse: number;
  prowl: number;
  skirmish: number;
  wreck: number;
  attune: number;
  command: number;
  consort: number;
  sway: number;
}

export interface BladesResources {
  stress: number;
  trauma: number;
  coin: number;
}

export interface BladesCharacter {
  id: string;
  name: string;
  playbook: string;
  attributes: BladesAttributes;
  actionRatings: BladesActionRatings;
  resources: BladesResources;
}

interface CharacterSheetProps {
  character?: BladesCharacter;
  isEditable?: boolean;
  onUpdate?: (character: Partial<BladesCharacter>) => void;
}

/** Maps each action rating to the attribute it aggregates into, per system.json's `skills`. */
const ACTIONS_BY_ATTRIBUTE: Record<keyof BladesAttributes, Array<keyof BladesActionRatings>> = {
  insight: ['hunt', 'study', 'survey', 'tinker'],
  prowess: ['finesse', 'prowl', 'skirmish', 'wreck'],
  resolve: ['attune', 'command', 'consort', 'sway'],
};

const ATTRIBUTE_LABELS: Record<keyof BladesAttributes, string> = {
  insight: 'Insight',
  prowess: 'Prowess',
  resolve: 'Resolve',
};

const ACTION_LABELS: Record<keyof BladesActionRatings, string> = {
  hunt: 'Hunt',
  study: 'Study',
  survey: 'Survey',
  tinker: 'Tinker',
  finesse: 'Finesse',
  prowl: 'Prowl',
  skirmish: 'Skirmish',
  wreck: 'Wreck',
  attune: 'Attune',
  command: 'Command',
  consort: 'Consort',
  sway: 'Sway',
};

/**
 * Blades in the Dark Character Sheet Component
 *
 * Main character display UI with tabbed interface:
 * - Attributes: Insight/Prowess/Resolve and their linked action ratings
 * - Resources: Stress, Trauma, Coin
 *
 * Uses RxDB to read/write character data; all derived stats calculated client-side.
 */
const CharacterSheet: React.FC<CharacterSheetProps> = ({
  character,
  isEditable = true,
  onUpdate,
}) => {
  const [selectedTab, setSelectedTab] = React.useState<string>('attributes');

  if (!character) {
    return (
      <div className="p-4 text-center text-gray-500">
        No character selected. Create or load a character to begin.
      </div>
    );
  }

  const handleActionRatingChange = (action: keyof BladesActionRatings, value: number) => {
    if (!isEditable || !onUpdate) return;
    onUpdate({
      actionRatings: {
        ...character.actionRatings,
        [action]: value,
      },
    });
  };

  const handleResourceChange = (resource: keyof BladesResources, value: number) => {
    if (!isEditable || !onUpdate) return;
    onUpdate({
      resources: {
        ...character.resources,
        [resource]: value,
      },
    });
  };

  return (
    <div className="w-full max-w-4xl mx-auto p-4 bg-white rounded-lg shadow-lg">
      {/* Header */}
      <div className="mb-6 border-b pb-4">
        <h1 className="text-3xl font-bold">{character.name}</h1>
        <p className="text-lg text-gray-600">{character.playbook}</p>
      </div>

      {/* Tabbed Interface */}
      <Tabs.Root value={selectedTab} onValueChange={setSelectedTab} className="w-full">
        <Tabs.List className="flex gap-2 border-b mb-4">
          <Tabs.Trigger
            value="attributes"
            className="px-4 py-2 font-semibold border-b-2 border-transparent data-[state=active]:border-blue-500 hover:text-blue-600"
          >
            Attributes
          </Tabs.Trigger>
          <Tabs.Trigger
            value="resources"
            className="px-4 py-2 font-semibold border-b-2 border-transparent data-[state=active]:border-blue-500 hover:text-blue-600"
          >
            Resources
          </Tabs.Trigger>
        </Tabs.List>

        {/* Attributes Tab: each attribute plus its linked action ratings */}
        <Tabs.Content value="attributes" className="p-4">
          {(Object.keys(ACTIONS_BY_ATTRIBUTE) as Array<keyof BladesAttributes>).map((attribute) => (
            <div key={attribute} className="mb-6">
              <div className="flex items-center gap-2 mb-2">
                <span className="text-xl font-bold">{ATTRIBUTE_LABELS[attribute]}</span>
                <span className="text-sm text-gray-500">
                  {character.attributes[attribute]}
                </span>
              </div>
              <div className="grid grid-cols-2 gap-2 pl-4">
                {ACTIONS_BY_ATTRIBUTE[attribute].map((action) => (
                  <div key={action} className="flex items-center justify-between gap-2">
                    <span>{ACTION_LABELS[action]}</span>
                    {isEditable ? (
                      <input
                        type="number"
                        min={0}
                        max={4}
                        value={character.actionRatings[action]}
                        onChange={(e) => handleActionRatingChange(action, Number(e.target.value))}
                        className="w-16 border rounded px-1"
                      />
                    ) : (
                      <span>{character.actionRatings[action]}</span>
                    )}
                  </div>
                ))}
              </div>
            </div>
          ))}
        </Tabs.Content>

        {/* Resources Tab: Stress, Trauma, Coin */}
        <Tabs.Content value="resources" className="p-4">
          <div className="grid grid-cols-3 gap-4">
            {(Object.keys(character.resources) as Array<keyof BladesResources>).map((resource) => (
              <div key={resource} className="flex flex-col items-center">
                <span className="font-semibold capitalize">{resource}</span>
                {isEditable ? (
                  <input
                    type="number"
                    min={0}
                    value={character.resources[resource]}
                    onChange={(e) => handleResourceChange(resource, Number(e.target.value))}
                    className="w-16 border rounded px-1 text-center"
                  />
                ) : (
                  <span>{character.resources[resource]}</span>
                )}
              </div>
            ))}
          </div>
        </Tabs.Content>
      </Tabs.Root>
    </div>
  );
};

export default CharacterSheet;
