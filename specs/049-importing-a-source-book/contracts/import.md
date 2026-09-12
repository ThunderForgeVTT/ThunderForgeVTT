# Contract: Importing, and What Crosses the Wire

**Feature**: 049 | **Phases**: 4-7 | **Spec**: FR-020 to FR-057

---

## What crosses the wire, and what does not

**The PDF never leaves the machine.** Not on review, not on submit, not ever.
It is read in the browser by `@thunderforge/pdf`, and what is sent is the
**parsed content** — some megabytes of text, not seventy of document.

This is worth stating against two existing upload paths that do **not** apply
here and should not be reached for by analogy:

- `POST /api/scenes/{id}/import/uvtt` — REST multipart, 50 MB body limit, for
  map files. Sends the file.
- `uploadCanvasImage(… file: Upload)` — GraphQL multipart, for images. Sends
  the file.

Book import sends neither. It is an ordinary GraphQL mutation with a
structured payload, because by the time anything is sent the document has
already been reduced to entries a person has reviewed and approved.

---

## Before anything is sent

| Step | Where | Requirement |
|---|---|---|
| Hash the file | browser | FR-047; spec 047 FR-072 — **before** the upload, so a wrong file is caught in a moment |
| Ask whether this account already has it | server | FR-047, against **the account's library**, not one world |
| Read the book | browser | FR-020 |
| Apply the system's content patterns | browser | FR-010 |
| Show everything found | browser | FR-021 to FR-023 |
| Wait for an explicit submit | browser | FR-024 — no timer, no default, no submit-on-close |

**Nothing above sends book content.** The hash check sends a hash, which is not
content. This is the claim `apps/web/e2e/book-import-review.spec.ts` exists to
break: it observes the network and fails if any request body carries entry text
before submit (FR-061). Proving absence by inspection is not proving it —
a telemetry call added next year would pass a code review and break the
promise.

---

## The ingest mutation

One mutation, one transaction, all or nothing (FR-032).

```graphql
createCompendiumFromImport(
  input: CreateCompendiumFromImportInput!
): GraphQLCompendium!
```

**Input** carries: the book's title, the file's SHA-256, the system it was read
as, page and silent-page counts, the parser version, whether this replaces an
existing compendium (FR-047), and the entries — each with kind, name, page,
values-with-certainty or text, and whether the reader distrusted it.

**Input does not carry origin.** Origin is written by the server (FR-051), is
not editable afterwards (FR-057), and is the field every sharing rule is
enforced against. A field the client can set is a field an attacker can set.

### What the server re-checks on arrival

A review that happened in a browser is not a permission (FR-036, Principle
III). On arrival, before anything is written:

1. **The caller is authenticated**, and is the account the compendium will
   belong to — through the new `require_account_owner` helper, because no
   account-scope equivalent of `require_world_member` exists today
   (research §11).
2. **The system exists and declares content patterns**, re-read from the
   manifest rather than trusted from the payload (FR-015).
3. **The entry count is within the stated bound** (FR-035). The browser refuses
   first; the server refuses again.
4. **Every entry's kind is one the system declared.** A kind the manifest does
   not name is a payload that was not produced by this flow.

### Application

Inside `conn.transaction(…)`, following the precedent the map importer already
sets — it inserts one row at a time inside a transaction rather than doing a
single multi-row insert, and `reconcileQueuedChanges` is the only batch
mutation in the schema and also loops.

Either the whole compendium and all its entries exist afterwards, or none of it
does. Abandonment (FR-034) is the client dropping the request; nothing partial
survives, because nothing partial was ever committed.

### Progress

Progress reflects **work sent**, not an animation (FR-030), and says what is
being sent rather than only how much (FR-031). Since the commit is one
transaction, progress is upload progress — and the contract is honest about
that: the bar reaching the end means "the server now has it and is applying
it", not "it is applied".

---

## Reading a library

```graphql
myLibrary: [GraphQLCompendium!]!        # FR-002 of spec 050, account-scoped
compendium(id: UUID!): GraphQLCompendium
compendiumEntries(compendiumId: UUID!, kind: String, after: String): …
```

Every one of these is account-scoped and goes through the same ownership
helper. **There is no world-scoped read in this spec** — a world cannot reach a
compendium until spec 050 gives it the book list (research §1).

---

## Removal

```graphql
removeCompendium(id: UUID!, confirm: Boolean!): GraphQLRemovalReport!
```

Called with `confirm: false` it **reports and changes nothing**: what is in
use, per world, and what a Game Master has edited by hand (FR-045, FR-046).
Called with `confirm: true` it removes the compendium and everything that
import contributed, and nothing else (FR-044).

Two calls rather than one because FR-045 requires naming what is in use
*before* it is confirmed, and a single destructive call with an advisory
response cannot do that.

---

## What may not leave (Phase 7)

Enforced as an **invariant at the data boundary**, not as a check on each route
(FR-054a). The routes known today — share, publish, export, collection adoption
— are not the list; they are today's list.

The crossing to be most careful about is `src/server/src/collections/` and the
`world_collections` table: a world-scoped thing being fed from an
account-scoped one, which is exactly where the two authorization helpers meet.

Every refusal names the origin as the reason (FR-053), and where the only
remaining routes are authoring it or proposing it as a system pack, says so
(FR-056a).
