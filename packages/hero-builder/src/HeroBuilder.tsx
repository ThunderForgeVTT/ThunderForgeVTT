/**
 * The builder: a spec on one side, the hero it draws on the other.
 *
 * State is a `HeroSpec`, always the smallest one that draws the hero, so a
 * colour put back to its default follows again rather than staying pinned.
 * Every change passes `validateHero` before it is accepted; a refused change
 * is reported and leaves the hero as it was (FR-004, FR-010, B3).
 */
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import {
  HERO_RACES,
  minimalSpec,
  randomHero,
  resolveHero,
  validateHero,
  type HeroSpec,
  type RaceKey,
  type ResolvedHero,
} from "@thunderforge/heroes";
import { ChoiceGroup } from "./controls/ChoiceGroup.tsx";
import { ColorControl } from "./controls/ColorControl.tsx";
import { FlagControl } from "./controls/FlagControl.tsx";
import { TextControl } from "./controls/TextControl.tsx";
import {
  CHOICE_FIELDS,
  COLOR_FIELDS,
  FLAG_FIELDS,
  fieldLabel,
  setFields,
  type HeroField,
} from "./controls/catalogue.ts";
import { HeroPreview } from "./preview/HeroPreview.tsx";
import { toProblems, type HeroProblem } from "./problems.ts";
import { freshSeed } from "./roll/freshSeed.ts";
import { RollBar } from "./roll/RollBar.tsx";
import { BUILDER_CSS } from "./styles.ts";

export interface HeroBuilderProps {
  /** The spec the builder opens on. Run through validateHero before drawing;
   *  a failure is reported through `onInvalid`, not thrown. */
  initialSpec: HeroSpec;
  /** Unique within the host document. Every SVG and radio group the builder
   *  mounts derives its id or name from this (FR-011). */
  idPrefix: string;
  /** Called on every accepted change, with the minimal spec. */
  onChange?(spec: HeroSpec): void;
  /** What the host offers at the bottom of the builder. The library performs
   *  no action of its own. */
  actions?: ReactNode;
  /** A spec that would not validate. */
  onInvalid?(problems: readonly HeroProblem[]): void;
  /** Where the race picker starts; null, absent or unknown starts on "any"
   *  (FR-007a). The builder reports no race back. */
  initialRace?: RaceKey | null;
}

type Locks = Partial<Record<HeroField, true>>;

const MAX_NAME = 80;
const MAX_TITLE = 120;

function knownRace(race: RaceKey | null | undefined): RaceKey | null {
  return race != null && Object.hasOwn(HERO_RACES, race) ? race : null;
}

export function HeroBuilder({
  initialSpec,
  idPrefix,
  onChange,
  actions,
  onInvalid,
  initialRace,
}: HeroBuilderProps) {
  const opening = useMemo(() => validateHero(initialSpec), [initialSpec]);
  const [spec, setSpec] = useState<HeroSpec | null>(() =>
    opening.ok ? minimalSpec(initialSpec) : null,
  );
  const [problems, setProblems] = useState<HeroProblem[]>(() =>
    opening.ok ? [] : toProblems(opening.problems),
  );
  const [race, setRace] = useState<RaceKey | null>(() =>
    knownRace(initialRace),
  );
  const [seed, setSeed] = useState("");
  const [locks, setLocks] = useState<Locks>({});

  const reported = useRef(false);
  useEffect(() => {
    if (!opening.ok && !reported.current) {
      reported.current = true;
      onInvalid?.(toProblems(opening.problems));
    }
  }, [opening, onInvalid]);

  // Continuous input — a held arrow key, a typed hex — redraws at most once a
  // frame (FR-021). Controls follow `spec` at once; the pictures follow `drawn`.
  const [drawn, setDrawn] = useState(spec);
  const latest = useRef(spec);
  const frame = useRef(0);
  const scheduleDraw = useCallback((next: HeroSpec) => {
    latest.current = next;
    if (frame.current !== 0) return;
    frame.current = requestAnimationFrame(() => {
      frame.current = 0;
      setDrawn(latest.current);
    });
  }, []);
  useEffect(() => () => cancelAnimationFrame(frame.current), []);

  const accept = useCallback(
    (candidate: Record<string, unknown>): boolean => {
      const result = validateHero(candidate);
      if (!result.ok) {
        const refused = toProblems(result.problems);
        setProblems(refused);
        onInvalid?.(refused);
        return false;
      }
      const next = minimalSpec(candidate as HeroSpec);
      setProblems([]);
      setSpec(next);
      scheduleDraw(next);
      onChange?.(next);
      return true;
    },
    [onChange, onInvalid, scheduleDraw],
  );

  const resolved: ResolvedHero | null = useMemo(
    () => (spec === null ? null : resolveHero(spec)),
    [spec],
  );
  const isSet = useMemo(
    () => (spec === null ? new Set<string>() : setFields(spec)),
    [spec],
  );

  if (spec === null || resolved === null || drawn === null) {
    return (
      <div className="tfhb" data-testid="hero-builder">
        <style>{BUILDER_CSS}</style>
        <ProblemList problems={problems} />
      </div>
    );
  }

  const change = (field: HeroField, value: unknown) =>
    accept({ ...spec, [field]: value });
  const follow = (field: HeroField) => {
    const without: Record<string, unknown> = { ...spec };
    delete without[field];
    accept(without);
  };
  const toggleLock = (field: HeroField) =>
    setLocks((current) => {
      const next = { ...current };
      if (next[field]) delete next[field];
      else next[field] = true;
      return next;
    });
  const roll = (withSeed: string) => {
    setSeed(withSeed);
    // A roll keeps who the hero is and what is locked, and replaces the rest,
    // so the same seed and race always give the same hero.
    const kept: Record<string, unknown> = { name: spec.name };
    if (spec.title !== undefined) kept.title = spec.title;
    for (const field of Object.keys(locks)) {
      const value = (spec as Record<string, unknown>)[field];
      if (value !== undefined) kept[field] = value;
    }
    accept({ ...kept, ...randomHero(withSeed, { locked: locks, race }) });
  };
  const value = (field: HeroField) =>
    (resolved as unknown as Record<string, unknown>)[field];

  return (
    <div className="tfhb" data-testid="hero-builder">
      <style>{BUILDER_CSS}</style>
      <div className="tfhb-stage">
        <HeroPreview spec={drawn} idPrefix={idPrefix} />
      </div>
      <div className="tfhb-panel">
        <ProblemList problems={problems} />
        <RollBar
          race={race}
          seed={seed}
          onRace={setRace}
          onRoll={roll}
          onRollFresh={() => roll(freshSeed())}
        />
        <div className="tfhb-texts">
          <TextControl
            field="name"
            label={fieldLabel("name")}
            value={spec.name}
            maxLength={MAX_NAME}
            onText={(text) => accept({ ...spec, name: text })}
          />
          <TextControl
            field="title"
            label={fieldLabel("title")}
            value={spec.title ?? ""}
            maxLength={MAX_TITLE}
            onText={(text) => accept({ ...spec, title: text })}
          />
        </div>
        {CHOICE_FIELDS.map(({ field, choices }) => (
          <ChoiceGroup
            key={field}
            field={field}
            choices={choices}
            value={String(value(field))}
            name={`${idPrefix}-${field}`}
            locked={locks[field] === true}
            onLock={() => toggleLock(field)}
            onChoose={(choice) => change(field, choice)}
          />
        ))}
        <div className="tfhb-flags">
          {FLAG_FIELDS.map((field) => (
            <FlagControl
              key={field}
              field={field}
              value={value(field) === true}
              locked={locks[field] === true}
              onLock={() => toggleLock(field)}
              onToggle={(on) => change(field, on)}
            />
          ))}
        </div>
        {COLOR_FIELDS.map((field) => (
          <ColorControl
            key={field}
            field={field}
            value={String(value(field))}
            isSet={isSet.has(field)}
            name={`${idPrefix}-${field}`}
            locked={locks[field] === true}
            onLock={() => toggleLock(field)}
            onPick={(hex) => change(field, hex)}
            onFollow={() => follow(field)}
          />
        ))}
        {actions && <div className="tfhb-actions">{actions}</div>}
      </div>
    </div>
  );
}

function ProblemList({ problems }: { problems: readonly HeroProblem[] }) {
  if (problems.length === 0) return null;
  return (
    <ul className="tfhb-problems" role="alert" data-testid="hero-problems">
      {problems.map((problem, index) => (
        <li key={index} data-field={problem.field}>
          {problem.field === ""
            ? problem.message
            : `${fieldLabel(problem.field)}: ${problem.message}`}
        </li>
      ))}
    </ul>
  );
}
