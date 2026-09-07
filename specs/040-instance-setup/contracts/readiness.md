# Contract: Readiness, and the gate it makes possible

What this instance can and cannot do given how it is configured — and, for one
capability, a refusal rather than a warning.

## The report

```graphql
type ReadinessGap {
  settingKey: String!
  "The variable that would also set it. Null when there is no environment form."
  envVar: String
  "What to set, in a sentence. Never a value, never a fragment, never a length."
  whatToSet: String!
  "What is limited while this is unset."
  whatIsLimited: String!
}

type Capability {
  key: String!             # send_mail | publish_beyond_world | publish_terms | sync_lore | feedback
  label: String!
  available: Boolean!
  gaps: [ReadinessGap!]!
}

type SourceFlip {
  settingKey: String!
  was: SettingSource!
  now: SettingSource!
}

type InstanceReadiness {
  capabilities: [Capability!]!
  "True only when every capability is available. Not an absence of complaints."
  fullyConfigured: Boolean!
  "Rows in instance_settings the registry no longer declares."
  unrecognisedSettings: [String!]!
  "A value whose source changed since the last start — an env var that appeared or vanished."
  sourceFlips: [SourceFlip!]!
}

extend type Query {
  "Administrators only."
  instanceReadiness: InstanceReadiness!
}
```

## Rules

1. **Derived, never stored.** There is no `is_ready` column and no cache. A
   stored flag is a second source of truth for a pure function of
   configuration, and it goes stale in the direction that hurts — reporting
   ready after a credential was revoked.
2. **No secret, no fragment, no length** (FR-027). A gap names a key and a
   variable. There is no masked preview anywhere in this report, including for
   settings that *are* set.
3. **A fully configured instance says so** (FR-025 scenario 3). `available`
   and `fullyConfigured` are positive assertions, not the absence of entries.
   An empty list reads as "we did not check".
4. **`instanceRepositoryIntegration` becomes one capability here** rather than
   a separate answer with a separate vocabulary. Its
   `RegistrationProblem::guidance()` strings become that capability's
   `whatToSet`, unchanged.
5. **`sourceFlips` exists for the redeploy case.** "A container is redeployed
   with a fresh environment and an existing database" is a spec edge case: the
   value silently reverts from the environment to the stored row, and the
   operator is told rather than discovering it.
6. **An unrecognised row is reported, not deleted.** An operator who
   downgraded and upgraded again keeps their values. Same posture ADR-041 took
   for provider rows.

## The gate

FR-026, and spec 039's FR-053 which must not drift from it:

> An instance with no notice contact MUST refuse every operation that
> publishes content beyond its world, and MUST tell its administrator exactly
> what is missing. Playing privately MUST remain possible.

**Enforced server-side, at the mutation boundary, in the same place the
authorization decision for that mutation is made** — Principle III. Not in the
client; a client-side check is a warning.

Gated operations, the family ADR-069/070/071 already treat as one:

| Mutation | File |
|---|---|
| collection share creation | `graphql/mutations_collection_shares.rs` |
| actor share creation | `graphql/mutations_actor_shares.rs` |
| item share creation | `graphql/mutations_item_shares.rs` |
| ability share creation | `graphql/mutations_ability_shares.rs` |

The predicate is one function — `readiness::may_publish_beyond_world(state)` —
called by all four, so there is one place to read and one place to change.

**What is not gated**, and the spec is emphatic about it: anything inside a
world. Creating, editing, playing, inviting, rolling, sharing *within* a
world. An instance with nobody to notify is a private instance, not a broken
one.

**The refusal names what is missing**: "This instance cannot publish content
outside a world until a contact for copyright notices is set. An administrator
sets it in Admin → Readiness." It does not name the setting's value, because
there is not one.

**Reading an existing share is not gated.** Removing the notice contact must
not break links already issued — that would be a data-loss event triggered by
a configuration change. Only *creating* a new one is refused. A test asserts
this, because the obvious implementation gets it wrong.

## What is deliberately absent

- **No per-capability override or "I accept the risk" switch.** FR-026 is a
  copyright rule that spec 039 depends on being a gate. A switch would make it
  a suggestion.
- **No public readiness endpoint.** `/status` and `/readyz` already answer
  "is this service up" for anyone; this report answers "what has the operator
  not configured", which is a map of an instance's soft spots.
- **No notification when readiness changes.** It would be the first thing to
  need mail, which is the thing readiness is most often reporting as missing.
