# Data Model: Importing a Source Book

**Feature**: 049 | **Date**: 2026-09-12

Phase 1. What exists after a book has been read in, and what may be done to it.
Field lists are the shape the requirements demand, not a schema — column types
and indexes belong to the migration.

---

## Compendium

Everything one book produced. **Owned by an account, never by a world**
(FR-040) — research §1 records why that is true from the first migration rather
than migrated later.

| Field | Why it exists |
|---|---|
| id | |
| owner (account) | FR-040. The shelf this sits on. |
| book title | FR-041. What the Game Master calls it. |
| source hash (SHA-256 of the file) | FR-041, FR-047. The re-import check, against the **account's** library rather than one world. |
| system | FR-041. Which game system it was read as; decides which worlds may ever use it (spec 050 FR-041). |
| origin | FR-051. Always **uploaded** for anything this spec produces. Written by the server, never accepted from the client, never editable afterwards (FR-057). |
| parser version | So a re-read can tell "the book changed" from "the reader improved". |
| pages, silent pages | FR-005. A book that was a third scans must be able to say so afterwards, not only in the review. |
| counts per kind | FR-041. What the library view lists without reading every entry. |
| imported by, imported at | FR-041, and the `created_by` convention. |

**Rules.** The uploaded PDF is **not** stored — it never leaves the Game
Master's machine. Removal takes the compendium and everything it contributed,
and nothing else (FR-044), after naming what is in use (FR-045).

---

## Entry

One spell, item, creature, feature or feat.

| Field | Why it exists |
|---|---|
| id | |
| compendium | FR-043. Every entry names its bucket. |
| kind | Which content pattern found it. Open — a system declares its own kinds. |
| name | The entry's identity within the compendium, with kind (spec 050 FR-025). |
| page | FR-043. Where to look it up in the real book. |
| values | For **anchored** entries: the labelled fields, each with its certainty. Empty for prose. |
| text | For **prose** entries: what it says. |
| suspect | FR-004. The reader distrusted the text this was built from. |

**Rules.** FR-002: absence is recorded as absence. A field that was not read
carries no value — not an empty string, not a zero. FR-001b: a prose entry has
no mechanical fields at all, and the model gives it nowhere to put one.

---

## Read value

A single field of an anchored entry. Not a string — a string cannot say it is
unsure.

| State | Meaning |
|---|---|
| read | Found, with the value. |
| uncertain | Found, with the value, and the reader is not confident. Shown differently in the review (FR-023) and correctable there. |
| unread | Looked for, not found. **Carries no value.** |

This mirrors spec 048 FR-003 and the `Confidence` the existing creature reader
already produces; it is not a new idea, it is the existing one made shared.

---

## Content pattern *(declared by a system pack, not stored)*

What a system says its content looks like. Lives in the pack manifest beside
the `vision` block spec 045 established (FR-011), and is read by shared code
that names no system (FR-012).

| Field | Why it exists |
|---|---|
| kind | What this pattern finds — spell, item, creature, feat… |
| shape | **anchored** or **prose**. Decides which reader runs and what an entry may hold. |
| anchor | For anchored kinds: the label that unambiguously starts an entry of this kind. Armour class is the worked example. |
| fields | For anchored kinds: the labels to read, and what each one is. |
| name rule | How the entry's name is found relative to the anchor — the creature reader reads backwards from armour class. |

**Rules.** A system that declares nothing cannot have a book read into it
(FR-015), and is told so before any file is read. D&D 5e declares five kinds in
this spec (FR-013); Pathfinder 2e must be addable by declaration alone
(FR-014).

---

## Origin *(a property of content everywhere, not a table)*

Two values — **authored** and **uploaded** — and a third thing that is neither,
system-pack content, which the platform distributes under its own `legal` block
(FR-050b).

Recorded automatically at the point of writing (FR-051). Not editable by any
role (FR-057). Enforced as an invariant at the data boundary rather than as a
check on each route out (FR-054a), and carried by anything **derived** from
uploaded content (FR-054) — which in spec 050 means a world's change to an
uploaded entry is uploaded, while a world-only addition beside it is authored.

---

## Import record

Who imported what, when, from which file, and what it wrote. What a removal
works against, and what makes FR-047's "you already have this, imported on that
date" answerable.

Distinct from the compendium because a compendium can be re-imported: the
compendium is the current state, the records are what happened.

---

## What this spec deliberately does not model

- **A world's link to a compendium**, and **a world's delta over it**. Spec
  050. Research §1 records the decision to leave the gap rather than build a
  shape 050 would have to migrate.
- **Collections.** They exist already (spec 026); this spec only adds the
  invariant that one can never contain uploaded content.
- **The uploaded file.** Never stored, so never modelled.
