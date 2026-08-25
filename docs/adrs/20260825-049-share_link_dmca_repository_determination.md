# ADR-049: Share Links Are Not a Centralized Public Repository (DMCA Guardrail Determination)

**Date**: 2026-08-25

**Status**: Proposed — requires acceptance by an accountable owner before it takes effect

**Supersedes/Amends**: none. Satisfies the review requirement in spec
`015-dmca-notice-takedown` FR-012 and Constitution v1.1.0's "DMCA / Content
Moderation Guardrail".

## Problem Statement

Constitution v1.1.0 requires that, **before implementation begins**, any feature
making one world's compendium content "visible, copyable, searchable, or
otherwise accessible outside that world" must have on record:

- **(a)** confirmation that the notice-and-takedown program is operational, and
- **(b)** an explicit determination of whether the feature constitutes "a
  centralized public repository" for user-shared, potentially-copyrighted
  content — and if so, redesign or explicit risk acceptance by an accountable
  owner.

Spec `025-world-abilities-compendium`'s User Story 6 (ability share links with
Copy-to-World) triggers this. Planning that feature surfaced that **no such
determination exists for any share-link feature**, though two have already
shipped.

### What planning found

| Feature | Spec | Ships share links? | Determination on record? |
|---|---|---|---|
| Actor share links | 010, FR-023 | Yes, with Copy-to-World | **No** |
| Item share links | 013, FR-022..FR-027 | Yes, with Copy-to-World | **No** |
| Ability share links | 025, US6 | Proposed | **No** — this ADR |

Spec 013's own Constitution Check does not mention the guardrail at all.

**Root cause identified**: spec 015's Assumptions section states — "The platform
currently has no public compendium-sharing, marketplace, or cross-world
content-browsing feature (confirmed by repository search); this spec's guardrail
requirements (FR-011, FR-012, User Story 4) are **preventative**, to be enforced
before any such feature is proposed, **not a retrofit of an existing one**."

That assumption was **factually incorrect when written**. Actor share links had
already shipped (spec 010), and item share links were being built the same day
(specs 013 and 015 were both created 2026-08-23, the same day the constitution
was amended to v1.1.0). The guardrail was therefore authored as purely forward-
looking and was never applied to the sharing that already existed. This is a
same-day-amendment miss, not a deliberate exemption.

## Prerequisite (a): is the takedown program operational?

**Yes.** Spec 015 is complete — 41 of 41 tasks, zero unchecked. Verified
operational capabilities relevant to this determination:

- Per-entry disable/restore at individual compendium-entry granularity (FR-010),
  implemented as `ModerationEntityType` with `moderation::effective_status` and
  `moderation::filter_visible`.
- Notice intake, counter-notice, and restoration flow (FR-001..FR-007).
- Durable per-account infringement records and repeat-infringer tracking
  (FR-008, FR-009).
- Existing regression coverage that a share link stops resolving once its target
  is moderated (`shared_item_is_unavailable_once_moderation_disabled`).

## Determination (b)

### What the policy actually prohibits

Spec 015's originating description: *"we must never host a centralized public
repository where users can **freely share** their custom copyrighted compendiums
with **other users/worlds** (private, per-world compendium data entered by a GM
for their own game is a different, lower-risk case than a public
sharing/marketplace feature)."*

FR-011 prohibits a feature "that functions as a centralized public repository
allowing users to freely share their custom compendium content with other users
or worlds." Spec 015's own Assumptions add the distinguishing test: per-world
compendium data "is not itself a 'centralized public repository' **as long as it
remains scoped to that world's own members**."

Reading the three words as the policy uses them:

- **Centralized** — content is aggregated into a common store or index that
  exists independently of any one world.
- **Public** — content is reachable by users generally, not only by a specific
  recipient the owner chose.
- **Repository** — the collection is browsable, searchable, or enumerable.

### How share links measure against that test

| Property | Marketplace / public repository | ThunderForge share links |
|---|---|---|
| Aggregated index of shared content | Yes | **No** — no table, query, or view aggregates shares across worlds |
| Discoverable without the owner's action | Yes | **No** — reachable only by possessing an unguessable 20-char code |
| Browsable / searchable | Yes | **No** — no search, no listing, no directory |
| Enumerable | Yes | **No** — no "my shares" or "shares in this world" query exists |
| Owner controls the audience | No | **Yes** — the owner creates the link and chooses who receives it |
| Revocable by the owner | Typically no | **Yes** — soft `revoked` flag, immediate effect |
| Subject to takedown | Varies | **Yes** — a moderated entity's share stops resolving |
| Copy semantics | Ongoing distribution | **One-time deep copy**, no live link back |

A share link is closer to sending someone a file than to publishing to a
storefront. The recipient must be given the code by the owner; nothing about the
system helps a stranger find it.

### Decision

**Share links, as currently designed and constrained below, do NOT constitute a
centralized public repository under spec 015's policy.**

This determination covers all three share-link features — actor (spec 010), item
(spec 013), and ability (spec 025 US6) — since they are the same mechanism.

**It is conditional on the following invariants**, which are what keep the
feature on this side of the line. Each is currently true; violating any one
re-opens this determination:

1. **No enumeration.** No query returns share links by world, by user, or in
   aggregate. A share is reachable only by possessing its code.
2. **No discovery surface.** No search, index, directory, browse view, or
   recommendation over shared content.
3. **Unguessable codes.** Derived from a v4 UUID (never v7 — v7 front-loads a
   timestamp, which both narrows the search space and leaks creation time).
4. **Owner-controlled and revocable.** Only Owner-level access can create a
   link; the creator or a DM can revoke it, and a revoked link resolves to a
   distinct "no longer available" state.
5. **Takedown-effective.** A moderated entity's share link must stop resolving.
   Share endpoints must never become a moderation bypass.
6. **Copy is a one-time deep copy** producing an independent record with an
   empty ownership block and no referential link back to the source.

### Risk accepted

Residual risk, explicitly acknowledged rather than designed away: a determined
user can still redistribute infringing content by pasting a share code publicly.
This is the same residual risk carried by any unlisted-URL sharing mechanism.
It is mitigated — not eliminated — by revocation, per-entry takedown, and
repeat-infringer tracking, all operational per prerequisite (a). The platform
does not aggregate, index, promote, or profit from shared content, which is what
the safe-harbor posture depends on.

**Accountable owner**: _(to be recorded on acceptance)_

## Consequences

- Spec 025's User Story 6 is **unblocked** and may be implemented, subject to the
  six invariants above.
- Actor share links (spec 010) and item share links (spec 013) are **covered
  retroactively** by this same determination. No redesign is required for either;
  both already satisfy the invariants.
- **Spec 015's Assumptions section is factually wrong** and should be corrected:
  the guardrail was not purely preventative, because sharing already existed when
  it was written.
- Any future feature that would add an index, a browse view, a search over shared
  content, a "recently shared" surface, or any enumeration of share links
  **re-opens this determination** and requires a fresh FR-012 review before
  implementation. The invariants above are the tripwire.
- New content types gaining share links (a future lore-entry share, say) are
  covered by this ADR **provided** they satisfy all six invariants; they do not
  each need a fresh determination, but they do need to be checked against the
  list.

## Alternatives Considered

- **Redesign share links to keep content inside its world** — rejected. It would
  remove a shipped, useful capability (specs 010 and 013) to mitigate a risk the
  invariants already contain, and would not meaningfully reduce redistribution
  risk, since a determined user can copy content by hand regardless.
- **Treat each share-link feature as needing its own determination** — rejected
  as pure process overhead. They are one mechanism; a single determination with
  explicit invariants is more maintainable and harder to let drift than three
  near-identical documents.
- **Defer the determination and ship spec 025 without User Story 6** — viable,
  and was the original plan. Rejected because it leaves the pre-existing actor
  and item share gap unaddressed indefinitely, which is the larger exposure:
  those are already in production without a determination.
