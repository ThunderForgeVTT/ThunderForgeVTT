/**
 * `world-settings` — the optional rules Roll for Shoes leaves to the table.
 *
 * `world-settings` is an existing panel slot. Filling it needs no
 * registration: `apps/web/src/panels/systemPanels.ts` finds this file with a
 * build-time `import.meta.glob`, and `WorldSystemSettingsPage.tsx` mounts
 * whatever `resolvePanel(world.gameSystemId, "world-settings")` returns.
 *
 * Unlike Genie's panel in the same slot, this one does **not** read its
 * settings off the `WorldRecord` the host hands it — they are not on `worlds`
 * and will not be. They live in a table this pack owns (ADR-063), so the panel
 * fetches them itself. `onWorldChanged` is still called after a write, because
 * the page holding this panel may show other things that moved.
 *
 * The game itself is public domain (CC0 1.0), by Ben Wray — rollforshoes.com.
 */
import { useCallback, useEffect, useState, type ReactNode } from "react";
import {
  Button,
  Card,
  Input,
  type WorldSettingsPanelProps,
} from "@thunderforge/host";

import {
  BAND_DICE,
  BAND_LABEL,
  BAND_TARGET,
  BANDS,
  DEFAULT_SETTINGS,
  STARTING_SKILL,
  type DifficultyMode,
  type WorldSettings,
} from "../game";
import { fetchWorldSettings, saveWorldSettings } from "../settings";

const DIFFICULTY_MODES: { value: DifficultyMode; label: string; help: string }[] =
  [
    {
      value: "free",
      label: "A number the GM names",
      help: "The core game. The Game Master says a number and the roll has to beat it.",
    },
    {
      value: "rolled",
      label: "Dice the GM rolls",
      help: `The Game Master picks how hard it is and rolls that many d6 — ${BANDS.map(
        (band) => `${BAND_LABEL[band]} ${BAND_DICE[band]}`,
      ).join(", ")}.`,
    },
    {
      value: "target",
      label: "A fixed number per difficulty",
      help: `The Game Master picks how hard it is and the number is fixed — ${BANDS.map(
        (band) => `${BAND_LABEL[band]} ${BAND_TARGET[band]}`,
      ).join(", ")}.`,
    },
  ];

/** One of the five settings, drawn the same way each time. */
function Setting({
  testId,
  title,
  description,
  children,
}: {
  testId: string;
  title: string;
  description: string;
  children: ReactNode;
}) {
  return (
    <div className="grid gap-2 border-t pt-4 first:border-t-0 first:pt-0" data-testid={testId}>
      <h3 className="text-sm font-semibold">{title}</h3>
      <p className="text-sm text-muted-foreground">{description}</p>
      {children}
    </div>
  );
}

export default function RollForShoesWorldSettingsPanel({
  worldId,
  isGm,
  onWorldChanged,
}: WorldSettingsPanelProps) {
  const [draft, setDraft] = useState<WorldSettings>(DEFAULT_SETTINGS);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [savedAt, setSavedAt] = useState<number | null>(null);

  useEffect(() => {
    let cancelled = false;
    setIsLoading(true);
    fetchWorldSettings(worldId)
      .then((settings) => {
        if (!cancelled) {
          setDraft(settings);
          setError(null);
        }
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          // The defaults are already in state, so a failed read shows the core
          // game rather than an empty panel — which is also what a world with
          // no stored row genuinely is.
          setError(
            err instanceof Error
              ? err.message
              : "Could not read this world's Roll for Shoes settings.",
          );
        }
      })
      .finally(() => {
        if (!cancelled) {
          setIsLoading(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [worldId]);

  const handleSave = useCallback(async () => {
    setIsSaving(true);
    setError(null);
    try {
      const stored = await saveWorldSettings(worldId, draft);
      setDraft(stored);
      setSavedAt(Date.now());
      onWorldChanged();
    } catch (err) {
      setError(
        err instanceof Error
          ? err.message
          : "Could not save this world's Roll for Shoes settings.",
      );
    } finally {
      setIsSaving(false);
    }
  }, [draft, onWorldChanged, worldId]);

  // Read-only rather than hidden: a player who can see the rules their table
  // plays under is better served than one who cannot, and the server refuses
  // the write regardless of what this renders.
  const locked = !isGm || isSaving || isLoading;

  return (
    <Card className="grid gap-4 p-6" data-testid="rfs-settings">
      <div className="grid gap-1">
        <h2 className="text-lg font-semibold">Optional rules</h2>
        <p className="text-sm text-muted-foreground">
          Roll for Shoes leaves these to the table. Everything here is off
          unless you turn it on, and each one is independent — a world that
          changes none of them plays the six core rules exactly as written.
          {isGm ? "" : " Only the Game Master can change these."}
        </p>
      </div>

      <Setting
        testId="rfs-setting-difficulty"
        title="How the opposition is set"
        description="What the roll has to beat, and how the Game Master arrives at that number."
      >
        <div className="grid gap-2">
          {DIFFICULTY_MODES.map((mode) => (
            <label key={mode.value} className="flex items-start gap-2 text-sm">
              <input
                type="radio"
                name="rfs-difficulty-mode"
                className="mt-1"
                value={mode.value}
                checked={draft.difficultyMode === mode.value}
                disabled={locked}
                onChange={() =>
                  setDraft((current) => ({
                    ...current,
                    difficultyMode: mode.value,
                  }))
                }
                data-testid={`rfs-setting-difficulty-${mode.value}`}
              />
              <span>
                <span className="font-medium">{mode.label}</span>
                <span className="block text-muted-foreground">{mode.help}</span>
              </span>
            </label>
          ))}
          <p className="text-sm text-muted-foreground">
            A Game Master who wants to name a number can always name one,
            whichever of these is chosen.
          </p>
        </div>
      </Setting>

      <Setting
        testId="rfs-setting-tie"
        title="A tie counts as a success"
        description="Core rule 2 says the roll has to be higher, so a tie is a failure and earns its experience. Turn this on and a tie succeeds instead — and, being a success, pays no experience."
      >
        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={draft.tieSucceeds}
            disabled={locked}
            onChange={(event) =>
              setDraft((current) => ({
                ...current,
                tieSucceeds: event.target.checked,
              }))
            }
            data-testid="rfs-setting-tie-toggle"
          />
          Matching the opposition is good enough
        </label>
      </Setting>

      <Setting
        testId="rfs-setting-statuses"
        title="Statuses"
        description="Named conditions a character carries, each worth a plus or minus applied to the roll's total. They sum, and they never change whether every die showed a six — a status can neither create nor destroy an advancement."
      >
        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={draft.statusesEnabled}
            disabled={locked}
            onChange={(event) =>
              setDraft((current) => ({
                ...current,
                statusesEnabled: event.target.checked,
              }))
            }
            data-testid="rfs-setting-statuses-toggle"
          />
          Let characters carry statuses
        </label>
      </Setting>

      <Setting
        testId="rfs-setting-slots"
        title="Skill slots"
        description="A limit on how many skills a character may hold at each level — four at level 2, three at level 3, two at level 4. Levels 1 and 5 and up are unlimited. More room can be bought with experience at twice the level."
      >
        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={draft.skillSlotsEnabled}
            disabled={locked}
            onChange={(event) =>
              setDraft((current) => ({
                ...current,
                skillSlotsEnabled: event.target.checked,
              }))
            }
            data-testid="rfs-setting-slots-toggle"
          />
          Limit how many skills sit at each level
        </label>
      </Setting>

      <Setting
        testId="rfs-setting-starting-skills"
        title="Starting skills"
        description={`What a brand-new character is given. Leave this empty for the core default, ${STARTING_SKILL.name} ${STARTING_SKILL.level}. Changing it never alters a character who already exists.`}
      >
        <div className="grid gap-2">
          {draft.startingSkills.length === 0 ? (
            <p className="text-sm text-muted-foreground" data-testid="rfs-starting-skills-default">
              New characters start with {STARTING_SKILL.name}{" "}
              {STARTING_SKILL.level}.
            </p>
          ) : null}
          {draft.startingSkills.map((skill, index) => (
            // The index is the identity here: these rows have no ids, they are
            // a short ordered list the Game Master is editing in place, and a
            // generated id would be stored for nothing.
            <div key={index} className="flex items-center gap-2">
              <Input
                aria-label="Starting skill name"
                value={skill.name}
                disabled={locked}
                onChange={(event) =>
                  setDraft((current) => ({
                    ...current,
                    startingSkills: current.startingSkills.map((row, i) =>
                      i === index ? { ...row, name: event.target.value } : row,
                    ),
                  }))
                }
                data-testid={`rfs-starting-skill-name-${index}`}
              />
              <Input
                aria-label="Starting skill level"
                type="number"
                min={1}
                className="w-20"
                value={String(skill.level)}
                disabled={locked}
                onChange={(event) =>
                  setDraft((current) => ({
                    ...current,
                    startingSkills: current.startingSkills.map((row, i) =>
                      i === index
                        ? {
                            ...row,
                            level: Math.max(
                              1,
                              Math.trunc(Number(event.target.value) || 1),
                            ),
                          }
                        : row,
                    ),
                  }))
                }
                data-testid={`rfs-starting-skill-level-${index}`}
              />
              <Button
                type="button"
                variant="ghost"
                disabled={locked}
                onClick={() =>
                  setDraft((current) => ({
                    ...current,
                    startingSkills: current.startingSkills.filter(
                      (_, i) => i !== index,
                    ),
                  }))
                }
                data-testid={`rfs-starting-skill-remove-${index}`}
              >
                Remove
              </Button>
            </div>
          ))}
          <div>
            <Button
              type="button"
              variant="secondary"
              disabled={locked}
              onClick={() =>
                setDraft((current) => ({
                  ...current,
                  startingSkills: [
                    ...current.startingSkills,
                    { name: STARTING_SKILL.name, level: STARTING_SKILL.level },
                  ],
                }))
              }
              data-testid="rfs-starting-skill-add"
            >
              Add a starting skill
            </Button>
          </div>
        </div>
      </Setting>

      {isGm ? (
        <div className="flex items-center gap-3">
          <Button
            type="button"
            disabled={locked}
            onClick={() => void handleSave()}
            data-testid="rfs-settings-save"
          >
            {isSaving ? "Saving…" : "Save optional rules"}
          </Button>
          {savedAt !== null && !isSaving && error === null ? (
            <span className="text-sm text-muted-foreground" data-testid="rfs-settings-saved">
              Saved.
            </span>
          ) : null}
        </div>
      ) : null}

      {error !== null ? (
        <p className="text-sm text-destructive" data-testid="rfs-settings-error">
          {error}
        </p>
      ) : null}
    </Card>
  );
}
