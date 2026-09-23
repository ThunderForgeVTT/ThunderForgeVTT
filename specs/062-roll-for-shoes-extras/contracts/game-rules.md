# Contract: the pure rules module, `web/src/game.ts`

**Feature**: [../spec.md](../spec.md) · **Decisions**: [../research.md](../research.md) D3–D6

`game.ts` imports no React, no network and no host. It is where four of the five
Extras actually live, and it is tested by `node --test` against `game.test.ts`
with no browser and no stack. This contract fixes what it exposes; the e2e then
proves those rules reached a screen rather than re-deriving the arithmetic.

**Everything below is additive.** Every function spec 061 shipped keeps its name
and its meaning, so a world with no settings runs the same code path it ran
before (FR-001, FR-004).

---

## Settings, as the rules see them

```ts
export interface WorldSettings {
  difficultyMode: "free" | "rolled" | "target";
  tieSucceeds: boolean;
  statusesEnabled: boolean;
  skillSlotsEnabled: boolean;
  startingSkills: { name: string; level: number }[];
}

export const DEFAULT_SETTINGS: WorldSettings;
```

`DEFAULT_SETTINGS` is the core game: `"free"`, three `false`s, and an empty
`startingSkills` meaning `Do Anything 1`. Passing it to anything below must
produce exactly what spec 061 produced — that is the single most important
property in this file and it gets its own tests.

---

## Difficulty (FR-008–014)

```ts
export type Band = "easy" | "moderate" | "hard" | "veryHard";

export const BAND_DICE: Record<Band, number>;    // 1, 2, 3, 4
export const BAND_TARGET: Record<Band, number>;  // 3, 6, 9, 12
```

Two lookup tables, not a formula. The site publishes four bands with two sets of
numbers; a formula would imply a fifth band exists.

The mode is a world setting; the band is a per-roll choice. `free` leaves the
opposition the typed number spec 061 already accepts, and remains available in
every mode — a Game Master who wants to say "beat 7" may always say it.

---

## The resolved roll (FR-015–023)

```ts
export interface RollInput {
  faces: number[];
  statuses: Status[];
  opposition: number | null;
  tieSucceeds: boolean;
}

export interface RollOutcome {
  sum: number;        // the dice as rolled
  modifier: number;   // the statuses, summed
  total: number;      // sum + modifier
  verdict: Verdict;
  xpAwarded: number;
}

export function resolve(input: RollInput): RollOutcome;
```

**The order of those fields is the decision** (research D3): dice → sum →
statuses → opposition → comparison under the tie rule. Three Extras touch one
roll, and left implicit every pair of them is a question somebody has to
re-derive. `game.test.ts` asserts the order directly.

- `opposition: null` yields `verdict: "unjudged"` and `xpAwarded: 0`, as today.
- `tieSucceeds: false` — `total > opposition`, and a tie is a failure earning
  1 XP. Unchanged from spec 061.
- `tieSucceeds: true` — `total >= opposition`, and a tie awards no XP. The
  suppression is not a second rule: a tie is a success, and successes never
  award XP.

```ts
export function verdict(total: number, opposition: number | null): Verdict;
```

Kept, and re-expressed in terms of `resolve` so spec 061's tests stay meaningful
rather than being rewritten to match new code.

---

## Statuses (FR-019–027)

```ts
export interface Status { id: string; name: string; modifier: number }

export function statusesOf(traitData: Record<string, unknown> | null | undefined): Status[];
export function statusModifier(statuses: Status[]): number;
export function addStatus(statuses: Status[], name: string, modifier: number): Status[] | StatusRefusal;
export function removeStatus(statuses: Status[], id: string): Status[];
```

`statusesOf` returns `[]` when nothing is stored — unlike `skillsOf`, there is no
default to supply, so no read-time-default problem arises here.

Modifiers sum, including to zero, and may be negative. Names are labels the
table writes; the system ships no list and validates only that a name is not
empty (FR-020).

### The guarantee that gets a test of its own

```ts
export function isAdvancement(faces: number[], bought: number): boolean;
```

**Unchanged signature, and it is never passed a status.** A function that cannot
see statuses cannot be affected by them, which is how FR-023 — a status can
neither create nor destroy an advancement — is made structural rather than
remembered (research D4). `sixesShown` and `remainingNonSixes` likewise keep
their signatures.

---

## Skill slots (FR-028–036)

```ts
export const SLOT_CAPS: Record<number, number>;  // { 2: 4, 3: 3, 4: 2 }

export function capAtLevel(level: number): number | null;   // null = uncapped (1, 5+)
export function slotsUsed(skills: Skill[], level: number): number;
export function slotsAvailable(skills: Skill[], bought: BoughtSlots, level: number): number | null;
export function slotCost(level: number): number;            // level * 2

export function buySlot(
  balance: number, bought: BoughtSlots, level: number,
): SlotBought | SlotRefusal;
```

- Levels 1 and 5+ are uncapped: `capAtLevel` returns `null`, and
  `slotsAvailable` returns `null` meaning "no limit", never a large number.
- A character at the cap who rolls an advancement is **told there is no room**
  (FR-030). The refusal is a returned reason, not a thrown error and not a
  silent denial — the advancement happened, the room did not exist.
- `slotCost(level)` is `level * 2`, so a second-level slot costs 4 XP.

### The XP ledger (FR-035)

`buySlot` and the existing `spendXp` debit the same `resource_data.xp`. Both
refuse on an insufficient balance rather than going negative, and both return
the new balance rather than mutating. This is the only Extra that spends XP on
anything other than turning a die into a six, and the two paths agreeing is
worth asserting in one test that exercises them in sequence.

**No validator rule caps a stored character's breadth.** Enabling the setting in
a world whose characters already exceed the caps must not make them unstorable
(FR-036) — the cap governs gaining a skill, which is a transition, not a shape
(research D6).

---

## Starting skills (FR-037–041)

```ts
export function startingSkills(settings: WorldSettings): Skill[];
export function skillsOf(traitData: ..., settings?: WorldSettings): Skill[];
```

`startingSkills` returns the world's set, or the core `Do Anything 1` when the
world defines none — so the pack's default and a world's override have one shape.

`skillsOf` keeps its existing one-argument behaviour when `settings` is omitted,
and every stored character is unaffected by either. The read-time default and
what FR-039 therefore protects are set out in research D5; the rule this file
holds is narrower and mechanical: **a character with stored skills never reads
the world's starting skills**, whatever the world later says.
