# A Link-Shared Collection Is Not a Centralized Public Repository — With One Accepted Risk

- **Date**: 2026-09-05
- **Status**: Accepted, with an explicitly accepted risk (see "The risk, accepted on the record")
- **Accepted by**: MBRound18, as accountable owner of the ThunderForgeVTT project
- **Spec**: `specs/026-content-collections/` (FR-027), `specs/015-dmca-notice-takedown/` (FR-011, FR-012)
- **Follows**: ADR-043 (content moderation and DMCA safe harbor), ADR-049 (share-link repository determination), ADR-067 (user-initiated mirror)
- **Governs**: Constitution v1.1.0, Development Workflow — DMCA / Content Moderation Guardrail

## Why this ADR exists at all

The constitution requires, before any feature is proposed that would make one
world's compendium content accessible outside that world, that design review
confirm two things: the notice-and-takedown program is operational, and an
explicit determination is on record as to whether the feature constitutes "a
centralized public repository" under spec 015's policy.

Spec 026 is that kind of feature. It bundles a world's authored content and
makes it reachable by a link.

**ADR-049 and ADR-067 both explicitly declined to cover it.** ADR-049 named
packs — what spec 026 now calls collections — as "a new concept we haven't
designed for" and later work. ADR-067 says in as many words: "Bundling authored
content for link-sharing changes the unit of distribution... This determination
is not pre-approval for it." So this document is not a formality restating a
settled position. It is the first time the question is being answered.

## Condition (a): the takedown program is operational

Confirmed, and confirmed the same way ADR-067 confirmed it. Spec 015 shipped:
designated agent published without login, intake with validation of the
statutory elements, per-entry disable, owner notification, counter-notice and
restoration after the statutory period, and a durable per-account record for
repeat-infringer evaluation. `apps/web/e2e/dmca-takedown.spec.ts` exercises it
end to end.

Spec 026 uses that machinery directly rather than reimplementing it: every
member of a collection is resolved through `moderation::effective_status` at
read and at copy time, and nothing caches the result. FR-021 through FR-025 are
that program reaching inside a set.

## Condition (b): the determination

**A link-shared collection does NOT constitute a centralized public repository
under spec 015's policy.**

The reasoning:

1. **Nothing is aggregated.** The platform never assembles one view of many
   users' content. A collection is one owner's artifacts from one world
   (FR-003), reachable by one code. There is no surface on which two owners'
   content appears together.

2. **Nothing is discoverable.** FR-020 forbids every enumeration surface in as
   many words: no query lists collections by world, by user, or globally, and
   there is no browsing, searching or counting beyond a user's own. This is
   guaranteed structurally — the schema carries no index that would serve such a
   query — which is the same discipline the spec-025 share tables state in their
   own migrations. SC-007 requires it verified by inspecting every read path
   rather than by sampling.

3. **Nothing is shared by default.** A collection is unreachable outside its
   world until its owner explicitly shares it (FR-006), and reachable then only
   by possessing an unguessable, non-time-derived code (FR-007, FR-008).

4. **The owner keeps control.** Revocation is immediate and is a first-class
   part of the delivery rather than a follow-up (FR-010, and spec 026's User
   Story 2 is P1 for exactly this reason: a build that can share before it can
   revoke has shipped the half that creates the liability without the half that
   answers it).

5. **A takedown remains fully effective.** Unlike ADR-067's mirror, everything
   here stays on the platform. A disabled member is unreachable through the
   collection and is not copied by anyone who copies it afterwards (FR-021).
   This is the property ADR-067 had to concede and this feature does not.

Under ADR-049's test, this is the same shape as the single-artifact share link
it already governs, with the unit changed from one artifact to a set of them.
It is distribution the user performs, not distribution the platform offers.

## What genuinely changed since ADR-049, stated plainly

Two things, and neither is cosmetic. A determination that did not name them
would be a determination of a different feature.

### 1. The unit of distribution is now a set

ADR-049 governed one artifact per link. A collection carries up to 100
(FR-005a), across five types including scenes with their images.

This raises the *value* of a single leaked link without changing its *kind*. One
link still reaches one owner's content, chosen by that owner, from one world.
The bound matters here: 100 is a substantial adventure module, not a library,
and it was chosen so that copying stays a single action rather than because it
was the largest defensible number.

### 2. The read path is anonymous

This is the larger change and the one worth being blunt about.

Spec 025's shipped shares are **authenticated**. `sharedAbility`, `sharedItem`
and `sharedActor` each call `authenticated_user(ctx)?` — deliberately skipping
the *membership* check, but not the session. A share link today reaches any
signed-in user. It does not reach the public.

Spec 026's FR-009a makes `sharedCollection` reachable with no account at all.
That was clarified on 2026-09-04 on the stated grounds that it matched existing
behaviour; **it did not, and the spec now records that correction rather than
carrying the false premise.** The decision was re-affirmed on its own merits:
sharing with someone who has not joined is most of the point, and what protects
the content is unguessability plus revocability, not a login wall.

**This is a real step toward "public" that no shipped feature has taken.** The
honest characterisation is that a link-shared collection is *more public than
anything the platform has shipped, and less public than a repository, because
nothing about it can be found without already holding an ~80-bit code.*

It is that gap — between "reachable by anyone who has the code" and "findable by
anyone at all" — that the determination above rests on. If a future change makes
collections findable, this determination does not survive it.

## The risk, accepted on the record

The owner was presented with three options on 2026-09-05: draft the
determination first and implement with the gate cleared; implement
infrastructure only and land the determination in parallel; or proceed fully
with the risk explicitly accepted. **The third was chosen, and the constitution
provides for exactly that: "the feature must be redesigned or the risk
explicitly accepted by an accountable owner before build work starts."**

What is being accepted:

- **That an anonymous read path is the right trade** for a feature whose purpose
  is sharing with people who have not joined, given that the code is unguessable
  and the share is revocable.
- **That a leaked code exposes up to 100 artifacts rather than one.** Bounded by
  the owner's own choice of what to include, and endable by the owner at any
  time.
- **That the platform will be briefly inconsistent.** FR-009e leaves the three
  shipped single-artifact shares authenticated, because relaxing authentication
  on live share paths is a security change deserving its own decision rather
  than one arriving as a side effect of a collections build.

What is **not** being accepted, and what would invalidate this determination:

- Any enumeration, browsing, search, or listing surface beyond a user's own
  collections (FR-020). This is the load-bearing constraint.
- Any aggregation of multiple owners' collections into one view.
- Any registry or marketplace. Spec 015 FR-011 forbids these outright and
  ADR-049 gates a registry on demonstrated demand and a fresh review.
- Removing revocation, deferring it behind the sharing half, or weakening a
  takedown's reach into a collection.

## Conditions this determination rests on

These are not decoration. If any fails to ship, this ADR needs revisiting rather
than the requirement being relaxed:

- **FR-020** — no enumeration surface, verified by inspection of every read path
  (SC-007), not by sampling.
- **FR-009c** — the anonymous read path is rate limited. An unguessable code is
  unguessable only while guessing is bounded. The limiter must not honour
  `THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT`, which the e2e harness sets on every
  run.
- **FR-009d** — an unknown code, a revoked share and a deleted collection are
  indistinguishable to an outsider, so existence cannot be probed.
- **FR-010, FR-011** — revocation ships with sharing, and states its honest limit
  at the moment of revoking.
- **FR-021 to FR-025** — a takedown reaches a member of a shared collection, and
  a reversal returns it without the owner rebuilding anything.
- **FR-026** — the person sharing is told they are responsible for having the
  right to share what is in the collection, and that a copy taken by someone else
  is theirs and cannot be recalled.
- **FR-001a, FR-001b** — content restricted to a subset of a world's members
  cannot enter a collection, and content that becomes restricted is withheld
  from that point. A collection is read by strangers, so publishing restricted
  content into one is the single failure in this feature its owner cannot undo.

## What this determination does not cover

- **A public registry, marketplace, or any cross-world browsing surface.** Still
  forbidden by spec 015 FR-011, still gated by ADR-049.
- **Versioned collections or any update path to already-copied content.** Spec
  026 places these out of scope; an update path is a genuinely new distribution
  model and re-opens this determination.
- **Collections spanning worlds.** Out of scope by FR-003.
- **Aligning the three shipped single-artifact shares to anonymous access.**
  FR-009e defers it; doing it needs its own decision.

## Consequences

- Spec 026's implementation is unblocked. `tasks.md` T001 is satisfied by this
  document.
- The risk is accepted rather than absent, and this document says which risk, on
  what date, by whom, and on what conditions.
- **The public wording has not been reviewed by a lawyer.** `legal/README.md`
  records that. A determination is not legal advice, and this one is a product
  and policy decision by the project's owner.
