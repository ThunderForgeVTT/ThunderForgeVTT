# ADR-103: The Access Mode Decides the Legal Duty

**Date:** 2026-09-15
**Status:** **ACCEPTED** by the accountable owner, 2026-09-15, the day it was proposed.
**Participants:** ThunderForgeVTT Team
**Related:** spec 052 (decisions 1–4, FR-001 … FR-063), spec 015, spec 035, spec 039, spec 040, ADR-043, ADR-070, ADR-071, ADR-072, constitution v1.2.0

---

## Problem Statement

Spec 040's wizard asks every instance, on first run, for the name, e-mail
address and postal address of a person on whom a copyright notice may be
served. It does so because `CAPABILITIES_SETUP_ASKS_ABOUT`
(`src/server/src/auth/setup_requirements.rs:109-117`) collects
`Capability::PublishBeyondWorld`, and the three `notice.*` declarations
(`src/server/src/settings/registry.rs:309-355`) sit behind it.

For somebody installing ThunderForge to run one campaign for four friends,
that question has no answer worth giving. The instance already knows enough to
tell that case apart: spec 035 gave it an access mode — open, invite-only or
closed (`src/server/src/auth/instance_access.rs:37-45`) — and nothing reads it
except the admission gate.

The constitution's DMCA / Content Moderation Guardrail was written as though
sharing were always possible, so it offered no way to say that some instances
share nothing.

## Decision

1. **The access mode decides which legal capabilities an instance must carry.**
   An **open** instance must hold the notice contact before it becomes open;
   an **invite-only** or **closed** instance is not asked for it at setup or at
   any other time.

2. **The gate moves to the mode change, and the old gate stays.** A change to
   open does not take effect while any setting behind
   `Capability::PublishBeyondWorld` is unset. Independently of mode,
   `readiness::may_publish_beyond_world` (`src/server/src/readiness.rs:226`)
   keeps refusing to mint a new share link while those settings are unset, with
   its caller list closed by `graphql/publishing_gate.rs`. That is what makes
   the relaxation safe: it changes when the question is asked, not what an
   unanswered instance may do.

3. **New instances start closed; existing instances keep their mode.** The
   `CASE` in spec 035's migration
   (`src/server/migrations/2026-09-06-000000-0000_instance_access/up.sql:36-40`)
   keeps an upgraded instance exactly as it was. The fresh-install arm changes
   from `invite_only` to `closed`, and the three statements of the default that
   disagree today — that seed, the registry's declared `"closed"`
   (`registry.rs:848`) and the repair path's `"closed"` (`admin.rs:748-757`) —
   are reconciled.

4. **Readiness gains a third state.** A capability the current mode does not
   require is reported as *not required in this mode*, naming the mode that
   would require it — neither a gap nor a tick, so an operator can see the whole
   ladder from the bottom rung.

5. **The constitution is amended in the same change**, to v1.2.0, stating which
   instances carry the program and what the relaxation does not mean.

## The argument, and its limits

**Claimed**: the § 512 programme — a designated agent, an intake, a response
window, a counter-notice path, a repeat-infringer policy — is the price of a
safe harbour for material stored at the direction of users *and made available
to others*. An instance that admits nobody and publishes nothing outward is not
asking for that shelter.

**Not claimed**, and recorded here so that a later reader cannot mistake the
scope:

- Copyright still applies to material typed into a private world.
- A rights holder keeps every remedy; they lose a channel here, and an
  operator running without a safe harbour carries that exposure themselves.
- "Closed" is this product's word for an admission policy. As a statement about
  reach it is true only while nothing else on the instance publishes outward.
- Nothing here is legal advice, and § 512 is one jurisdiction's law.

## Consequences

- **Two outward paths do not consult the access mode.** Share links resolve for
  callers with no account at all — `sharedCollection` (ADR-070) and
  `sharedActor`, `sharedItem`, `sharedAbility` (ADR-071), per
  `src/server/src/graphql/anonymous.rs:1-12` — and lore synchronisation (spec
  034) copies a world's lore to a repository the instance does not operate. Both
  are gated on configuration today, not on mode. Spec 052 asks the owner (Q1,
  Q2) whether a non-open instance should additionally require a session to read
  a share, and whether enabling lore sync should oblige an instance the way
  going open does; this ADR is written on the current answer, which is that the
  configuration gate is the whole protection.
- **A fresh instance starts in a mode that refuses even a valid invitation.**
  That is why spec 035 chose `invite_only`. Spec 052's FR-020 covers it at the
  point it bites: issuing an invitation on a closed instance says nobody can
  redeem it yet, and offers the change.
- **An environment-pinned mode cannot be refused.** Where
  `THUNDERFORGE_INSTANCE_ACCESS_POLICY` fixes the mode, the product reports the
  unmet requirement and names the variable instead of enforcing it.
- **An open instance without a notice contact keeps running.** It is warned,
  unmissably, and cannot mint new share links. Closing it automatically would
  be an outage the operator did not choose — the same reasoning as spec 035's
  upgrade arm.

## Alternatives Considered

- **Require the programme of every instance, as today.** Honest and simple, and
  it asks a person running a game for four friends to nominate a copyright
  agent. Rejected: the question has no meaning in that case, and a question
  with no meaning is answered with noise, which is worse for a rights holder
  than no answer at all.
- **Drop the setup question and require nothing anywhere.** Rejected outright:
  it would leave an open instance publishing with no channel, which is the
  exact failure the guardrail exists to prevent.
- **Gate on whether any share link exists rather than on mode.** Closer to the
  real risk, and unpredictable for an operator: the instance's obligations
  would change the first time somebody pressed Share. Mode is a deliberate act
  by the accountable person, which is what an obligation should attach to.
