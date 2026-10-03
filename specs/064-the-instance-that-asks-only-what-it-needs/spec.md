# Feature Specification: The Instance That Asks Only What It Needs

**Feature Branch**: `064-the-instance-that-asks-only-what-it-needs`

**Created**: 2026-10-02

**Status**: Implemented, pending the first-run lane's green run

**Depends on**: [spec 052, What an Access Mode Obliges](../052-access-mode-and-legal-duty/spec.md)

**Input**: Project owner, 2026-10-02: "now i want a test that does the full
instantiation of a new thunderforge instance like if the user signifies its a
private instance we shouyldnt force them to eneter all the legal stuff if they
assert its a public instance we should be explaining why they need that info.
do a test split at this point one does private one does public, private should
be basically smooth saioling letting them setup like OIDC or oauth2 providers
and then getting the instance online where as public is all the legal stuff
first thennnnnnn all the OIDC nad oauth2 stuff and the rest of the sign up. and
i want qa dedicated wizard that is required before the app is usable think
software sign up so its screeened and easy to follow" — then: "if possible this
is also a good pooint where we let the user input anything they forgot like s3
connection info or anything haha great spot for it" — then, on what private
should mean: "let the user decide but io thiknk invbite only with a unique link
that lets them sign up or sign in with a configured oidc / oauth provider would
be awesome".

## What this spec is for, given that 052 exists

Spec 052 states the rule: an instance should not be asked for a legal apparatus
it has nobody to be legal towards. It has `spec.md` and no plan, and it states
the rule in terms of the **access policy** — "unless the instance is being set
up as open" (FR-010).

This spec takes 052 as a dependency and does three things 052 does not:

1. **It moves the hinge.** What an instance owes is decided by an explicit
   statement that it **publishes content beyond the world that content was made
   in** — a new setting, `instance.publishes_beyond_world` — and not by its
   access policy. See [ADR: an instance states whether it
   publishes](../../docs/adrs/20261002-109-an_instance_states_whether_it_publishes.md)
   for why, which is the one genuinely architectural decision here.
2. **It specifies the wizard's shape**, which 052 does not touch: step order,
   the explanation that precedes each duty, provider configuration, and the
   final step for whatever the operator meant to set.
3. **It finishes the job the wizard left half-done.** Before this spec an
   operator was never asked about access policy, sign-in providers or object
   storage, so a wizard that gates the whole application handed back an
   instance that might be unable to store an image or admit a second person.

## The problem, in one operator's afternoon

Somebody installs ThunderForge for one campaign and four friends. During
first-run setup they are asked for the name, the e-mail address and the postal
address of a person on whom a copyright notice may be served, because
`CAPABILITIES_SETUP_ASKS_ABOUT` listed `Capability::PublishBeyondWorld`
unconditionally. They owe that to nobody. The likely outcome is not that they
abandon the install — it is that they type something false into a field whose
whole purpose is to be true, which is worse than not asking: a notice address
nobody reads is a liability wearing compliance's clothes.

They are then **not** asked who may join, whether anybody can sign in with an
account they already have, or where uploads are kept. They reach `/admin` with
a configured instance that cannot store a map.

## Requirements

Requirements this spec satisfies rather than restates: spec 052's **FR-010**
(no copyright-notice contact required of an instance that does not publish),
**FR-011** (a capability this mode does not require is reported as a third
state, not as a gap), **FR-030** (a capability does not take effect until every
setting behind it holds a value), **FR-032** (the duties are stated before they
are confirmed) and **FR-060** (an end-to-end test proves setup asks for no
copyright-notice detail). 052's **FR-061** stays 052's own: it governs the
*switch to open* in the admin area, which is a different path from first run.

**The statement that decides everything**

- **FR-001**: The instance MUST carry an explicit statement of whether it
  publishes content beyond the world that content was made in, settable at
  first run, defaulting to **false**.
- **FR-002**: What setup asks for MUST be derived from that statement. A
  declaration marked `AskedWhenPublishing` MUST NOT appear in setup while the
  statement is false, and MUST appear while it is true.
- **FR-003**: An access policy MUST NOT, by itself, oblige anything. Choosing
  **open** without saying this instance publishes beyond a world obliges
  nothing; saying it publishes while **invite-only** obliges everything.

**Saying what is not being asked**

- **FR-010**: A question this instance is not being asked MUST be said out loud
  rather than being merely absent. An operator who has read that ThunderForge
  asks for a copyright-notice contact and is never asked for one has no way to
  tell "it decided I do not owe this" from "it forgot", and the second reading
  sends them looking for a setting that is not there.
- **FR-011**: Before the statement is saved, the wizard MUST name what checking
  it will add — 052 FR-032 applied to first run rather than to the mode switch.

**The wizard's shape**

- **FR-020**: Step order MUST be declaration order, so that the legal steps
  precede providers and the optional extras without a sort being introduced:
  "all the legal stuff first, thennnnnnn all the OIDC and oauth2 stuff".
- **FR-021**: Each step whose answers exist for a reason the operator cannot be
  expected to infer MUST state that reason **above** the fields, not as a hint
  beneath them. A hint under a field is read after the question has already
  landed as an imposition.
- **FR-022**: Each access policy MUST carry its consequence, and **invite-only**
  MUST be marked as the recommendation — the operator's own preferred shape for
  a private table, and the one that needs no further decisions.
- **FR-023**: Settings an instance may want but does not need MUST be collected
  on **one** final step, not one step per group. Four more mandatory-looking
  screens is how a wizard stops being read. That step MUST say that everything
  on it can be changed later in the admin area.

**Signing in with an account they already have**

- **FR-030**: Setup MUST offer to configure the seeded sign-in providers and
  enable them, and MUST never require one: an instance with local accounts
  alone is a supported instance.
- **FR-031**: Where the policy is invite-only, the providers step MUST say that
  an invite link admits its holder **either** by creating an account **or** by
  signing in with any provider enabled there — which `oauth.rs` already
  implements, and which no operator could be expected to guess.
- **FR-032**: A provider whose endpoints are derived from an issuer MUST be
  configurable from the issuer alone. Before this spec the seeded Keycloak row
  carried empty endpoint strings and no field could fill them, so generic OIDC
  was unreachable through the product.
- **FR-033**: An issuer URL MUST pass the same outbound-URL guard as any other
  operator-supplied URL, and more strictly in consequence: three addresses this
  server will talk to are derived from it, one of them with an access token
  attached.
- **FR-034**: A provider that publishes its own endpoints MUST **refuse** an
  issuer URL with a message saying so, rather than silently ignoring it.

**Where uploads are kept**

- **FR-040**: The object store's connection MUST be declared as settings, so it
  is answerable in the wizard. The environment MUST still win, as it does for
  every other declared setting.
- **FR-041**: No shipped default may be a credential. The bundled stack's
  development fallback stays in the storage client, where it is a convenience
  rather than a declared setting an operator could mistake for configuration.
- **FR-042**: The wizard MUST be able to **test** the connection and report
  what it found, before the operator leaves the step. Finding out now is the
  difference between a configuration error and a lost map.

**Proof**

- **FR-060**: An end-to-end test MUST walk a private instance from an empty
  database to a usable one: invite-only, publishing nothing, never asked for a
  notice contact or a jurisdiction, with a provider configured from an issuer
  and storage proved reachable — and MUST then prove the payoff by handing a
  second person a link they redeem **through the provider**.
- **FR-061**: An end-to-end test MUST walk a public instance: told what
  publishing adds before it is saved, asked for the notice contact with the
  reason above it, refused a reserved-domain notice address, and told at the
  review what it left unanswered — and MUST then read `/legal/dmca` and
  `/legal/terms` as a stranger, finding the answers given and the unwritten
  prose marked unwritten rather than composed.
- **FR-062**: Each first-run test MUST run on an instance of its own. Setup
  completes once; a second walk on the same stack finds `/setup` redirecting
  and fails with something that looks nothing like its subject.

## Not in scope

- **OIDC `.well-known` discovery.** There is none anywhere today, by deliberate
  design; an issuer URL plus per-kind derivation is the existing contract.
- **A create/delete mutation for providers.** All four kinds are seeded rows; a
  fifth provider is a `ProviderKind` variant, which is a code change by design.
- **Spec 052's US3** (instances already open when this rule arrives) **and
  US5** (a non-open instance publishing nothing), beyond what the two tests
  assert in passing.
