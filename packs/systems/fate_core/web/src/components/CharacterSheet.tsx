import React, { useState } from 'react';
import * as Tabs from '@radix-ui/react-tabs';

/** The Ladder: Fate Core's universal skill-rating/opposition scale (Legendary +8 down to Terrible -2). */
export const LADDER: { key: string; label: string; abbreviation: string; value: number }[] = [
  { key: 'legendary', label: 'Legendary', abbreviation: '+8', value: 8 },
  { key: 'epic', label: 'Epic', abbreviation: '+7', value: 7 },
  { key: 'fantastic', label: 'Fantastic', abbreviation: '+6', value: 6 },
  { key: 'superb', label: 'Superb', abbreviation: '+5', value: 5 },
  { key: 'great', label: 'Great', abbreviation: '+4', value: 4 },
  { key: 'good', label: 'Good', abbreviation: '+3', value: 3 },
  { key: 'fair', label: 'Fair', abbreviation: '+2', value: 2 },
  { key: 'average', label: 'Average', abbreviation: '+1', value: 1 },
  { key: 'mediocre', label: 'Mediocre', abbreviation: '+0', value: 0 },
  { key: 'poor', label: 'Poor', abbreviation: '-1', value: -1 },
  { key: 'terrible', label: 'Terrible', abbreviation: '-2', value: -2 },
];

/** The 18 Fate Core skills, each tagged with the actions it supports. */
export const SKILLS: { key: string; label: string; actions: string[] }[] = [
  { key: 'athletics', label: 'Athletics', actions: ['Overcome', 'Create an Advantage', 'Defend'] },
  { key: 'burglary', label: 'Burglary', actions: ['Overcome', 'Create an Advantage'] },
  { key: 'contacts', label: 'Contacts', actions: ['Overcome', 'Create an Advantage', 'Defend'] },
  { key: 'crafts', label: 'Crafts', actions: ['Overcome', 'Create an Advantage'] },
  { key: 'deceive', label: 'Deceive', actions: ['Overcome', 'Create an Advantage', 'Defend'] },
  { key: 'drive', label: 'Drive', actions: ['Overcome', 'Create an Advantage', 'Defend'] },
  { key: 'empathy', label: 'Empathy', actions: ['Overcome', 'Create an Advantage', 'Defend'] },
  { key: 'fight', label: 'Fight', actions: ['Overcome', 'Create an Advantage', 'Attack', 'Defend'] },
  { key: 'investigate', label: 'Investigate', actions: ['Overcome', 'Create an Advantage'] },
  { key: 'lore', label: 'Lore', actions: ['Overcome', 'Create an Advantage'] },
  { key: 'notice', label: 'Notice', actions: ['Overcome', 'Create an Advantage', 'Defend'] },
  { key: 'physique', label: 'Physique', actions: ['Overcome', 'Create an Advantage', 'Defend'] },
  { key: 'provoke', label: 'Provoke', actions: ['Overcome', 'Create an Advantage', 'Attack'] },
  { key: 'rapport', label: 'Rapport', actions: ['Overcome', 'Create an Advantage', 'Defend'] },
  { key: 'resources', label: 'Resources', actions: ['Overcome', 'Create an Advantage'] },
  { key: 'shoot', label: 'Shoot', actions: ['Overcome', 'Create an Advantage', 'Attack'] },
  { key: 'stealth', label: 'Stealth', actions: ['Overcome', 'Create an Advantage', 'Defend'] },
  { key: 'will', label: 'Will', actions: ['Overcome', 'Create an Advantage', 'Defend'] },
];

const LADDER_BY_VALUE: Record<number, { label: string; abbreviation: string }> = Object.fromEntries(
  LADDER.map((rung) => [rung.value, { label: rung.label, abbreviation: rung.abbreviation }])
);

function ladderLabel(rating: number): string {
  const rung = LADDER_BY_VALUE[rating];
  return rung ? `${rung.label} (${rung.abbreviation})` : `${rating >= 0 ? '+' : ''}${rating}`;
}

export interface FateStressTrack {
  /** Box values, e.g. [1, 2] for the default two-box track. */
  boxes: number[];
  /** Which boxes are currently marked (checked), by index into `boxes`. */
  marked: boolean[];
}

export interface FateConsequence {
  severity: 'mild' | 'moderate' | 'severe' | 'extreme';
  /** The shift value this slot absorbs (2/4/6/8). */
  absorbs: number;
  /** The negative aspect text once the slot has been used, or empty if open. */
  aspect: string;
}

export interface FateCharacter {
  id: string;
  name: string;
  /** High Concept, Trouble, and any further aspects from the Phase Trio/play. */
  aspects: string[];
  /** Skill key -> Ladder rating (e.g. { fight: 3 } for Good (+3) Fight). */
  skills: Record<string, number>;
  stress: {
    physical: FateStressTrack;
    mental: FateStressTrack;
  };
  consequences: FateConsequence[];
  fatePoints: number;
  refresh: number;
}

interface CharacterSheetProps {
  character?: FateCharacter;
  isEditable?: boolean;
  onUpdate?: (character: Partial<FateCharacter>) => void;
}

/**
 * Fate Core Character Sheet Component
 *
 * Mirrors packs/systems/dnd5e's CharacterSheet.tsx conventions (Radix tabs,
 * props-driven, no internal data fetching) but adapts the tab structure to
 * Fate's actual character sheet shape:
 * - Aspects: High Concept, Trouble, and further aspects (free-text list)
 * - Skills: the 18-skill list with each skill's Ladder rating and actions
 * - Stress & Consequences: Physical/Mental stress tracks, consequence
 *   slots, plus Refresh and Fate Points (Fate's resource economy)
 */
const CharacterSheet: React.FC<CharacterSheetProps> = ({ character, isEditable = true, onUpdate }) => {
  const [selectedTab, setSelectedTab] = useState<string>('aspects');

  if (!character) {
    return (
      <div className="p-4 text-center text-gray-500">
        No character selected. Create or load a character to begin.
      </div>
    );
  }

  const handleAspectChange = (index: number, value: string) => {
    if (!isEditable || !onUpdate) return;
    const aspects = [...character.aspects];
    aspects[index] = value;
    onUpdate({ aspects });
  };

  const handleSkillChange = (skillKey: string, rating: number) => {
    if (!isEditable || !onUpdate) return;
    onUpdate({ skills: { ...character.skills, [skillKey]: rating } });
  };

  const handleStressToggle = (track: 'physical' | 'mental', index: number) => {
    if (!isEditable || !onUpdate) return;
    const current = character.stress[track];
    const marked = [...current.marked];
    marked[index] = !marked[index];
    onUpdate({
      stress: {
        ...character.stress,
        [track]: { ...current, marked },
      },
    });
  };

  const handleConsequenceChange = (index: number, aspect: string) => {
    if (!isEditable || !onUpdate) return;
    const consequences = character.consequences.map((c, i) => (i === index ? { ...c, aspect } : c));
    onUpdate({ consequences });
  };

  const handleFatePointsChange = (value: number) => {
    if (!isEditable || !onUpdate) return;
    onUpdate({ fatePoints: Math.max(0, value) });
  };

  return (
    <div className="w-full max-w-4xl mx-auto p-4 bg-white rounded-lg shadow-lg">
      {/* Header */}
      <div className="mb-6 border-b pb-4">
        <h1 className="text-3xl font-bold">{character.name}</h1>
        <div className="mt-2 flex gap-4 text-sm">
          <div>
            <span className="font-semibold">Fate Points:</span> {character.fatePoints}
          </div>
          <div>
            <span className="font-semibold">Refresh:</span> {character.refresh}
          </div>
        </div>
      </div>

      {/* Tabbed Interface */}
      <Tabs.Root value={selectedTab} onValueChange={setSelectedTab} className="w-full">
        <Tabs.List className="flex gap-2 border-b mb-4">
          <Tabs.Trigger
            value="aspects"
            className="px-4 py-2 font-semibold border-b-2 border-transparent data-[state=active]:border-blue-500 hover:text-blue-600"
          >
            Aspects
          </Tabs.Trigger>
          <Tabs.Trigger
            value="skills"
            className="px-4 py-2 font-semibold border-b-2 border-transparent data-[state=active]:border-blue-500 hover:text-blue-600"
          >
            Skills
          </Tabs.Trigger>
          <Tabs.Trigger
            value="stress"
            className="px-4 py-2 font-semibold border-b-2 border-transparent data-[state=active]:border-blue-500 hover:text-blue-600"
          >
            Stress &amp; Consequences
          </Tabs.Trigger>
        </Tabs.List>

        {/* Aspects Tab */}
        <Tabs.Content value="aspects" className="p-4">
          <h2 className="text-2xl font-bold mb-4">Aspects</h2>
          <div className="space-y-2">
            {character.aspects.map((aspect, index) => (
              <div key={index} className="flex items-center gap-3">
                <span className="w-32 shrink-0 text-sm text-gray-500">
                  {index === 0 ? 'High Concept' : index === 1 ? 'Trouble' : `Aspect ${index + 1}`}
                </span>
                <input
                  type="text"
                  value={aspect}
                  disabled={!isEditable}
                  onChange={(e) => handleAspectChange(index, e.target.value)}
                  className="flex-1 border rounded px-3 py-2 disabled:bg-gray-50"
                />
              </div>
            ))}
            {character.aspects.length === 0 && (
              <p className="text-gray-500">No aspects recorded yet.</p>
            )}
          </div>
        </Tabs.Content>

        {/* Skills Tab */}
        <Tabs.Content value="skills" className="p-4">
          <h2 className="text-2xl font-bold mb-4">Skills</h2>
          <div className="space-y-2">
            {SKILLS.map((skill) => {
              const rating = character.skills[skill.key] ?? 0;
              return (
                <div
                  key={skill.key}
                  className="flex items-center gap-4 p-3 rounded border border-gray-200 hover:border-gray-300"
                >
                  <div className="flex-1">
                    <div className="font-semibold text-gray-900">{skill.label}</div>
                    <div className="text-xs text-gray-500">{skill.actions.join(' / ')}</div>
                  </div>
                  <div className="text-right w-40">
                    {isEditable ? (
                      <select
                        value={rating}
                        onChange={(e) => handleSkillChange(skill.key, Number(e.target.value))}
                        className="border rounded px-2 py-1"
                      >
                        {LADDER.slice()
                          .reverse()
                          .map((rung) => (
                            <option key={rung.key} value={rung.value}>
                              {rung.label} ({rung.abbreviation})
                            </option>
                          ))}
                      </select>
                    ) : (
                      <span className="text-lg font-bold">{ladderLabel(rating)}</span>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        </Tabs.Content>

        {/* Stress & Consequences Tab */}
        <Tabs.Content value="stress" className="p-4">
          <h2 className="text-2xl font-bold mb-4">Stress &amp; Consequences</h2>

          <div className="grid grid-cols-1 sm:grid-cols-2 gap-6 mb-6">
            {(['physical', 'mental'] as const).map((track) => (
              <div key={track}>
                <h3 className="font-semibold capitalize mb-2">{track} Stress</h3>
                <div className="flex gap-2">
                  {character.stress[track].boxes.map((boxValue, index) => (
                    <button
                      key={index}
                      type="button"
                      disabled={!isEditable}
                      onClick={() => handleStressToggle(track, index)}
                      className={`w-10 h-10 border rounded flex items-center justify-center font-bold ${
                        character.stress[track].marked[index]
                          ? 'bg-red-100 border-red-400 text-red-700'
                          : 'bg-white border-gray-300 text-gray-700'
                      }`}
                    >
                      {boxValue}
                    </button>
                  ))}
                </div>
              </div>
            ))}
          </div>

          <h3 className="font-semibold mb-2">Consequences</h3>
          <div className="space-y-2">
            {character.consequences.map((consequence, index) => (
              <div key={index} className="flex items-center gap-3">
                <span className="w-40 shrink-0 text-sm text-gray-500 capitalize">
                  {consequence.severity} ({consequence.absorbs} shifts)
                </span>
                <input
                  type="text"
                  value={consequence.aspect}
                  placeholder="Open"
                  disabled={!isEditable}
                  onChange={(e) => handleConsequenceChange(index, e.target.value)}
                  className="flex-1 border rounded px-3 py-2 disabled:bg-gray-50"
                />
              </div>
            ))}
          </div>

          <div className="mt-6 flex items-center gap-4">
            <label className="font-semibold" htmlFor="fate-points-input">
              Fate Points
            </label>
            <input
              id="fate-points-input"
              type="number"
              min={0}
              value={character.fatePoints}
              disabled={!isEditable}
              onChange={(e) => handleFatePointsChange(Number(e.target.value))}
              className="w-20 border rounded px-2 py-1 disabled:bg-gray-50"
            />
            <span className="text-sm text-gray-500">Refresh: {character.refresh}</span>
          </div>
        </Tabs.Content>
      </Tabs.Root>
    </div>
  );
};

export default CharacterSheet;
