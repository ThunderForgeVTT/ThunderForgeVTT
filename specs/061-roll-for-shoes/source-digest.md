# Roll for Shoes — Implementation Digest

Spidered 2026-09-22 from <https://rollforshoes.com/>. Every page on the site was retrieved (the site
has no sitemap; the full page set was discovered by crawling the nav and the Settings index, then
re-crawling each of those pages for further internal links — no further internal links exist beyond
those listed below). Nothing outside `rollforshoes.com` was fetched.

---

## 1. Source URLs

| # | URL | What is on it |
|---|-----|---------------|
| 1 | `https://rollforshoes.com/` | The complete core rules: a six-item numbered list, plus the attribution sentence and the CC0 footer. |
| 2 | `https://rollforshoes.com/extras/` | Optional/expanded rules: Advancement clarifications, Difficulty table, Ties, Statuses, Skill Slots, Leveling, Starting Skills. |
| 3 | `https://rollforshoes.com/tools/` | A General Body Parts list and two D66 NPC tables (Description, Personality), each with a browser "Roll" button. |
| 4 | `https://rollforshoes.com/settings/` | Index of one third-party adventure and six first-party scenarios, plus the shared scenario format. |
| 5 | `https://rollforshoes.com/settings/the-heist` | "The Heist" scenario. |
| 6 | `https://rollforshoes.com/settings/kitchen-khaos` | "Kitchen Khaos" scenario. |
| 7 | `https://rollforshoes.com/settings/minimon` | "Minimon: Miniature Monsters" scenario. |
| 8 | `https://rollforshoes.com/settings/outbreak-hotel` | "Outbreak Hotel" scenario. |
| 9 | `https://rollforshoes.com/settings/redshirt-recon` | "Redshirt Recon" scenario. |
| 10 | `https://rollforshoes.com/settings/the-stranger` | "The Stranger" scenario. |
| 11 | `https://rollforshoes.com/links/` | Community links (Discord, Reddit, RPG Stack Exchange) and an actual-play list (podcasts, YouTube). No rules content. |
| 12 | `https://rollforshoes.com/new-shoes/` | "New Shoes!" — a nine-rule variant ruleset, described on the page as a stabilized evolution of the core game for longer scenarios. |

External links present on the site but **not fetched** (recorded for completeness): a
`web.archive.org` snapshot of a story-games.com forum discussion (the attribution target on the
homepage); `creativecommons.org/publicdomain/zero/1.0` (the licence target, in every page footer);
`sasswrites.itch.io` (third-party adventure); `discord.gg`, `reddit.com/r/rollforshoes`,
`rpg.stackexchange.com` (communities); assorted YouTube/Podbean actual-play links. The only external
URL used for confirmation was the CC0 deed target named in the footer.

---

## 2. Core rules (homepage)

The homepage states the entire game as six numbered rules. Paraphrased, with the load-bearing
wording quoted:

1. **Declare and roll.** Say what you do, then roll a number of d6 equal to the level of the
   relevant skill you have. Pool size = skill level. Dice are always d6.
2. **Resolution is sum-vs-sum.** "*If the sum of your roll is higher than an opposing roll, the thing
   you wanted to happen, happens.*" Note the strictness: **higher than**, not "equal or higher" —
   so a tie is not a success in the core rules (the Extras page adds a tie rule; see §5).
3. **Starting character.** "*At start, you have only one skill: Do Anything 1.*" One skill, named
   *Do Anything*, at level 1 — i.e. every character begins rolling exactly 1d6 for everything.
4. **All-6s grants a new skill.** "*If you roll all 6s, you get a new skill specific to the action,
   one level higher than the one you used.*" So rolling N sixes on an N-dice skill of level N yields
   a new skill at level N+1, named for the specific action just attempted.
5. **Failure earns XP.** "*For every roll you fail, you get 1 XP.*" One XP per failed roll.
6. **XP buys sixes, for advancement only.** "*XP can be used to change a die into a 6 for advancement
   purposes only.*" XP converts rolled dice into 6s so that rule 4 can trigger; it does **not**
   change whether the action succeeded.

The Extras page makes the intent of rule 6 explicit: spending XP to force all-6s gains you the new
skill, but "*the narrative result of the action still stands as rolled*" — i.e. a failed roll stays
failed even after you buy the advancement off it. Extras also states that the XP gained from a
failure may be spent immediately on that same roll ("*After you fail a roll, you can immediately use
the gained XP for a new skill.*").

There is **no** hit-point, wound, or death system in the core six rules. There is no attribute
array, no inventory rules, no initiative or turn order, no action economy, no currency. Everything
is skills, d6 pools, and XP.

### Skill naming/specificity

The core rules require only that a new skill be "specific to the action". The Extras page tightens
this: "*New skills should be more specific than the skill rolled, and relevant to the action taken.*"
So a skill tree is a chain of narrowing specificity rooted at *Do Anything 1* — e.g. *Do Anything 1*
→ *Climbing 2* → *Climbing Walls in the Rain 3*. The site gives no formal grammar, no banned-word
list, and no arbitration procedure beyond "should be more specific"; adjudication is the table's.

### Relationship between skill levels

A skill obtained via rule 4 is exactly **one level higher** than the skill whose dice were rolled.
Skills are therefore not "ranks of a single skill" but separate, independently-named entries, each
with its own integer level, forming an implicit parent→child lineage. The site never says a child
skill's level can later increase in place; advancement always produces a *new, more specific* skill
at level+1. The site states no maximum level.

---

## 3. Character model (fields a sheet needs)

Derived from the core rules plus Extras plus the scenario pages. Types and ranges below are the
minimum a faithful implementation needs; where the site is silent this is flagged in §9.

### Core (always required)

| Field | Type | Range / default | Notes |
|---|---|---|---|
| `name` | string | free text | Every scenario asks for "your name and a brief description". |
| `description` | string (multiline) | free text | Same. |
| `skills[]` | list of Skill | at least 1; starts with exactly one entry | See Skill below. |
| `xp` | integer | ≥ 0, starts at 0 | +1 per failed roll; spent 1-per-die to turn a die into a 6. |

### Skill

| Field | Type | Range / default | Notes |
|---|---|---|---|
| `name` | string | free text | Starting skill is literally `Do Anything`. |
| `level` | integer | ≥ 1; starts at 1 | Equals the d6 pool size for that skill. No cap stated. |
| `parent` | reference to Skill, nullable | null for `Do Anything` | The skill that was rolled when this one was gained; `level == parent.level + 1`. Useful for enforcing the specificity rule and for rendering a tree. |
| `origin` | enum | `starting` \| `advancement` \| `scenario-granted` | Scenarios grant skills directly (see Minimon, §6). |

### Optional-rule fields (only if the corresponding Extras rule is enabled)

| Field | Type | Range / default | Notes |
|---|---|---|---|
| `statuses[]` | list of Status | empty | Extras §Statuses. |
| `skillSlots` | map level→int | e.g. {2:4, 3:3, 4:2} | Extras §Skill Slots; only if slot-limiting is on. |
| `disabledSkills[]` (New Shoes only) | list of Skill refs | empty | Cleared at end of scene. |
| `marks` (New Shoes only) | boolean per Skill | false | Replaces XP entirely in that variant. |
| `outOfScene` (New Shoes only) | boolean | false | Knocked out of the current scene. |

### Status (Extras)

| Field | Type | Range | Notes |
|---|---|---|---|
| `rating` | signed integer | negative or positive; examples on the site are −4, −3, −2, +2 | Written **rating first**, e.g. `-2 Weak to Fire`, `+2 Clean Shoe`, `-3 Broken Nose`. |
| `label` | string | free text | The narrative name. |
| `permanence` | enum | `temporary` \| `semi-permanent` \| `permanent` | Site uses all three informally (Minimon: "permanent status"; Outbreak Hotel: "semi-permanent"). |
| `scope` | string, optional | free text | "modify the results of any *related* actions" — relatedness is GM-adjudicated, not encoded. |

Statuses are flat numeric modifiers **added to the roll total after summing dice**, not dice-pool
changes. A status persists "*until it's removed, either by action or otherwise made narratively
untrue*". Crucially for advancement: "*Advancement still works as normal with statuses, based on the
rolled dice*" — i.e. the all-6s check inspects the raw dice, never the status-modified total.

### Scenario-specific character fields observed

Scenarios add free-text fields that a generic sheet can model as a list of prompt/answer pairs:
court position + assigned feast task (Kitchen Khaos); species name, how you met your Coach, element
skill, weakness status, signature Move skill (Minimon); reason you're here + a phobia (Outbreak
Hotel); ship job, why you were chosen, assigned equipment (Redshirt Recon); heist job + reason for
joining (The Heist); how you make a living + spare-time activity (The Stranger).

---

## 4. Advancement & XP (consolidated)

- **Earning XP:** exactly 1 XP per failed roll. Nothing else on the site grants XP. Ties grant **no**
  XP (Extras: "*You don't get XP*" on a tie).
- **Spending XP:** 1 XP converts one rolled die into a 6. The only purpose is to complete an all-6s
  result and thereby trigger rule 4. It cannot buy success, cannot buy levels directly, and cannot be
  banked into anything else — except under the optional *Leveling* rule (below), where XP can also
  buy a skill slot at a cost of twice the level.
- **Timing:** XP from a failure is usable immediately on the roll that produced it.
- **Result of advancement:** one new skill, named for the specific action, at (rolled skill's level +
  1). The old skill is retained.
- **Rate implication for implementers:** a level-1 character rolls 1d6; a natural 6 immediately
  yields a level-2 skill, or 1 XP converts the single die to a 6. Higher levels need progressively
  more sixes (N dice all showing 6), so XP cost to force an advancement from level N is
  N − (number of natural 6s rolled).

---

## 5. Optional rules (all from `/extras/`, all explicitly supplementary to the six core rules)

Every item in this section is **optional**. The homepage's six rules stand alone; Extras is a
separate page of additions.

### 5.1 Advancement clarifications (optional clarifications, not new mechanics)
- New skills must be more specific than the skill rolled and relevant to the action taken.
- Rule 6's "for advancement purposes only" = buying 6s gains the skill, the narrative outcome is
  unchanged.
- XP from a failure may be spent immediately on that same roll.

### 5.2 Difficulty (optional — the core rules only mention "an opposing roll")
When the opposition is not another character, the GM either rolls a pool or uses a static target:

| Difficulty | GM dice (d6) | Static target |
|---|---|---|
| Easy | 1 | 3 |
| Moderate | 2 | 6 |
| Hard | 3 | 9 |
| Very Hard | 4 | 12 |

The static numbers are exactly 3× the dice count (average of a d6 rounded to 3.5 → they use 3).
The GM chooses per task whether to roll or use the static number. The site's worked example has a
Hard task rolled as 3d6 for a total of 7, which the player then had to beat.

### 5.3 Ties (optional)
On an exact tie, you "*both partially succeed and fail*": no XP, and you accomplish the goal
"*barely*", with an unexpected twist or a minor consequence. The site's example: the climber reaches
the top but loses a shoe. (Note this softens core rule 2, under which a tie is simply a failure —
but it also removes the XP a failure would have given.)

### 5.4 Statuses (optional)
Signed numeric modifiers written rating-first, applied to roll totals, persisting until removed or
made narratively untrue. Ratings may be randomized with dice if desired. Four named uses:
- **Harm/damage**, rated by the result of the inflicting action (this is the site's only
  damage/health system, and it is optional);
- **Temporary conditions** (site examples: *Poisoned*, *Highly Caffeinated*);
- **Advantages and disadvantages** (site examples: *-4 Raining*, *+2 Clean Shoe*);
- **Progress tracking** — a craft project, an item's quality, etc.

The damage example: a defender with *Karate 2* rolls against a Moderate 6 attack, totals 3, and takes
a `-3 Broken Nose`. Read literally, the status's rating equals the **margin** by which the defence
was beaten (6 − 3 = 3), which is consistent with "rated by the result of the inflicting action" only
under one reading — see §9.

### 5.5 Skill Slots (optional — for longer campaigns or to restrict advancement)
Cap the number of skills per level. Gaining a skill when the level's slots are full forces the player
to replace an existing skill. Suggested starting "triangle": four level-2 slots, three level-3 slots,
two level-4 slots. (The site does not state a slot count for level 1 or for level 5+.)

### 5.6 Leveling (optional, layered on Skill Slots)
Skill slots may be purchased with XP at a cost of **twice the level** (so a level-3 slot costs 6 XP).
Buying the slot only makes it available; obtaining the skill still happens normally (all-6s). A
player may buy the slot at the same moment they obtain the skill.

### 5.7 Starting Skills (optional)
Depending on scenario, characters may start customized with additional skills — broad
archetypes/classes, scenario-specific skills, and so on, rather than only *Do Anything 1*.

### 5.8 "New Shoes!" (a separate, optional variant ruleset — `/new-shoes/`)
Presented as "*A stabilized evolution of Roll for Shoes for longer scenarios.*" It is a **replacement**
rule list, not an add-on; nine rules, and it removes XP entirely in favour of marks:

1. Roll d6 equal to the level of the **most relevant** skill you have.
2. Success if your sum **beats** the opposing roll.
3. You start with **three level-1 skills**: single words describing who you are and what you know.
4. All 6s → advance the skill used: gain a new skill one level higher, specific to the action, and
   clear any mark on it. "*This can only happen once per character per scene.*"
5. Fail with an **unmarked** skill → mark it.
6. **Tie** with a **marked** skill → advance it and unmark it.
7. No relevant skill → roll 1 die, with no advancement possible.
8. Fail a **dangerous** action → the skill used is **disabled** for the rest of the scene.
9. No relevant skill for a dangerous action → you are **knocked out of the scene**. All disabled
   skills recover at the end of a scene.

Differences an implementation must respect if it offers this mode: no XP at all; three starting
skills instead of one; skill names are single words at level 1; a per-scene advancement cap; marks
are a per-skill boolean; "dangerous" is a GM flag on an action; scene is a first-class concept with
an end-of-scene reset step.

---

## 6. Settings (the `/settings/` index)

The index lists one third-party **adventure** and six first-party **scenarios**. Every scenario is
laid out the same way, which the index states explicitly: an **Introduction** (questions and initial
prompts to set up the scenario), **Gameplay Segments** (narrative blocks guiding phases of the game),
and **Random Tables** (tables to roll on for setting-relevant data). Each scenario has exactly three
gameplay segments.

- **The Night Before Christmas** — an adventure by *SassWrites*, hosted off-site on itch.io. The
  site gives no description beyond the title and author.

- **The Heist** — a team of supposedly skilled sleuths plans and executes a theft. Setup questions
  establish the treasured item and where/how it is secured; each player records a name/description,
  their assigned job on the team, and their reason for joining. Segments: *Planning and Recon*
  (in-character planning, establishing connections, obstacles and positioning), *Action!* (the plan
  runs, each character gets a beat, and a twist lands late in the segment), and *The Escape* (pressure
  escalates toward a cliffhanger or set piece). Tables: six Professions (law enforcement, charismatic
  "expert", stuntperson, lockpicker, self-proclaimed master of disguise, loudmouth jerk) and six
  Twists (item is elsewhere; a double-cross; a trap captures a team member; a rival thief or crew
  appears; the item is not what it seems; the authorities get the jump on the team).

- **Kitchen Khaos** — courtly servants must throw together a grand royal feast at the last minute.
  Setup establishes the occasion, why the court is uptight, why preparation fell behind, what is at
  stake, and a rare nigh-mythical key ingredient and why it is hard to get; each player records name
  and description, court position, and assigned feast task. Segments: *Gathering the Ingredients*,
  *If You Can't Stand the Heat...* (the cooking itself), *Dinner is Served* (the court judges the
  food and the attendant chaos). Table: six Species (Dragonkin, Dwarf, Elf, Halfling, Human, Orc),
  with a pointer to the NPC Tables for adjectives.

- **Minimon: Miniature Monsters** — the PCs are a team of miniature monsters helping their Coach
  find and catch the last, elusive species. This is the only scenario that hands out **mechanics** at
  creation: each player records a species name and description, how they first met their Coach, an
  element type as a **level-2 skill**, an element type they are weak against as a **permanent level
  (−2) status** (site example: `-2 Weak to Fire`), and a signature Move as a **level-3 skill**.
  Segments: *I Choose You!* (start in medias res in a fight against a rival trainer), *Missile
  Squad* (a greedy villain group; framed as a problem-solving challenge rather than necessarily a
  fight), *The Legendary* (the climactic hunt for the elusive Minimon). Tables: six Locations
  (slimy swamp, snowy mountain, spooky cave, sunny beach, tangled forest, windy desert), six Forms
  (bird, blob, canine, feline, insect, rodent), and twelve Types split across two d6-ranges —
  1–3: Bug, Electric, Fighting, Fire, Grass, Ground; 4–6: Ice, Metal, Poison, Psychic, Rock, Water.

- **Outbreak Hotel** — vacationers and staff at a packed resort hotel survive a zombie-spawning
  virus. Setup establishes where the hotel is and what it is called (which in turn defines the
  resort's theme and amenities); each player records name and description, their reason for being
  there, and a phobia of a common everyday thing. The page advises that character details should
  define the supporting cast, ideally with each PC attached to an NPC and several PCs sharing one,
  and suggests the phobia can optionally become a semi-permanent negative status (site example:
  `-2 Fear of Heights`). Segments: *Checking In* (overwhelming activity with subtle outbreak
  foreshadowing), *The Outbreak* (NPCs turn, the hotel is overrun, set-piece action), *Escape*
  (a route out presents itself; final actions are heavily punished or rewarded). Table: six Hotel
  Occupations (bellhop, housekeeper, manager, pool maintenance person, receptionist, valet).

- **Redshirt Recon** — a Spacefleet away team investigates a distress signal planetside. Setup
  establishes the planet type, what makes the crew hesitant, and what convinces the Captain to send
  a team; each player records name and description, their lowly shipboard job, why they were chosen,
  and one piece of issued equipment. An NPC mission leader (a Chief Officer or equivalent) is rolled
  up on the NPC Tables. Segments: *Mission Briefing*, *Planet Exploration*, *Source Encounter*.
  Tables: six Dangers (aggressive wildlife; a corrupt former Spacefleet Admiral; a rogue violent
  android; environmental disaster; attack by a local primitive species; evil clones of the away
  team), six Species (robot, reptilian alien, insectoid alien, sentient plant, mutant, space pirate),
  and six Signal Sources (a crashed ship; a failed colony; a trap laid by enemies; a creature
  emitting the signal; a space anomaly emitting the signal; an otherworldly artifact).

- **The Stranger** — a struggling western frontier town receives an ominous visitor. Setup
  establishes the town's name, why it exists or what it is known for, and nearby landmarks; each
  player records name and description, how they make a living, and what they do in their spare time.
  Segments: *A Typical Afternoon* (ordinary life, funnelling toward the saloon at evening), *The
  Stranger's Arrival* (the black-clad stranger picks a fight, each PC interacts with him, and one is
  called out for a duel at high noon the next day), *High Noon* (the town's tension, then the duel
  itself — the page notes the stare-down-and-draw is only the *expected* outcome and the actual
  resolution is up to the players; if the stranger survives he drinks and rides out of town).
  Table: six Professions (blacksmith, barkeep, doctor, prospector, prostitute, tailor).

---

## 7. Tools (`/tools/`)

Three things, all generator-grade content rather than character management. **There is no character
sheet, no dice roller, and no character generator on the site.**

1. **General Body Parts** — a flat list of six: head, left arm, right arm, torso, left leg, right
   leg. No roll instructions are given, but the six-item shape makes it a d6 table; in practice it
   pairs with the optional Statuses rule for locating harm. *Input:* none/implicit d6. *Output:* a
   body location.

2. **NPC Description table** — 36 adjectives indexed by **D66** ("*Roll D66 (two D6 side by side)*"),
   i.e. first die = tens digit, second = units, values 11–66 with no 7/8/9/0. Entries: 11 Adorable,
   12 Attractive, 13 Bald, 14 Bearded, 15 Beefy, 16 Bony, 21 Bulky, 22 Chiseled, 23 Chubby,
   24 Clean, 25 Creepy, 26 Elderly, 31 Filthy, 32 Furry, 33 Glamorous, 34 Huge, 35 Lanky,
   36 Muscular, 41 Obese, 42 Pasty, 43 Petite, 44 Scary, 45 Shifty, 46 Short, 51 Slender, 52 Slimy,
   53 Spiky, 54 Stinky, 55 Stylish, 56 Sunburned, 61 Tall, 62 Tattooed, 63 Tentacled, 64 Tiny,
   65 Ugly, 66 Voluptuous.

3. **NPC Personality table** — 36 traits, same D66 indexing: 11 Annoying, 12 Arrogant, 13 Awkward,
   14 Bossy, 15 Clumsy, 16 Confident, 21 Courageous, 22 Demanding, 23 Embarrassed, 24 Enthusiastic,
   25 Evil, 26 Excited, 31 Fearless, 32 Fidgety, 33 Friendly, 34 Grumpy, 35 Judgemental, 36 Kind,
   41 Lazy, 42 Maniacal, 43 Mean, 44 Messy, 45 Murderous, 46 Nervous, 51 Noisy, 52 Optimistic,
   53 Quiet, 54 Rowdy, 55 Rude, 56 Sad, 61 Sarcastic, 62 Selfish, 63 Shy, 64 Silly,
   65 Simple-Minded, 66 Stubborn.

**Interactive behaviour.** Each NPC table has a **Roll** button. The page's inline script picks a
cell uniformly at random from the table and highlights it — and it explicitly *excludes the currently
selected cell from the candidate pool*, so pressing Roll twice never returns the same result twice in
a row. That is a "roll without immediate repeat", not a true D66 roll; a faithful VTT implementation
should decide whether to copy that behaviour or roll honestly (see §9). *Input:* a button press.
*Output:* one highlighted table entry.

An NPC is assembled by rolling once on Description and once on Personality (the scenario pages then
add a third roll on their own profession/species table — e.g. Kitchen Khaos and Outbreak Hotel both
say to "roll for adjectives on NPC Tables").

---

## 8. Licence & attribution

**Licence.** Every page carries the same footer, immediately after a mailto link and a `|` divider.
The exact visible text is:

> "This work is marked with CC0 1.0"

…where "CC0 1.0" is a link to `http://creativecommons.org/publicdomain/zero/1.0?ref=chooser-v1`,
followed by the two Creative Commons badge icons. The markup uses the CC chooser's standard
`xmlns:cc` wrapper. This is a **CC0 1.0 public-domain dedication**, and the phrasing "is marked
with" is the CC Public Domain Mark wording produced by the CC chooser tool. It appears in the
`<footer>` of the homepage, Extras, Tools, Settings, each individual setting page, Links, and New
Shoes — i.e. site-wide, with no per-page variation and no additional terms anywhere.

**Attribution.** The homepage's opening sentence reads:

> "**Roll for Shoes** is a tabletop RPG "micro system" [created by *Ben Wray*], with these simple
> rules:"

…where the bracketed text is a hyperlink pointing at a `web.archive.org` snapshot of a
`story-games.com` forum discussion. The link's URL path contains the thread slug
`microdungeons-i-roll-to-see-if-i-have-shoes-on` and the archive timestamp `20200619145843`.

Points to be careful about for any attribution we ship:
- The site names **Ben Wray** as creator, and nobody else. It does **not** name "DWeird", does
  **not** give a year (2010 or otherwise), and does **not** describe the forum thread beyond
  linking it. Anything beyond "created by Ben Wray, per rollforshoes.com, which links a
  story-games.com thread" is not supported by the site.
- The site names no author for itself as a website, and no publisher or organisation. The only
  contact is an obfuscated mailto in the footer (Cloudflare email protection; the address is not
  rendered in the raw HTML).
- **SassWrites** is credited on the Settings page as the author of the off-site adventure *The Night
  Before Christmas*.
- Under CC0 no attribution is legally required, but crediting Ben Wray and rollforshoes.com is the
  courteous and expected practice.

---

## 9. Open questions / ambiguities an implementation must decide

Each of these is genuinely undefined or underdefined by the site.

**Resolution**
1. **Ties in the core rules.** Core rule 2 requires the sum to be *higher*, so a bare tie is a
   failure and (by rule 5) earns 1 XP. The optional Extras tie rule says a tie is a partial
   success/partial failure that earns **no** XP. These cannot both be active. Decide whether the tie
   rule is a toggle, and if on, whether it suppresses the XP award.
2. **Unopposed actions.** The core rules assume there is always "an opposing roll". They never say
   what happens if the GM sets no opposition. Difficulty (Extras) fills this, but is optional.
3. **Static target vs. roll.** With a static target, is the target beaten on exceeding it only
   (consistent with rule 2), or met-or-exceeded? The Extras worked example only covers the rolled
   case. Also undefined: whether the GM's choice must be declared before the player rolls.
4. **Opposed rolls between two PCs / PC vs NPC.** The site never states who rolls first, whether the
   defender rolls at all, or how a GM-side character's skill level is determined. NPC skill levels
   are never specified anywhere on the site.
5. **Which skill applies.** Core says "relevant skill"; New Shoes says "most relevant skill you have".
   The core rules do not say what happens when several skills could apply, nor whether skills stack
   (they clearly do not, but it is never stated). Also: the core rules never say what happens if a
   more specific skill *doesn't* apply — presumably you fall back to a parent or to *Do Anything 1* —
   but the fallback is only spelled out in New Shoes (rule 7: roll 1 die, no advancement).
6. **Multiple dice pools / partial relevance.** Not addressed at all.

**Advancement**
7. **Is there a level cap?** Never stated in core or Extras. The Skill Slots triangle stops at level
   4, and Leveling lets you buy slots "at a cost of twice the level" without bounding the level. An
   implementation must pick a cap or allow unbounded growth (and note the XP cost to force an
   advancement grows linearly with level, so growth self-limits in practice).
8. **XP and the all-6s rule interaction.** Several sub-questions: (a) May XP be spent *after* seeing
   the dice? (Extras implies yes — you spend XP you just earned from the failure.) (b) May XP be
   spent on a *successful* roll to convert non-6s into 6s and harvest an advancement? Rule 6 says
   only "for advancement purposes", and nothing forbids it, so presumably yes. (c) May XP convert
   dice on a roll where you rolled zero natural 6s at level 4 — i.e. buy all four — for 4 XP? Nothing
   forbids it. (d) Is there a cap on XP spent per roll? None stated. (e) Does spending XP on a *tie*
   work, given ties award no XP under the optional rule? Undefined.
9. **Can the same action produce the same skill twice?** Nothing prevents advancing off *Climbing 2*
   twice into two different level-3 skills, nor prevents creating a duplicate name. No uniqueness
   rule exists.
10. **Is the specificity rule enforceable?** "More specific than the skill rolled" has no formal
    test. A VTT must either leave it to GM approval (recommended) or invent a heuristic.
11. **Can you advance from a skill you gained this session/scene?** Core has no limit at all; New
    Shoes caps advancement at once per character per scene. Core's lack of a limit means a lucky
    streak can chain advancements in one action sequence.
12. **Skill slots at level 1 and 5+.** The triangle specifies only levels 2–4. Undefined whether
    level-1 skills occupy slots (the starting *Do Anything 1* presumably does not) and what slot
    counts continue above 4.
13. **Replacing a skill under slot limits.** When a slot is full, you "choose to replace" — the site
    does not say whether descendants of a removed skill survive, or whether the removed skill is
    refunded in any way.
14. **Buying slots:** "twice the level" — the level of the *slot* being bought, presumably, but the
    site's phrasing could also be read as the level of the skill triggering the purchase. In the
    normal case they are the same.

**Health / damage / death**
15. **There is no health system at all in the core rules.** Damage exists only as one bullet under the
    optional Statuses rule. A VTT must decide whether to ship a hit-point-like track, and if it does,
    it is inventing something the game does not have.
16. **How is damage rated?** "Rated by the result of the inflicting action." The site's own example
    gives `-3 Broken Nose` when a *Karate 2* defence totalled 3 against a Moderate (6) attack — which
    equals the defender's roll, the margin of failure, and nothing obviously equal to "the result of
    the inflicting action" unless the attack's result is read as the static 6 and the status as the
    margin. This example is genuinely ambiguous; pick one formula (defender's total? margin? attacker's
    total?) and document it.
17. **Death, incapacitation, and recovery.** Nothing in the core game or Extras. Statuses are removed
    "by action or otherwise made narratively untrue" — no healing rule, no rest, no track. New Shoes
    is the only place with an incapacitation rule (knocked out of the scene) and a recovery rule
    (disabled skills recover at end of scene), and that is a different ruleset.
18. **Do statuses stack?** Multiple statuses on one character presumably all apply to a related action,
    but the site never says so explicitly, nor whether two identically-named statuses can coexist, nor
    whether a total can be driven below zero (and what a negative total means against a positive
    opposing roll — presumably just a loss).
19. **Status relevance.** "Modify the results of any *related* actions" — relatedness is pure GM call.
    A VTT must decide between always-apply, manual-toggle-per-roll (recommended), or a tag system.

**Scene / session structure**
20. **"Scene" is undefined** but load-bearing in New Shoes (per-scene advancement cap, disabled-skill
    recovery, knocked-out-of-scene). A VTT implementing New Shoes needs an explicit scene boundary
    control.
21. **"Dangerous" is undefined** in New Shoes — it is a GM label on an action, with no criteria given.
22. **New Shoes starting skills** must be "single words". Is that enforced? And do skills gained later
    in New Shoes remain single words, given they must be "specific to the action"? Presumably not, but
    it is unstated.
23. **New Shoes has no XP.** If a VTT offers both modes, switching modes mid-campaign has no defined
    conversion.

**Tools / content**
24. **The Roll buttons exclude the previous result.** Faithful-to-the-site behaviour is a
    no-immediate-repeat draw; faithful-to-the-stated-rule behaviour ("Roll D66") is a uniform draw
    over 36. Pick one; note it.
25. **The Body Parts list has no stated roll or use.** Six entries invites a d6, and Statuses invite
    hit locations, but the site connects neither.
26. **Scenario tables have no stated die.** Most are six entries (implicitly d6); the Minimon Types
    table is twelve entries presented as two d6 groups labelled 1–3 and 4–6, which reads as a d6 to
    pick the group-half then a d6 within it, but the exact procedure is not written down.
27. **Starting Skills (optional) gives no budget.** "Additional starting skills" — how many, at what
    levels, is entirely open. Minimon is the only concrete data point: one level-2 skill, one level-3
    skill, one −2 permanent status, on top of the assumed *Do Anything 1*.
28. **Whether *Do Anything 1* is ever lost or replaceable.** Under Skill Slots, presumably never —
    but unstated.
