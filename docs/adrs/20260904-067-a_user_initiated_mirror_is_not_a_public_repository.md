# A User-Initiated Mirror to a Repository They Own Is Not a Centralized Public Repository

- **Date**: 2026-09-04
- **Status**: Accepted
- **Accepted by**: MBRound18, as accountable owner of the ThunderForgeVTT project
- **Spec**: `specs/034-lore-git-sync/` (FR-042), `specs/015-dmca-notice-takedown/` (FR-011, FR-012)
- **Follows**: ADR-043 (content moderation and DMCA safe harbor), ADR-049 (share-link repository determination)
- **Governs**: Constitution v1.1.0, Development Workflow — DMCA / Content Moderation Guardrail

## Why this ADR exists at all

The constitution requires that before any feature is *proposed* that would make
one world's compendium content accessible outside that world, design review
confirms two things: that the notice-and-takedown program is operational, and
that an explicit determination is on record as to whether the feature
constitutes "a centralized public repository" under spec 015's policy.

Spec 034 is exactly that kind of feature. It mirrors a world's lore to a
repository the platform does not control. So this determination is the gate its
`tasks.md` calls T001, and it is written before any of the feature's code
exists — which is the point. Writing the feature and then seeking the
determination inverts the checkpoint into a rubber stamp, and the checkpoint
exists because the platform's own legal research identifies this feature
category as its single highest-liability move.

## Condition (a): the takedown program is operational

Confirmed. Spec 015 shipped: designated agent published on a page reachable
without login, takedown intake with validation of the statutory elements,
per-entry disable, owner notification, counter-notice intake and restoration
after the statutory period, and a durable per-account record for repeat-infringer
evaluation. `apps/web/e2e/dmca-takedown.spec.ts` exercises it end to end.

## Condition (b): the determination

**A user-initiated mirror of their own world's lore to a repository they own,
using a credential they granted, does NOT constitute a centralized public
repository under spec 015's policy.**

The reasoning, which is spec 034's and is adopted here rather than restated
loosely:

1. **The user chooses to export.** Nothing is mirrored without a world owner
   connecting a repository and acknowledging the pre-synchronisation notice
   (FR-038). There is no default, and no world is mirrored by the platform's
   decision.
2. **The destination is theirs, not the platform's.** The repository is the
   user's, reached with the user's own granted credential. The platform writes
   to it on their instruction; it does not host it, serve it, or control access
   to it.
3. **There is no aggregation, no discovery, and no enumeration.** FR-039 forbids
   all three in as many words: no query lists connections beyond a user's own,
   and nothing indexes or searches across them. The platform never assembles one
   view of many users' content, which is the property that would make it a
   repository rather than a pipe.
4. **It is not a marketplace and cannot become one by increment.** Spec 034's
   scope boundary confines the feature to lore, and states that extending the
   mirror to other content types is a separate spec that re-opens this
   determination. `specs/026-content-collections/` — the shareable-bundle idea —
   is explicitly a different feature under a different, still-unwritten
   determination.

Under ADR-049's test, this is the same shape as the single-artifact share link
it already governs, with the destination changed from a recipient's world to a
repository the same user owns. It is distribution the user performs, not
distribution the platform offers.

## The cost, accepted rather than glossed

**A takedown cannot reach content that has already been mirrored.**

The platform's entire reach, once content has been written to a repository it
does not control, is:

- stop exporting the disabled entry (spec 034 FR-015, SC-009);
- deactivate the outward path entirely where excluding the item cannot stop
  republication, or where the repeat-infringer policy applies (FR-041a,
  spec 015 FR-016);
- tell the world owner that the content may already exist outside the platform's
  control and that removing it there is theirs to do (FR-040).

That is a **genuine reduction in takedown effectiveness for connected worlds**,
and this ADR accepts it rather than arguing it away.

### Amended the same day: public repositories

The first version of this determination reasoned as though a connected
repository were private. **That assumption was wrong and is corrected here
rather than quietly replaced.** Not everyone has a private repository — free
plans, shared accounts, and organisations with policies against them are all
ordinary — and a mirror to a *public* repository is a materially larger
exposure than a mirror to a private one.

It does not change the determination. The repository is still the user's, still
reached with their credential, still chosen by them; the platform still
aggregates nothing, indexes nothing and enumerates nothing. What makes something
a centralized public repository is the platform assembling one view of many
users' content, and that is absent whether an individual user's destination is
public or not.

It does change what the platform owes. Spec 034 gained FR-037a and FR-037b —
the pre-synchronisation notice must determine and state whether the repository
is public, because "everyone you invited" and "everyone on the internet" are
different sentences and a notice covering only the first is silently wrong for
the users most exposed. And it gained FR-040b: where a takedown disables content
mirrored to a public repository, the platform lodges an issue there recording
that it has disabled the content at source, stopped exporting it, and no longer
associates itself with what remains.

That issue is a **public withdrawal, not an accusation**. It names no
complainant, asserts no infringement, and reproduces no content — the platform
disabled content on receipt of a notice, it did not adjudicate one, and claiming
otherwise in public would pull it into a dispute it has no standing in rather
than out of one. Recording the withdrawal where a reader of that repository can
see it is the most the platform can do about a repository it does not control,
and FR-040c makes it deliberately all it does.

**This widens the access the feature requests** (FR-036e): opening an issue
needs more than writing contents, so FR-036's "narrowest access" is now
narrowest-that-does-the-job rather than minimal. That trade is recorded rather
than absorbed — a disassociation the product cannot perform is a commitment it
should not make, and a grant that quietly grew is how a boundary erodes.

### Why the acceptance is defensible

Three things make it so rather than convenient:

- **It is disclosed before the fact.** No synchronisation begins until the Game
  Master has acknowledged a notice explaining what leaves the platform
  (FR-037, FR-038), including that per-entry lore permissions do not survive the
  mirror.
- **It is disclosed publicly.** `legal/dmca-policy.md` carries a section stating
  where the platform's reach ends: what it will do, what it has no authority to
  do, that content a user exported was published by that user, and that a rights
  holder should direct a notice about a third-party service to that service's
  provider. Spec 015 FR-015 through FR-018 require it to stay there.
- **The reduction is bounded by the user's own act.** It applies only to worlds
  whose owner chose to connect a repository, and only to what was mirrored
  before the notice arrived.
- **Where the exposure is public, the withdrawal is public too.** FR-040b puts
  the platform's disassociation in the same place as the exposure, which is the
  only forum where it is worth anything.

## What this determination does not cover

- **Any other content type.** Actors, items, abilities, scenes and packs are out
  of scope for spec 034. Extending the mirror to them re-opens this.
- **`specs/026-content-collections/`.** Bundling authored content for
  link-sharing changes the unit of distribution, which spec 026 already records
  as requiring its own FR-012 review. This determination is not pre-approval for
  it.
- **Any public registry, marketplace, or cross-world browsing surface.** Spec
  015 FR-011 still forbids these outright, and ADR-049 records a registry as a
  future consideration gated on demonstrated demand and a fresh review.

## Consequences

- Spec 034's implementation is unblocked. `tasks.md` T001 is satisfied by this
  document.
- `specs/034-lore-git-sync/checklists/requirements.md` reaches 29/29.
- The disclosure obligations above are not optional decoration of the feature —
  they are the conditions this determination rests on. FR-037, FR-037a, FR-037b,
  FR-038, FR-039, FR-040, FR-040b and FR-041a failing to ship would invalidate
  the reasoning here, and this ADR would need revisiting rather than the
  requirements being relaxed.
- **The public wording has not been reviewed by a lawyer.** `legal/README.md`
  records that, and the review it asks for covers the text this determination
  depends on. A determination is not legal advice, and this one is a product and
  policy decision by the project's owner.
