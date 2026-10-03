# ADR-109: An Instance States Whether It Publishes, and That Statement Decides What It Is Asked For

**Date:** 2026-10-02
**Status:** **ACCEPTED** by the accountable owner, 2026-10-02, the day it was proposed.
**Participants:** ThunderForgeVTT Team
**Related:** spec 064 (FR-001 … FR-003, FR-010, FR-011), spec 052 (FR-010, FR-011, FR-030, FR-032, FR-060), spec 035, spec 039, spec 040, ADR-043, ADR-049, ADR-103, constitution v1.2.0

---

## Problem Statement

ADR-103 decided that **the access mode decides the legal duty**: an open
instance must hold a copyright-notice contact, an invite-only or closed one is
never asked. Spec 052 states that rule in the same terms — "unless the instance
is being set up as open" (FR-010).

Building spec 064's first-run fork on that hinge did not survive contact with
the wizard. Three problems, in increasing order of seriousness:

1. **The access mode is about admission, not about reach.** It answers "who may
   get an account here". Whether material reaches somebody who was not at the
   table is a different question, and ADR-103's own Consequences section already
   records two paths that answer it differently: share links resolve for callers
   with no account at all (ADR-049, ADR-070, ADR-071), and lore synchronisation
   (spec 034) copies a world's lore to a repository the instance does not
   operate. Both are reachable from an invite-only instance. So "invite-only" was
   never a statement that nothing is published.

2. **The reverse is just as wrong.** A public instance in the sense the owner
   meant — anyone may sign up, play at their own tables — publishes nothing
   outward if nobody presses Share and lore sync is off. Obliging it on the
   strength of its sign-up page asks for a designated agent on the basis of a
   fact about registration.

3. **A derived obligation cannot be explained.** Spec 064 FR-011 and spec 052
   FR-032 both require the operator to be told *why* an answer is wanted, before
   they give it. "You are asked for a postal address because you chose `open`"
   is an inference the operator did not make and would dispute. "You said this
   instance publishes content beyond the world it was made in" is a sentence
   they wrote themselves.

## Decision

1. **An instance carries an explicit statement:
   `instance.publishes_beyond_world`**, a declared boolean setting, default
   **false**, in the `Access` group, settable at first run and in the admin area.
   It is the operator's own assertion about reach, not a derivation from
   anything.

2. **That statement — and not the access policy — decides what setup asks
   for.** `SettingDeclaration` gains a `SetupVisibility` of
   `AskedWhenPublishing`; the copyright-notice and legal-prose declarations
   carry it. `setup_asks_about` consults the resolved settings, so the step does
   not exist while the statement is false and exists while it is true. An access
   policy, by itself, obliges nothing (spec 064 FR-003): **open** without the
   statement obliges nothing, and **invite-only** with it obliges everything.

3. **The capability gate does not move.** `readiness::may_publish_beyond_world`
   keeps refusing to mint a share link while the settings behind
   `Capability::PublishBeyondWorld` are unset, with its caller list closed by
   `graphql/publishing_gate.rs`. This ADR changes **when the question is asked**,
   not what an unanswered instance may do — the same safety argument ADR-103
   made, and it is what makes a default of `false` safe: an instance that says
   nothing and then tries to publish is refused at the act, not permitted by the
   omission.

4. **Readiness reports a third state, and it is about applicability, not
   availability.** `CapabilityReport` gains `applicable: bool`. A capability the
   instance has declined is neither a gap nor a tick; it is a rung the operator
   can see from the bottom of the ladder. ADR-103 asked for this in terms of
   "not required in this mode"; the mode is replaced by the statement, and the
   field is additive so existing readers are unaffected.

5. **ADR-103 is amended, not superseded.** Its decisions 2, 4 and 5 stand as
   written. Its decision 1 — that the mode decides the duty — is narrowed to:
   *the duty is decided by the instance's statement about reach, and the mode
   change in the admin area is one of the places that statement is collected.*
   Its decision 3 (fresh installs start `closed`) is reversed in favour of
   `invite_only`, which is what spec 035's migration already seeds and what the
   owner named as the recommended private shape.

## The argument, and its limits

**Claimed**: the § 512 programme is the price of a safe harbour for material
stored at the direction of users *and made available to others*. "Made
available to others" is a fact about reach. The instance cannot compute it — a
share link that is never pressed and a lore repository that is never configured
are both outward paths that stay shut — so the accountable person states it.

**Not claimed**:

- That the statement is binding on anybody. It governs what the product asks
  for and what it refuses, not what the law says.
- That a `false` statement makes publishing impossible. It makes it *refused*
  (decision 3), which is a different and stronger thing than impossible.
- That this removes the access mode's meaning. It admits people; that is a real
  job and it keeps doing it.
- Nothing here is legal advice, and § 512 is one jurisdiction's law.

## Consequences

- **An operator can be told the truth in one sentence.** The explanation above
  the copyright-notices step names the statement the operator made, which is why
  spec 064 FR-021 (reason above the fields) is satisfiable at all.
- **A question that is not asked is said out loud.** Because absence is now a
  derivable fact rather than an accident, the wizard's rail can state it
  (spec 064 FR-010). ADR-103's hinge gave no such fact to report: "you chose
  invite-only" does not explain a missing field.
- **The two outward paths ADR-103 flagged now have somewhere to attach.** A
  future change can require the statement before lore sync is enabled or before
  the first share link is minted, rather than requiring a mode change that means
  something else. This ADR does not make that change; it makes it expressible.
- **An environment-pinned statement cannot be refused**, the same as any other
  env-fixed declaration: the product reports the unmet requirement and names the
  variable.
- **An instance that says it publishes and then leaves the prose blank keeps
  running.** `/legal/terms` serves what was answered and marks the rest
  unwritten rather than composing it, and readiness shows `publish_terms` as
  applicable-but-unavailable. Inventing an operator's terms of service would be
  worse than serving none.
- **Two statements of the fresh-install access policy had to be reconciled**
  (the migration's `CASE`, the registry's declared default). They now both say
  `invite_only`.

## Alternatives Considered

- **Keep ADR-103's hinge: the access mode decides.** Rejected for the three
  reasons above, the decisive one being that the obligation could not be
  explained to the person incurring it without asking them to accept an
  inference they would dispute.
- **Derive the statement from observed facts** — does a share link exist, is
  lore sync configured. Closer to the real risk and unpredictable for an
  operator: their obligations would change the first time somebody pressed
  Share, retroactively, with a page already published. An obligation should
  attach to a deliberate act by the accountable person.
- **Ask for the legal apparatus from everybody, as before this spec.** Honest
  and simple, and it asks a person running a game for four friends to nominate a
  copyright agent. The likely answer is a false one, and a notice address nobody
  reads is a liability wearing compliance's clothes.
- **Make the statement a three-way choice** (publishes / does not / undecided).
  Rejected as a wizard that asks a question whose honest answer is "I do not
  know yet" and then cannot act on it. `false` plus a refusal at the act of
  publishing already encodes "undecided" without a third option to explain.
