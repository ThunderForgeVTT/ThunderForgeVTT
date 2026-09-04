# Contract: The Shape of an Exported File

**Feature**: `034-lore-git-sync` · **Date**: 2026-09-04

This is the contract with a **human holding a clone**, and with the platform's
own future importer. It is the only artefact of this feature that outlives the
platform's involvement, so it is specified as carefully as an API.

---

## One file per entry

`<directory>/<tree path>/<slug>.md`, where `<tree path>` mirrors the entry's
ancestors (FR-008) and `<slug>` derives from the title.

**The path is a label; the header's `id` is the key** (FR-009, R7). The system
renames files freely and identifies them never by name. FR-027 is the same rule
from the import side.

Disambiguation, when two siblings normalise to one slug or a title normalises to
nothing: deterministic, stable across runs, and recorded as a
`path_disambiguated` fidelity note so the Game Master can see why a file is not
named what they expected.

## Front matter

```markdown
---
id: 01a06d8f-5236-76d1-af6b-cd5d71dfbf7c
title: The Red Keep
tags: [location, ruined]
updated: 2026-09-04T18:15:08Z
unresolvable_links:
  - text: "Ser Willem"
    kind: actor
---

The keep has stood since...
```

- `id` — the durable entry identifier. **The only field used for matching.**
- `title`, `tags`, `updated` — FR-009's minimum.
- `unresolvable_links` — FR-013. A cross-link to an actor, item or ability
  cannot resolve in a repository that contains only lore. It stays readable in
  the body *and* is recorded here, so a round trip cannot silently drop it or
  convert it into a broken lore link.

**The body below the front matter is the entry's markdown exactly as authored**
(FR-011), with only the link rewriting FR-012 and FR-014 require. No
reformatting, no normalisation, no prettifying. SC-008's byte-identical round
trip is the test, and any cleverness here fails it.

## Links

- **To another lore entry in the same world** → a relative path to that entry's
  file (FR-012). Resolves in any markdown viewer, which is SC-011.
- **To an actor, item or ability** → left as readable text and recorded in
  `unresolvable_links` (FR-013). A declared loss, not an error.
- **To an image** → a relative path into the image directory below.

## Images

`<directory>/_images/<image id>.<ext>`, referenced relatively (FR-014).

The **uploaded original only**. Derived renditions stay on the platform; the
repository is not a rendition store. This is what makes a clone render with no
network access to the platform (SC-011).

## Commits

- **Committer**: `ThunderForge VTT <noreply@<instance domain>>`. The platform
  made this commit, on someone's behalf, and a history that claims a human ran
  `git commit` misleads whoever later has to work out where a change came from.
- **Author**: the world member who wrote the revision, under a generated
  no-reply address. Never a personal email address (FR-017).
- **Message**: names the entry and the nature of the change in language readable
  without the app open (FR-018). A restore says so (FR-019).

One commit per revision, in order (FR-016), except where FR-020's bounded
window batches a run of rapid edits — and no revision recorded in the app may be
missing from the repository's eventual content.

## What is never written

- An entry disabled by moderation (FR-015). Its absence does not block the rest
  (SC-009).
- Anything outside `<directory>`. Files the system did not write are never
  deleted or modified, forever (FR-032, SC-007).
- Per-entry permissions. They do not survive the mirror; FR-037's notice says so
  before the first run, and a `permission_not_carried` fidelity note records it.
