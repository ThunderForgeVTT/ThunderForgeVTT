# Open Game System Sourcing Guidelines

This document explains **why** ThunderForgeVTT ships the game systems it ships, **what license each one relies on**, the **exact attribution** each requires, and **how to propose adding another system**. It exists so that "can we add System X?" has a repeatable, defensible answer instead of a one-off legal review every time.

It distills a longer legal research pass into the operating rules the project actually follows.

> **A note on `research/`**: the working research notes and per-system digests referenced throughout this document (`vtt_open_license_game_systems.md`, `system_<id>.json`) live in a local `research/` directory that is **intentionally excluded from version control** (see `.gitignore`). Those digests were extracted from official source PDFs/documents that are themselves not ours to redistribute — only this document's *distilled conclusions* (the license names, selection criteria, and the attribution text below, which the licenses themselves require us to republish) are checked into the repository. If you need the underlying digests to do extraction work on a new or existing system, ask a maintainer.

## Table of contents

- [The two-minute version](#the-two-minute-version)
- [Selection criteria](#selection-criteria)
- [Systems we ship, and why](#systems-we-ship-and-why)
- [Attribution text (copy verbatim)](#attribution-text-copy-verbatim)
- [Systems we deliberately do NOT ship](#systems-we-deliberately-do-not-ship)
- [What "in scope" content looks like](#what-in-scope-content-looks-like)
- [User-generated content boundary](#user-generated-content-boundary)
- [Proposing a new system](#proposing-a-new-system)
- [Related documents](#related-documents)

## The two-minute version

- Game **mechanics** (dice, target numbers, action economy) are not copyrightable — anyone can implement them. What's protected is the **specific written expression** of those mechanics (the exact spell text, a stat block's wording, named proprietary content).
- We only ship systems distributed under a license that is **irrevocable or perpetual**, from the **publisher's own official System Reference Document (SRD)** — never OCR'd from a retail rulebook, never a fan transcription.
- Every shipped system carries **required attribution** and, where its license demands one, a **specific notice or compatibility badge** — rendered to the GM in-app, not buried in a Terms of Service page. See spec [`016-system-pack-legal-compliance`](../specs/016-system-pack-legal-compliance/spec.md) for how that's enforced in the manifest contract and UI.
- We never claim a trademarked product name as our own module name (no "D&D System," no "Includes Pathfinder 2e"). We use each publisher's own stated safe-harbor phrasing instead.
- User-entered compendium content is the user's own responsibility, not something we pre-ship — see [`015-dmca-notice-takedown`](../specs/015-dmca-notice-takedown/spec.md) for how takedown requests against that content are handled.

## Selection criteria

A system is a candidate for ThunderForgeVTT if, and only if, **all** of the following hold:

1. **The publisher has released an official SRD** — not a fan compilation, not an OCR of a retail book — under a license that grants redistribution and modification rights.
2. **The license is irrevocable or perpetual.** We do not build on licenses a publisher can unilaterally revise or revoke (see [OGL 1.0a](#systems-we-deliberately-do-not-ship) below for why this matters — it already happened once, industry-wide, in 2023).
3. **The license permits commercial, digital/software use** ("all media and formats now known or hereafter created," or equivalent) — not just print/PDF distribution.
4. **We can cleanly separate Licensed Material (mechanics, rules text) from Reserved Material** (proprietary lore, named characters/deities/locations, artwork, trademarks) and ship only the former.
5. **The required attribution/notice is something we can render in-app** without misrepresenting endorsement or affiliation.

A system that's mechanically interesting but fails any one of these (closed-license systems like GURPS or Cyberpunk RED, revocable/hostile community licenses like Daggerheart's, or retail/DMs-Guild content with no open grant at all) does not get ingested, however much community demand exists — see [Systems we deliberately do NOT ship](#systems-we-deliberately-do-not-ship).

## Systems we ship, and why

Each system has a working research digest, `system_<id>.json` (not committed — see the note above), which is the source of truth this table summarizes. Digests capture core mechanics, resources, action economy, character creation, units/topology, and a structured `legal` object — not full spell/monster/item reprints.

| System | Publisher | License | Why it's here |
|---|---|---|---|
| **5E System Core** (`dnd5e`) | Wizards of the Coast | CC-BY-4.0 | The single largest player base of any system on this list. WotC's January 2023 release of the SRD under Creative Commons was a direct, irrevocable response to the OGL 1.0a crisis — the most stable license available for a system this popular. |
| **Pathfinder 2e (Remaster)** (`pathfinder2e`) | Paizo Inc. | Open RPG Creative License (ORC) | The second-largest player base, and the system whose publisher (Paizo, alongside Azorius Law) *wrote* the ORC License specifically to give downstream software developers irrevocable, digital-rights-inclusive access after the OGL crisis. Registered with the U.S. Library of Congress (TX 9-307-067). |
| **Cypher System** (`cypher_system`) | Monte Cook Games | Cypher System Open License | A distinct, narrative-forward mechanical engine (three stat Pools, Effort, GM Intrusion) that's trivial to automate well and covers Numenera/The Strange's fanbase. Perpetual license; the one non-negotiable extra requirement is a "Compatible with the Cypher System" notice, which we render at system-selection time. |
| **Fate Core** (`fate_core`) | Evil Hat Productions | CC-BY-3.0 | The reference implementation of aspect/fate-point narrative mechanics, broadly used as a base for other CC-licensed hacks. Minimal-automation-burden, high design-diversity value. |
| **Blades in the Dark / Forged in the Dark** (`blades_in_the_dark`) | John Harper / One Seven Design / Evil Hat | CC-BY-3.0 | The reference engine for the entire "Forged in the Dark" ecosystem (dice-pool + position/effect + progress clocks). Covers a large, active FitD-adjacent community with one digest. |
| **Year Zero Engine** (`year_zero_engine`) | Free League Publishing | Year Zero Engine Free Tabletop License | Covers Mutant: Year Zero, Forbidden Lands, Alien RPG, Tales from the Loop, and Twilight 2000's shared mechanical chassis (d6 pools, the Push mechanic) in one digest. Irrevocable license; the one restriction (no standalone video game/NFT) is understood industry-wide, including by Free League itself, not to cover a VTT digital-utility module. |

Pathfinder 2e's digest additionally incorporates **Rage of Elements** (kineticist class) and **Character Guide: Allies and Ancestries** — both further ORC-licensed core-line rulebooks from the same publisher, added the same way Player Core 2 and Monster Core were, under the same Licensed/Reserved split.

## Attribution text (copy verbatim)

Each string below is pulled directly from `legal.attributionText` in the corresponding digest and must be reproduced **exactly** wherever this project displays attribution for that system (system-selection screen, module settings/about tab). Do not paraphrase, shorten, or merge these.

### 5E System Core (CC-BY-4.0)

> This work includes material from the System Reference Document 5.2.1 ("SRD 5.2.1") by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative Commons Attribution 4.0 International License, available at https://creativecommons.org/licenses/by/4.0/legalcode.

**Do not** use "Dungeons & Dragons" as a product/module name. Use "5E Compatible" / "5E System Core" instead.

### Pathfinder 2e (ORC License)

> Pathfinder Player Core 2 / Pathfinder Monster Core © 2024, Paizo Inc.; licensed under the ORC License located at the Library of Congress at TX 9-307-067 and available online at various locations including paizo.com/orclicense, azoralaw.com/orclicense, and others. All warranties are disclaimed as set forth therein.

Plus the verbatim ORC Notice from Player Core's own colophon (p.463) — kept in full in the local `system_pathfinder2e.json` digest's `legal.requiredNotice` field; it must be displayed prominently wherever a GM selects this system, per the ORC License's own terms. **Do not** use "Pathfinder," "Paizo," the Paizo golem logo, or "Starfinder" as a product name or in a way implying endorsement.

### Cypher System (Cypher System Open License)

> This [product] uses the Cypher System, developed by Monte Cook Games, LLC, and licensed for use under the Cypher System Open License. Compatible with the Cypher System, © Monte Cook Games, LLC. The Cypher System and its logo are trademarks of Monte Cook Games, LLC in the U.S.A. and other countries. This work contains material derived from the Cypher System Reference Document, which is copyright Monte Cook Games, LLC and available under the Cypher System Open License. This is not an official Monte Cook Games product and is not affiliated with, or endorsed by, Monte Cook Games, LLC.

**Required notice** (must appear on the system-selection/storefront screen itself, not only in settings): **"Compatible with the Cypher System"**. **Do not** use any other Monte Cook Games trademark, product name (Numenera, The Strange), or the MCG company logo.

### Fate Core (CC-BY-3.0)

> This work is based on Fate Core System and Fate Accelerated Edition, products of Evil Hat Productions, LLC, developed, authored, and edited by Leonard Balsera, Brian Engard, Jeremy Keller, Ryan Macklin, Mike Olson, Clark Valentine, Amanda Valentine, Fred Hicks, and Rob Donoghue, and licensed for our use under the Creative Commons Attribution 3.0 Unported license. This work is also based on Fate Condensed, a product of Evil Hat Productions, LLC, developed, authored, and edited by PK Sullivan, Lara Turner, Fred Hicks, Richard Bellingham, Robert Hanz, and Sophie Lagacé, and licensed for our use under the Creative Commons Attribution 3.0 Unported license.

### Blades in the Dark / Forged in the Dark (CC-BY-3.0)

> This work is based on Blades in the Dark (found at http://bladesinthedark.com/), product of One Seven Design, developed and authored by John Harper, and licensed for our use under the Creative Commons Attribution 3.0 Unported license (http://creativecommons.org/licenses/by/3.0/).

### Year Zero Engine (Free Tabletop License)

> Year Zero Engine Standard Reference Document v1.0. Authored by Tomas Härenstam. (c) 2023 Fria Ligan AB. Permission to copy, modify and distribute this text is granted solely through the Year Zero Engine Free Tabletop License. Not for resale. Permission granted to print or photocopy this document for personal use only.

**Do not** build or market this integration as a standalone video game or NFT product — it must remain framed as a VTT digital-utility extension of tabletop play.

## Systems we deliberately do NOT ship

Documented so nobody re-proposes these without a materially different license showing up:

- **Anything under OGL 1.0a** (including the older D&D 5.1 SRD) — WotC's 2023 attempt to deauthorize this exact license is *why* the CC-BY-4.0 SRD 5.2.1 exists. We standardize on the newer, irrevocable license even where an older OGL 1.0a-licensed alternative exists for the same system.
- **Daggerheart, Candela Obscura** (Darrington Press Community Gaming License) — explicitly excludes unauthorized VTT integration by name, is unilaterally revisable by the publisher (not irrevocable), and includes indemnification clauses that push legal liability onto integrators.
- **GURPS, Cyberpunk RED**, and similar fully closed systems — no open/CC/OGL/ORC release exists for their core rules at all.
- **Pathfinder 1st Edition** content — different engine than our `pathfinder2e` digest, and what exists in this space is generally older OGL 1.0a-licensed material with the same revocability concern above.
- **Any Adventure Path, campaign setting book, or organized-play scenario** — even from a publisher whose *core rulebook* license we do use (e.g. Paizo's ORC-licensed line). These are overwhelmingly Reserved Material (named NPCs, plots, specific locations) with only incidental mechanics, and are not worth the ingestion risk. This is why our Pathfinder 2e digest pulls from Player Core, Player Core 2, Monster Core, Rage of Elements, and Allies and Ancestries — all rules-focused core-line books — and explicitly not from any Adventure Path in the same product line.
- **Retail/DMs-Guild third-party PDFs of any kind** — official retail books (Xanathar's, Volo's, etc.) are simply not SRD content; DMs Guild community content carries its own separate, more restrictive license that doesn't grant the redistribution rights this project needs.

## What "in scope" content looks like

For every system above, the digest and any downstream `system.json` pack must stay on the **Licensed Material** side of the line:

- ✅ Core mechanics, dice notation, resolution rules, action economy, skill/attribute lists, resource pools, generic subsystem descriptions (how spellcasting works, how stress tracks work).
- ✅ Numerical structures (DC tables, proficiency formulas, XP thresholds) — these are the unprotectable "system" itself.
- ❌ Full verbatim reproduction of spell/monster/item lists — summarized, not reprinted, in every digest.
- ❌ Proprietary campaign lore, named NPCs/deities/locations, official artwork, or anything designated "Reserved Material" / "Product Identity" by the publisher's own license.
- ❌ The publisher's trademark used as our product/module name (see the per-system attribution sections above for the exact safe-harbor phrasing to use instead).

## User-generated content boundary

None of the above governs what a GM manually types into their own world's compendium — that's covered separately by [`specs/015-dmca-notice-takedown`](../specs/015-dmca-notice-takedown/spec.md). Short version: private, per-world GM-entered content is the GM's own responsibility; we do not pre-populate compendiums with non-SRD retail content, and we will never ship a public, cross-world content-sharing feature without the DMCA notice-and-takedown program in that spec being fully operational first.

## Proposing a new system

Want to add a system, or think one of the above needs revisiting? Please **open a GitHub issue** on [ThunderForgeVTT/ThunderForgeVTT](https://github.com/ThunderForgeVTT/ThunderForgeVTT/issues) rather than opening a PR with source content directly — licensing calls should be discussed and agreed before any extraction work happens.

When opening an issue, please include:

1. **System name and publisher.**
2. **A link to the publisher's own official SRD/license page** (not a fan mirror, not a marketplace listing).
3. **The exact license name and a link to its full text.** State whether it's irrevocable/perpetual — if you're not sure, say so; that's exactly the kind of thing this discussion is for.
4. **Whether the license explicitly covers digital/software use.** Some print-era licenses are ambiguous here; that ambiguity is a real blocker, not a technicality.
5. Anything you already know about **required attribution wording or compatibility notices** the license demands.

From there, a maintainer will confirm it clears the [selection criteria](#selection-criteria) above, and if so, the extraction follows the same pattern as the six systems already shipped: a local, uncommitted research digest (`system_<id>.json`) first, then a `packs/systems/<id>/` implementation once the digest is solid.

## Related documents

Committed to this repository:

- [`specs/016-system-pack-legal-compliance`](../specs/016-system-pack-legal-compliance/spec.md) — how the `legal` object becomes a required, validated field of the `system.json` manifest contract, and how it's rendered to GMs in-app.
- [`specs/015-dmca-notice-takedown`](../specs/015-dmca-notice-takedown/spec.md) — the notice-and-takedown process governing user-entered compendium content.
- [`docs/adrs/20260504-027-game_system_packaging_and_manifest_contract.md`](adrs/20260504-027-game_system_packaging_and_manifest_contract.md) — the manifest contract this guide's licensing requirements feed into.

Not committed (local working files — see the note near the top of this document):

- `research/vtt_open_license_game_systems.md` — the full legal research this guide distills.
- `research/system_*.json` — the working digests for each shipped system (mechanics, resources, units, and the structured `legal` object per system).
