# ADR-083: Recovery Codes Are Credentials, Not Links

**Date:** 2026-09-07
**Status:** ACCEPTED
**Participants:** ThunderForgeVTT Team
**Related:** spec 041 US2 (FR-006 … FR-011), ADR-043 (share links)

---

## Problem Statement

Ten recovery codes are issued when a second factor is confirmed. They have to
be stored somehow, and this codebase already has **two** established ways to
store a secret string, which disagree:

- **Share codes** (specs 026/027) are stored **raw**. A share link has to be
  resolvable from the code alone, so the instance must be able to look one up.
- **The admin bootstrap code** is stored as an **Argon2id hash**. It is typed
  by a person once, to gain account-granting authority, and is never displayed
  again.

Picking the wrong precedent here is not a style question. Raw storage means a
reader of the database holds ten working second factors for every enrolled
account.

---

## Decision

**Recovery codes follow the bootstrap code: Argon2id, hashed, never readable
again.**

A recovery code is a single-use, human-transcribed, account-granting secret.
That is precisely what the bootstrap code is, and precisely what a share code
is not. The deciding question is not "is it a string a person types" but
**"what does holding it get you"** — a share code gets you a copy of one piece
of content its owner chose to publish; a recovery code gets you the account.

Consequences that follow from the choice rather than being decided separately:

- **They are shown exactly once**, at confirmation, and FR-009's "the instance
  cannot display them again" is a property of the storage rather than a promise
  the UI keeps. There is nothing to display.
- **They are not encrypted.** Encryption implies reading back, and nothing ever
  needs to. A decryptable store is a store with a key that can be stolen.
- **Verification has no early exit.** `consume_recovery_code` verifies against
  every unspent hash rather than stopping at the first match, so the time taken
  does not vary with *which* code was offered. That costs ten Argon2
  verifications on a path taken rarely, which is the right trade.
- **Spending is a conditional UPDATE** — `SET used_at = now() WHERE id = $1 AND
  used_at IS NULL`, zero rows meaning refuse. Two requests presenting one code
  are what a replay looks like, and exactly one may win.

---

## Alternatives Considered

**Store raw, like share codes.** Rejected on the threat, not the ergonomics: a
database backup, a stray query in a log, or a read-only replica reachable by
somebody who should not have it, would each hand over every enrolled account's
second factor. Share codes accept raw storage because what they unlock is
already published; recovery codes unlock the account.

**Encrypt with the instance secret, like OAuth tokens.** Reversible, so it
answers a question nobody asks ("what was the code?"), and the key that makes
it reversible is one more thing whose compromise is total. Encryption is right
for a token the server must *present* to somebody else; a recovery code is only
ever *checked*.

**Fewer, longer codes.** Considered. Ten is enough that losing a phone is not a
crisis and few enough that a set fits on a printed line; length is carried by
the alphabet `random_setup_code()` already uses.

---

## Consequences

**Good**

- A stolen database yields no usable second factor.
- "Shown once" needs no discipline to hold — it is arithmetic.
- One hashing implementation across passwords, the bootstrap code and recovery
  codes (`thunderforge_axum_auth_core::hashing`).

**Costs**

- A person who loses their codes **and** their authenticator cannot be helped
  by any amount of database access. That is the point, and it is why FR-024's
  operator reset exists — the answer is a defined path that clears the factor,
  never a lookup.
- Ten Argon2 verifications per recovery attempt. Deliberate: see the no-early-
  exit rule above.
