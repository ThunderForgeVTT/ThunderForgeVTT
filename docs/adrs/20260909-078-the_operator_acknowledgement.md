# ADR-078: The Operator Acknowledgement

**Date:** 2026-09-09
**Status:** ACCEPTED
**Participants:** ThunderForgeVTT Team
**Related:** spec 039 US8, ADR-076, ADR-092 (operator values in legal prose),
spec 040 (first-run setup), spec 034 (`instance_identity`)

---

## Problem Statement

Somebody takes this software, hosts it, and fills it with other people's work.
The project has **no access to their instance, no ability to take anything
down, and no standing to try.** That is not a gap to be closed — it is what
self-hosting means.

So the question is not "how do we enforce this?" It is: given that we cannot,
what is the honest thing to do?

There is a second, smaller problem underneath. A notice has to reach *somebody*,
and today the legal prose carries `[OPERATOR]` placeholders that whoever runs a
container is expected to edit in a markdown file they may never open. An
operator running an image should not have to edit source to become contactable.

---

## Decision

**An instance attests on the same record as a person, at the moment somebody
becomes an operator, and nothing pretends there is enforcement beyond that.**

### The same table, not a parallel mechanism

The acknowledgement is a row in `attestations` with `purpose = 'operator'`,
`publishable_kind` and `publishable_id` null.

FR-043 says the operator record is made "on the same terms as a sharing
attestation". Making it the same table is the only way that sentence stays true
as either side changes. A separate `operator_acknowledgements` table would be a
second implementation of one idea, and the two would drift the first time only
one of them was updated — most likely at the version-transition logic, which is
subtle and lives in exactly one place today.

It also means the archive, the version identity, the redaction rule and the
"survives the account" property (ADR-076) all apply without being restated.

### It happens at first-run setup, because that is when somebody *becomes* one

Not at install, not in a README, not on a website nobody reads. Setup is the one
moment a human being takes this on, and it is the only place this can be said to
the person it is about.

`legal/operator-responsibilities.md` is net-new prose saying plainly: the legal
obligations of everything in this instance belong to whoever operates it — the
content in it, notices filed against it, the law where it runs — and that
registering a designated agent, where the jurisdiction requires one, is the
operator's own act and is **not** performed by this software (FR-042, FR-055).

That last clause matters more than it looks. Software that implied it had
registered an agent on somebody's behalf would be worse than software that said
nothing.

### `instance_identity` is deliberately not reused

It holds one UUID for spec 034's binding records, and it has a documented hole
about database copies. Operator identity is a different thing with a different
lifetime and different consequences when it is wrong. Overloading a field
because it is nearby is how one bug becomes two features' bug.

### An instance with no notice contact boots, runs, and publishes nothing

The softest enforcement that is not merely advisory.

Refusing to boot would punish the wrong thing: a table mid-campaign whose
operator has not filled in a form cannot play, and the people harmed are the
ones with no say in it. Doing nothing at all makes FR-053 a suggestion.

Between them: the instance works, and **the paths that expose content to people
outside it stop working.** That is the exact surface the contact exists to serve,
so the sanction lands on the act that needs it.

This has a real cost in the test suite, and T005 exists because of it: with
FR-053 enforced, an instance without a notice contact refuses every existing
share test in `apps/web/e2e/`. The seed has to set one.

### Whose contact a stranger is shown

Somebody on **somebody else's** instance is pointed at *that* instance's
operator, not at this project. That is where the obligation actually sits, and
pointing at ourselves would be both untrue and a way of accepting a
responsibility we cannot discharge.

---

## Alternatives Considered

**A separate `operator_acknowledgements` table.** Rejected: one idea, two
implementations, guaranteed drift. See above.

**Acknowledge in a `LICENSE`, a README, or a website click-through.** Rejected.
None of them happens at the moment somebody becomes an operator, and none of
them is recorded. An acknowledgement nobody recorded is not evidence — the same
sentence ADR-076 is built on.

**Refuse to boot without a notice contact.** Rejected: it punishes the players
rather than the operator, and it turns a missing form into an outage.

**Do nothing, and rely on the terms of service.** Rejected: it is what exists
today, and it is why this spec exists. The prose already says the right thing;
nothing makes it real.

**Reuse `instance_identity` for the operator record.** Rejected — a different
thing with a different lifetime, and it already has a known hole.

---

## Consequences

**Good**

- A person who takes on an instance is told what they are taking on, in the
  moment they take it on, and it is on the record.
- One record type, one archive, one redaction rule, shared with the sharing
  attestation.
- The legal prose stops carrying `[OPERATOR]` placeholders somebody must edit by
  hand; setup collects the values and the pages render them (ADR-092).
- A notice reaches the operator who can act on it.

**Costs**

- Setup gains a step. It is one screen, at the one moment it is relevant.
- The e2e suite must seed a notice contact or every share test refuses (T005).
- **The project still cannot enforce anything against a self-hosted operator.**
  This decision does not change that and does not claim to. What it changes is
  that the position is stated to the person it binds, and recorded, rather than
  assumed.
