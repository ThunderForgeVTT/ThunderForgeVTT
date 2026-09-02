import { useCallback, useEffect, useMemo, useState } from "react";
import { Button } from "@/components/ui/button/Button";
import { Panel } from "@/components/ui/panel/Panel";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  getEffectRegistry,
  type ConfigField,
  type EffectDeclaration,
  type Interactive,
  type SubjectKind,
} from "@/api/interactives";
import { EffectHelperRow } from "./EffectHelperRow";
import { helpersFor, missingRequiredFields } from "./effectHelpers";

/**
 * The Game Master's authoring panel for interactive elements.
 *
 * Spec 030. Game-Master-only: the caller is responsible for not mounting it
 * for a player, the same contract `WallTool` has. That is a presentation
 * decision, not the security boundary — every mutation this panel calls is
 * refused server-side for anyone who does not run the world (Principle III).
 *
 * # Why the form is built from the registry
 *
 * Every effect this build can perform is contributed by the subsystem that
 * performs it, and `effectRegistry` is the union of those. Rendering from it
 * rather than from a list written here is what makes FR-038 true: a Game
 * Master is offered exactly what exists.
 *
 * The alternative — a hard-coded list — fails in the direction that is worst
 * at a table. It offers an option nothing handles, the GM configures it, and
 * at the session nothing happens: no error, no warning, nothing to attach a
 * debugger to. That is the same silent-drift failure spec 029 spent a whole
 * user story retiring at the engine boundary, and there is no reason to
 * reintroduce it one layer up.
 *
 * It also means an unbuilt subsystem needs no handling at all. There is no
 * audio yet, so nothing declares a sound effect and none is offered — no
 * greying out, no "coming soon", no dead option to maintain.
 */

/** What sort of reference a picker is for, so the caller can supply choices. */
export interface ReferenceChoice {
  id: string;
  label: string;
}

export interface InteractionAuthorProps {
  /** The subject being authored against. */
  subjectKind: SubjectKind;
  /** The token or wall, for a prop or a door. */
  subjectRef?: string | null;
  /** The interactive already attached, if there is one. */
  existing?: Interactive | null;
  /**
   * What each sort of reference may point at, keyed by the declaration's
   * `referenceOf` — `wall`, `light`, `loreEntry`, `scene`.
   *
   * Supplied by the caller rather than fetched here, because what is
   * referenceable is world state this component has no business owning.
   */
  references?: Record<string, ReferenceChoice[]>;
  onSave: (draft: {
    effectId: string | null;
    effectConfig: Record<string, unknown> | null;
    activation: string;
    fireMode: string;
  }) => void;
  onDelete?: () => void;
  /**
   * What the save control says.
   *
   * The same panel authors an interactive onto something already on the map
   * and authors one that is about to be carried there (FR-011), and those are
   * different sentences to the person clicking: "Save" describes a change to a
   * thing they selected, and describes nothing at all when the next thing to
   * happen is a token following their cursor. A second panel for placement was
   * the alternative and would duplicate the registry-driven form this exists
   * to be.
   */
  saveLabel?: string;
}

const ACTIVATION_OPTIONS = [
  { value: "anyone", label: "Anyone at the table" },
  { value: "gm_only", label: "Only me" },
  { value: "requires_approval", label: "Ask me first" },
];

const FIRE_MODE_OPTIONS = [
  { value: "always", label: "Every time" },
  { value: "once", label: "Once, until I reset it" },
];

/** Scenery: an interactive that carries no effect, which is legitimate. */
const NO_EFFECT = "__none__";

export function InteractionAuthor({
  subjectKind,
  existing = null,
  references = {},
  onSave,
  onDelete,
  saveLabel = "Save",
}: InteractionAuthorProps) {
  const [registry, setRegistry] = useState<EffectDeclaration[] | null>(null);
  const [effectId, setEffectId] = useState<string>(
    existing?.effectId ?? NO_EFFECT,
  );
  const [config, setConfig] = useState<Record<string, unknown>>(
    existing?.effectConfig ?? {},
  );
  const [activation, setActivation] = useState(
    existing?.activation ?? "anyone",
  );
  const [fireMode, setFireMode] = useState(existing?.fireMode ?? "always");

  useEffect(() => {
    let cancelled = false;
    getEffectRegistry()
      .then((declarations) => {
        if (!cancelled) setRegistry(declarations);
      })
      .catch(() => {
        // An empty registry and a failed read are different situations and
        // must not look the same: the first is a build with no contributors,
        // which is legitimate, and the second is something to fix.
        if (!cancelled) setRegistry(null);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  /** Only what attaches to this sort of subject. */
  const available = useMemo(
    () => (registry ?? []).filter((d) => d.subjectKinds.includes(subjectKind)),
    [registry, subjectKind],
  );

  const selected = useMemo(
    () => available.find((d) => d.id === effectId) ?? null,
    [available, effectId],
  );

  /**
   * The same set as `available`, shaped for the helper row (FR-028).
   *
   * Derived from the registry rather than passed in, so the row and the
   * dropdown cannot come to offer different things — one of them being
   * right and the other stale is exactly the drift ADR-054 is about.
   */
  const helpers = useMemo(
    () => helpersFor(registry ?? [], subjectKind),
    [registry, subjectKind],
  );

  /**
   * What the chosen effect still needs before it is worth saving.
   *
   * Read from the declaration; the server refuses the same thing and remains
   * the authority (Principle III). This exists because its refusal arrives as
   * "that could not be saved", which names neither the field nor the reason,
   * and the Game Master most likely to hit it is the one placing their first
   * lore marker with the entry picker still untouched.
   */
  const missing = useMemo(
    () => missingRequiredFields(selected, config),
    [selected, config],
  );

  /**
   * Whether the effect this interactive was authored against still exists.
   *
   * Shown as a state rather than repaired (FR-041). Silently clearing it would
   * destroy a Game Master's work because their build happens to lack a
   * subsystem today.
   */
  const unavailable =
    existing?.effectId != null && existing.available === false;

  const setField = useCallback((key: string, value: unknown) => {
    setConfig((previous) => ({ ...previous, [key]: value }));
  }, []);

  const save = useCallback(() => {
    const chosen = effectId === NO_EFFECT ? null : effectId;
    onSave({
      effectId: chosen,
      effectConfig: chosen ? config : null,
      activation,
      fireMode,
    });
  }, [effectId, config, activation, fireMode, onSave]);

  if (registry === null) {
    return (
      <Panel>
        <p>Could not read what this build can do.</p>
      </Panel>
    );
  }

  return (
    <Panel>
      <h3>Interaction</h3>

      {unavailable && (
        <p role="status">
          This uses <code>{existing?.effectId}</code>, which this build cannot
          perform. It has been left exactly as you set it.
        </p>
      )}

      <EffectHelperRow
        helpers={helpers}
        selectedId={effectId === NO_EFFECT ? null : effectId}
        onChoose={(chosen) => setEffectId(chosen ?? NO_EFFECT)}
      />

      <Label htmlFor="interaction-effect">What happens</Label>
      <Select value={effectId} onValueChange={setEffectId}>
        <SelectTrigger id="interaction-effect">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {/* Scenery first, because a table with nothing attached is the
              commonest thing a Game Master places. */}
          <SelectItem value={NO_EFFECT}>
            Nothing — it is just scenery
          </SelectItem>
          {available.map((declaration) => (
            <SelectItem key={declaration.id} value={declaration.id}>
              {declaration.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>

      {available.length === 0 && (
        <p>Nothing in this build can be attached to this yet.</p>
      )}

      {selected && <p>{selected.description}</p>}

      {selected?.config.map((field) => (
        <ConfigInput
          key={field.key}
          field={field}
          value={config[field.key]}
          choices={
            field.referenceOf ? (references[field.referenceOf] ?? []) : []
          }
          onChange={(value) => setField(field.key, value)}
        />
      ))}

      <Label htmlFor="interaction-activation">Who may set it off</Label>
      <Select value={activation} onValueChange={setActivation}>
        <SelectTrigger id="interaction-activation">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {ACTIVATION_OPTIONS.map((option) => (
            <SelectItem key={option.value} value={option.value}>
              {option.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>

      <Label htmlFor="interaction-fire-mode">How often</Label>
      <Select value={fireMode} onValueChange={setFireMode}>
        <SelectTrigger id="interaction-fire-mode">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {FIRE_MODE_OPTIONS.map((option) => (
            <SelectItem key={option.value} value={option.value}>
              {option.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>

      {missing.length > 0 && (
        <p role="status" data-testid="interaction-incomplete">
          Choose {missing.map((field) => field.label.toLowerCase()).join(", ")}{" "}
          first.
        </p>
      )}

      <Button
        onClick={save}
        disabled={missing.length > 0}
        data-testid="interaction-save"
      >
        {saveLabel}
      </Button>
      {onDelete && (
        <Button variant="ghost" onClick={onDelete}>
          Remove
        </Button>
      )}
    </Panel>
  );
}

/**
 * One configuration field, rendered from what its effect declared.
 *
 * Note what has no case here: free text. The vocabulary has no such field
 * kind, so a link effect *cannot* be pointed at an arbitrary address — not
 * because this form refuses to show a box but because nothing ever asks for
 * one. That is what retires the hostile-destination problem without an
 * allowlist or a warning interstitial.
 */
function ConfigInput({
  field,
  value,
  choices,
  onChange,
}: {
  field: ConfigField;
  value: unknown;
  choices: ReferenceChoice[];
  onChange: (value: unknown) => void;
}) {
  const id = `interaction-config-${field.key}`;

  switch (field.kind) {
    case "boolean":
      return (
        <>
          <Label htmlFor={id}>{field.label}</Label>
          <input
            id={id}
            type="checkbox"
            checked={value === true}
            onChange={(event) => onChange(event.target.checked)}
          />
        </>
      );

    case "choice":
      return (
        <>
          <Label htmlFor={id}>{field.label}</Label>
          <Select
            value={typeof value === "string" ? value : ""}
            onValueChange={onChange}
          >
            <SelectTrigger id={id}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {(field.options ?? []).map((option) => (
                <SelectItem key={option.value} value={option.value}>
                  {option.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </>
      );

    case "reference":
      return (
        <>
          <Label htmlFor={id}>{field.label}</Label>
          <Select
            value={typeof value === "string" ? value : ""}
            onValueChange={onChange}
          >
            <SelectTrigger id={id}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {choices.map((choice) => (
                <SelectItem key={choice.id} value={choice.id}>
                  {choice.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </>
      );

    case "referenceList": {
      const chosen = Array.isArray(value) ? (value as string[]) : [];
      return (
        <fieldset>
          <legend>{field.label}</legend>
          {choices.map((choice) => (
            <label key={choice.id} htmlFor={`${id}-${choice.id}`}>
              <input
                id={`${id}-${choice.id}`}
                type="checkbox"
                checked={chosen.includes(choice.id)}
                onChange={(event) =>
                  onChange(
                    event.target.checked
                      ? [...chosen, choice.id]
                      : chosen.filter((c) => c !== choice.id),
                  )
                }
              />
              {choice.label}
            </label>
          ))}
        </fieldset>
      );
    }

    default:
      // A field kind this build does not render. Shown as such rather than
      // skipped: a silently missing input is a Game Master saving an
      // interactive they believe they configured.
      return (
        <p role="status">
          This build cannot configure &ldquo;{field.label}&rdquo;.
        </p>
      );
  }
}
