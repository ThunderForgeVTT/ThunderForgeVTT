import { useState, type ReactNode } from "react";
import {
  Button,
  updateActorSystemData,
  useActorSystemData,
  useUpdateTraitData,
  type ActorSheetProps,
} from "@thunderforge/host";
import type {
  GenieAbilityData,
  GenieResourceData,
} from "./components/CharacterSheet";
import { GENIE_CONDITIONS } from "./conditions";
import { calculateMaxWishPoints } from "./derived-data.ts";
import {
  GENIE_ATTRIBUTES,
  GENIE_RESOURCES,
  GENIE_SHEET_REGIONS,
  type GenieSheetRegion,
} from "./sheet-regions";
import {
  cardClass,
  fieldClass,
  hintClass,
  sectionHeadingClass,
  textareaClass,
} from "./components/styles";

const DEFAULT_GENIE_ABILITIES: GenieAbilityData = {
  might: 0,
  cunning: 0,
  spirit: 0,
};

/**
 * Columns a region takes, by the span it declares.
 *
 * One column below `md` whatever a region asks for, so a 375px phone reads the
 * sheet top to bottom in the declared order. Two columns at `md`, three at
 * `lg`. Full literal strings: Tailwind only builds classes it finds verbatim.
 */
const SPAN_CLASS: Record<GenieSheetRegion["span"], string> = {
  1: "",
  2: "md:col-span-2",
  3: "md:col-span-2 lg:col-span-3",
};

/**
 * Genie's character sheet, laid out as a sheet is read at a table.
 *
 * # What changed, and why
 *
 * This was one `grid gap-4` card around a tabbed component: Abilities,
 * Skills, Conditions and Resources, one at a time, in a column about as wide
 * as a phone on any screen. The owner wanted this page to show how flexible
 * the configuration is. It now draws `GENIE_SHEET_REGIONS`: identity and
 * scores in the first row, resources and conditions in the second, traits and
 * notes in the third. One column at 375px, in the same order.
 *
 * The play dock still mounts the tabbed `CharacterSheet`, because a dock pane
 * is narrow and tabs suit it. This page no longer does.
 *
 * # What the pack decides, and what the host page decides
 *
 * The host's actor page draws the actor's imagery panel (portrait and token)
 * directly above this sheet on the edit route, for every system. It also
 * draws inventory and the actor's scrolls and knacks below it. So this sheet
 * has no picture and no ability list. Both would be second copies, and
 * `@thunderforge/host` gives a pack no way to read an actor's abilities or the
 * world's ability vocabulary anyway.
 *
 * # Who may edit
 *
 * `canEdit` comes from the host: the actor's own permission, and only on the
 * edit route. Without it every value is shown as text, and no input,
 * checkbox or Save is drawn. The server refuses a write from anyone else
 * regardless.
 *
 * # What could be declared rather than written here
 *
 * The host already has a declarative sheet: a system publishes values
 * (`attributes`, `resources`, `skills`, `movement`, `derived`) and the
 * world's interface pack lays them out with `section`, `row`, `column`,
 * `badgeGrid`, `barStack`, `rowList`, `value` and `block`
 * (`apps/web/src/sheet-layout/types.ts`). Genie's scores and pools are
 * already declared in `system.json` in exactly that vocabulary. What that
 * format cannot say, and this file has to:
 *
 *  1. A system cannot lay out its own sheet. The layout belongs to the
 *     *interface* pack (Forge), and Forge's layout is one stacked column of
 *     sections for every system.
 *  2. There is no width. `row` and `column` exist, but nothing says "this
 *     section takes two of three columns" or what happens at a breakpoint.
 *     `GenieSheetRegion.span` is that missing field.
 *  3. Conditions are not a set. `conditions` is declared in `system.json` but
 *     is not one of the sets a layout can address, so it cannot be placed.
 *  4. Notes are not placeable. `sheet` declares a `text` field in
 *     `traitData`, and a layout has no node for editable prose.
 *  5. Nothing is editable. A declared value is a rendered string. There is no
 *     input for a score, no stepper for a pool, no checkbox for a condition,
 *     and no way to say "writing level recomputes the Wish Point ceiling"
 *     (`wishPoints` is a declared table, but nothing applies it on write).
 *
 * With 1 to 4, this layout could be declared: a system-owned layout, a `span`
 * on sections, `conditions` and `notes` as addressable sets. 5 is what still
 * needs a pack's own React.
 */
export default function GenieActorSheet({ actor, canEdit }: ActorSheetProps) {
  const { data, refetch } = useActorSystemData(actor.id, "genie");
  const { updateTraits, isPending } = useUpdateTraitData(actor.id, "genie");

  const traitData = (data?.trait_data ?? {}) as Record<string, unknown>;
  const abilityData =
    (data?.ability_data as GenieAbilityData | undefined) ??
    DEFAULT_GENIE_ABILITIES;
  const activeConditions: string[] = Array.isArray(traitData.active_conditions)
    ? (traitData.active_conditions as string[])
    : [];
  const level = typeof traitData.level === "number" ? traitData.level : 1;
  const resourceData: GenieResourceData = (data?.resource_data as
    | GenieResourceData
    | undefined) ?? {
    current_wish_points: 0,
    max_wish_points: calculateMaxWishPoints(level),
    current_health: 1,
    max_health: 1,
  };

  const handleAbilityChange = async (
    ability: keyof GenieAbilityData,
    value: number,
  ) => {
    await updateActorSystemData(actor.id, "genie", "ability_data", {
      ...abilityData,
      [ability]: value,
    });
    await refetch();
  };

  const toggleCondition = async (key: string) => {
    const next = activeConditions.includes(key)
      ? activeConditions.filter((c) => c !== key)
      : [...activeConditions, key];
    await updateTraits({ ...traitData, active_conditions: next });
    await refetch();
  };

  const handleLevelChange = async (newLevel: number) => {
    await updateTraits({ ...traitData, level: newLevel });
    const newMaxWishPoints = calculateMaxWishPoints(newLevel);
    await updateActorSystemData(actor.id, "genie", "resource_data", {
      ...resourceData,
      max_wish_points: newMaxWishPoints,
      current_wish_points: Math.min(
        resourceData.current_wish_points,
        newMaxWishPoints,
      ),
    });
    await refetch();
  };

  const handleResourceChange = async (
    field: keyof GenieResourceData,
    value: number,
  ) => {
    await updateActorSystemData(actor.id, "genie", "resource_data", {
      ...resourceData,
      [field]: value,
    });
    await refetch();
  };

  const handleNotesSave = async (notes: string) => {
    await updateTraits({ ...traitData, notes });
    await refetch();
  };

  const regionBody = (region: GenieSheetRegion): ReactNode => {
    switch (region.kind) {
      case "identity":
        return (
          <dl className="grid gap-3">
            <Fact label="Name">{actor.label}</Fact>
            <Fact label="Kind">
              {actor.isNpc ? "Non-player character" : "Player character"}
            </Fact>
            {typeof traitData.size_category === "string" ? (
              <Fact label="Size">{traitData.size_category}</Fact>
            ) : null}
          </dl>
        );

      case "scores":
        return (
          <div className="grid grid-cols-3 gap-3">
            {GENIE_ATTRIBUTES.map((attribute) => {
              const id = `genie-score-${attribute.id}`;
              return (
                <div
                  key={attribute.id}
                  className="grid justify-items-center gap-1 rounded-lg border border-border bg-muted/30 p-3 text-center"
                  data-testid={id}
                >
                  <label
                    htmlFor={canEdit ? `${id}-input` : undefined}
                    className="text-sm font-semibold"
                  >
                    {attribute.label}
                  </label>
                  {canEdit ? (
                    <input
                      id={`${id}-input`}
                      type="number"
                      className={`${fieldClass} w-16 text-center text-lg font-semibold tabular-nums`}
                      value={abilityData[attribute.id]}
                      onChange={(event) =>
                        void handleAbilityChange(
                          attribute.id,
                          Number(event.target.value),
                        )
                      }
                    />
                  ) : (
                    <span className="text-3xl leading-none font-semibold tabular-nums">
                      {abilityData[attribute.id]}
                    </span>
                  )}
                  <abbr
                    title={attribute.label}
                    className={`${hintClass} tracking-widest no-underline`}
                  >
                    {attribute.abbreviation}
                  </abbr>
                </div>
              );
            })}
          </div>
        );

      case "pools":
        return (
          <div className="grid gap-4 sm:grid-cols-2">
            {GENIE_RESOURCES.map((pool) => {
              const current = resourceData[pool.current];
              const max = resourceData[pool.max];
              const id = `genie-pool-${pool.id}`;
              const inputTestId =
                pool.id === "health"
                  ? "genie-current-health-input"
                  : "genie-current-wish-points-input";
              return (
                <div key={pool.id} className="grid gap-2" data-testid={id}>
                  <div className="flex items-baseline justify-between gap-2">
                    <label
                      htmlFor={canEdit ? `${id}-input` : undefined}
                      className="text-sm font-semibold"
                    >
                      {pool.label}
                    </label>
                    <span className="flex items-baseline gap-1 tabular-nums">
                      {canEdit ? (
                        <input
                          id={`${id}-input`}
                          type="number"
                          min={0}
                          max={max}
                          className={`${fieldClass} w-16 text-center`}
                          data-testid={inputTestId}
                          value={current}
                          onChange={(event) =>
                            void handleResourceChange(
                              pool.current,
                              Number(event.target.value),
                            )
                          }
                        />
                      ) : (
                        <span className="text-lg font-semibold">{current}</span>
                      )}
                      <span className={hintClass}>/ {max} max</span>
                    </span>
                  </div>
                  <meter
                    min={0}
                    max={Math.max(max, 1)}
                    value={Math.min(current, max)}
                    aria-label={`${pool.label}: ${current} of ${max}`}
                    className="h-2 w-full"
                  />
                </div>
              );
            })}
          </div>
        );

      case "conditions":
        if (!canEdit) {
          const active = GENIE_CONDITIONS.filter((c) =>
            activeConditions.includes(c.key),
          );
          return active.length === 0 ? (
            <p className={hintClass} data-testid="genie-condition-track-sheet">
              None.
            </p>
          ) : (
            <ul className="grid gap-2" data-testid="genie-condition-track-sheet">
              {active.map((condition) => (
                <li key={condition.key} className="grid gap-0.5">
                  <span className="font-semibold">{condition.label}</span>
                  <span className={hintClass}>{condition.description}</span>
                </li>
              ))}
            </ul>
          );
        }
        // Every declared condition, so turning one on is where you read what
        // it does. The description is the checkbox's description, not part
        // of its name, so "Bound" is still the name a screen reader says.
        return (
          <ul
            className="grid gap-3"
            data-testid="genie-condition-editor"
            aria-busy={isPending}
          >
            {GENIE_CONDITIONS.map((condition) => {
              const id = `genie-condition-${condition.key}`;
              return (
                <li key={condition.key} className="flex items-start gap-2">
                  <input
                    id={id}
                    type="checkbox"
                    className="mt-1"
                    checked={activeConditions.includes(condition.key)}
                    disabled={isPending}
                    aria-describedby={`${id}-description`}
                    onChange={() => void toggleCondition(condition.key)}
                  />
                  <span className="grid gap-0.5">
                    <label htmlFor={id} className="text-sm font-semibold">
                      {condition.label}
                    </label>
                    <span id={`${id}-description`} className={hintClass}>
                      {condition.description}
                    </span>
                  </span>
                </li>
              );
            })}
          </ul>
        );

      case "traits":
        return (
          <dl className="grid gap-3">
            <div className="grid gap-1">
              <dt>
                <label
                  htmlFor={canEdit ? "genie-level" : undefined}
                  className={sectionHeadingClass}
                >
                  Level
                </label>
              </dt>
              <dd>
                {canEdit ? (
                  <input
                    id="genie-level"
                    type="number"
                    min={1}
                    max={10}
                    className={`${fieldClass} w-20`}
                    data-testid="genie-level-input"
                    value={level}
                    onChange={(event) =>
                      void handleLevelChange(Number(event.target.value))
                    }
                  />
                ) : (
                  <span className="text-lg font-semibold tabular-nums">
                    {level}
                  </span>
                )}
              </dd>
            </div>
            <Fact label="Wish Point ceiling">
              {calculateMaxWishPoints(level)} at level {level}
            </Fact>
          </dl>
        );

      case "notes":
        return (
          <NotesRegion
            key={String(traitData.notes ?? "")}
            notes={typeof traitData.notes === "string" ? traitData.notes : ""}
            canEdit={canEdit}
            isSaving={isPending}
            onSave={handleNotesSave}
          />
        );
    }
  };

  return (
    <div
      className="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-3"
      data-testid="genie-actor-sheet"
      data-editable={canEdit ? "true" : "false"}
    >
      {GENIE_SHEET_REGIONS.map((region) => (
        <section
          key={region.id}
          aria-labelledby={`genie-region-${region.id}`}
          className={`${cardClass} grid content-start gap-3 ${SPAN_CLASS[region.span]}`}
          data-testid={
            region.kind === "pools" ? "genie-resources-tab" : `genie-region-${region.id}`
          }
          data-region={region.id}
          data-span={region.span}
        >
          <header className="grid gap-0.5">
            <h2 id={`genie-region-${region.id}`} className={sectionHeadingClass}>
              {region.title}
            </h2>
            <p className={hintClass}>{region.blurb}</p>
          </header>
          {regionBody(region)}
        </section>
      ))}
    </div>
  );
}

function Fact({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="grid gap-0.5">
      <dt className={sectionHeadingClass}>{label}</dt>
      <dd className="font-medium break-words">{children}</dd>
    </div>
  );
}

/**
 * Notes, written through `trait_data.notes` — the one field Genie's `sheet`
 * block declares.
 *
 * A draft and a Save rather than a write per keystroke: a note is prose, and
 * a mutation on every letter would race itself. Keyed by the stored text in
 * the parent, so a save from elsewhere replaces an untouched draft.
 */
function NotesRegion({
  notes,
  canEdit,
  isSaving,
  onSave,
}: {
  notes: string;
  canEdit: boolean;
  isSaving: boolean;
  onSave: (notes: string) => Promise<void>;
}) {
  const [draft, setDraft] = useState(notes);

  if (!canEdit) {
    return notes ? (
      <p className="text-sm whitespace-pre-wrap">{notes}</p>
    ) : (
      <p className={hintClass}>No notes.</p>
    );
  }

  return (
    <div className="grid gap-2">
      <label htmlFor="genie-notes" className="sr-only">
        Notes
      </label>
      <textarea
        id="genie-notes"
        rows={4}
        className={textareaClass}
        value={draft}
        onChange={(event) => setDraft(event.target.value)}
        data-testid="genie-notes-input"
      />
      <Button
        size="sm"
        className="justify-self-start"
        disabled={isSaving || draft === notes}
        onClick={() => void onSave(draft)}
        data-testid="genie-notes-save"
      >
        {/* Not "Save notes": the host page already has a "Save" for the
            actor's own fields, and two buttons whose names both contain
            "Save" is one button too ambiguous for a person, a screen reader,
            or a `getByRole("button", { name: "Save" })`. */}
        Update notes
      </Button>
    </div>
  );
}
