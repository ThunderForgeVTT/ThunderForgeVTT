# Feature Specification: What an Access Mode Obliges

**Feature Branch**: `052-access-mode-and-legal-duty`

**Created**: 2026-09-15

**Status**: Draft

**Input**: Project owner, walking the admin portal on 2026-09-15: "On a closed
instance, the user doesn't need to provide DMCA takedown information, but only
if it's a closed instance. Instances should start closed and then go to
invite-only or open. On invite-only or closed there's no real sharing, so no
DMCA takedown is needed; only on an open instance is there real sharing."

## The problem

Every instance is asked for the same legal apparatus, whether or not it has
anybody to be legal towards.

A person who installs ThunderForge to run one campaign for four friends is
asked, during first-run setup, for the name, the e-mail address and the postal
address of a person on whom a copyright notice may be served. That is what
`CAPABILITIES_SETUP_ASKS_ABOUT` does
(`src/server/src/auth/setup_requirements.rs:109-117`): the wizard collects
`Capability::PublishBeyondWorld` alongside mail and terms, so the three
`notice.*` declarations (`src/server/src/settings/registry.rs:309-355`) are
questions on first run. For a private table, that question has no answer worth
giving and no reader who will ever ask it.

The instance already knows enough to tell the difference. Spec 035 gave it an
admission policy — **open**, **invite-only** or **closed**
(`src/server/src/auth/instance_access.rs:37-45`) — chosen at `/admin/access`,
held in a singleton row, changed only through `setInstanceAccessPolicy`
(`src/server/src/graphql/mutations_instance_access.rs:341`), which writes the
change and its audit event in one transaction
(`src/server/src/admin.rs:770-810`). Nothing reads that policy to decide what
the instance is obliged to carry. It decides exactly one thing: whether a new
account may be created (`src/server/src/auth/registration.rs`'s
`ensure_admission_allowed`, the single gate documented at
`src/server/src/auth/instance_access.rs:1-27`).

This spec makes the access mode decide the second thing too: **which legal
capabilities an instance must carry before it may operate in that mode.**

## Two claims the owner made, and what the code says

### "Instances should start closed" — they do not

They start **invite-only**, and an upgraded instance is opened.

```sql
-- src/server/migrations/2026-09-06-000000-0000_instance_access/up.sql:36-40
INSERT INTO instance_access_settings (id, access_policy)
VALUES (1, CASE WHEN EXISTS (SELECT 1 FROM users) THEN 'open' ELSE 'invite_only' END);
```

Spec 035 chose `invite_only` for a fresh install on purpose, and recorded the
reason in the migration: `closed` refuses even a valid invitation, so an
operator whose first act is to invite somebody would watch that invitation
fail. The `CASE` arm for an instance that already has users is the other half —
an upgrade must not close a running community.

Two other places disagree with the migration and with each other. The registry
declares the setting's default as `"closed"`
(`src/server/src/settings/registry.rs:848`), which is what an operator reads in
Admin → Instance. The repair path, which creates the row if it is somehow
missing, also writes `closed` and says why: "the safe way to be wrong about an
anomaly is to admit nobody" (`src/server/src/admin.rs:748-757`). So the product
holds three answers to "what does an instance start as": `invite_only` if it is
new, `open` if it was upgraded, and `closed` on the two paths that state a
default in code.

This spec settles it as the owner asked — **new instances start closed** — and
keeps spec 035's protection for the upgrade case unchanged. The confusing part
of that, an invitation that cannot be redeemed on a closed instance, is dealt
with in FR-020 rather than by picking a different starting mode.

### "On closed there's no real sharing" — mostly true, and not entirely

The access mode governs **who may create an account**. It does not govern who
may read content, and today three outward paths do not consult it at all:

1. **Share links resolve without a session.** `sharedCollection` (ADR-070) and
   `sharedActor`, `sharedItem`, `sharedAbility` (ADR-071) each answer a caller
   with no account at all; the module that supplies their rate-limit identity
   says so in its first sentence (`src/server/src/graphql/anonymous.rs:1-12`),
   and the share modules' own refusal constant exists because "the caller need
   not have an account" (`src/server/src/graphql/mutations_actor_shares.rs:47-54`).
   A closed instance can mint a link today that anybody on the internet can
   open.
2. **Lore synchronisation publishes to a repository the instance does not
   operate** (spec 034, `src/server/src/lore_sync/`). Spec 015's FR-015 to
   FR-018 exist precisely because that content leaves.
3. **Scene audio and tab sharing** (spec 038) carry a session's media to the
   people at the table, which stays inside the world, and is named here only so
   that the list is the whole list.

Paths 1 and 2 are already gated — but on configuration, not on mode.
`readiness::may_publish_beyond_world` (`src/server/src/readiness.rs:226-231`)
refuses to mint any new share link while the `notice.*` settings are unset, and
four mutations call it (`mutations_actor_shares.rs:215`,
`mutations_item_shares.rs:212`, `mutations_ability_shares.rs:259`,
`mutations_collection_shares.rs:180`), with a surface test closing the list
(`src/server/src/graphql/publishing_gate.rs`). So an instance that never
answered the copyright-contact question already cannot create a share link,
in any mode, today.

That is the honest shape of the owner's decision, and this spec is written to
it: **not asking at setup is safe because the gate at the point of use already
holds.** What changes is when the question is asked, not whether anything is
allowed without an answer.

## The legal reasoning, stated plainly

The argument for relaxing this on a closed or invite-only instance is narrow,
and it is worth writing down exactly, because a wider version of it would be
wrong.

**What is claimed**: the notice-and-takedown *programme* of 17 U.S.C. § 512 —
a designated agent published where a rights holder can find them, an intake, a
response window, a counter-notice path and a repeat-infringer policy — is the
price of a **safe harbour for material stored at the direction of users and
made available to others**. An instance that admits nobody and publishes
nothing outside itself is not asking for that shelter, because there is nothing
for a stranger to find and nobody to serve a notice about. A group of four
friends typing a subclass into their own world is in the same position as a
group of four friends typing it into a shared document.

**What is not claimed**, and must never be read into this spec:

- **Copyright still applies.** Material copied from a retail rulebook into a
  private world is no more licensed than it was before. This spec changes what
  the *operator* must publish and staff, not what anybody may copy.
- **A rights holder is not disarmed.** They keep every remedy they had; they
  simply have no takedown channel here, and an operator with no safe harbour
  carries the consequence of that themselves. This is why the mode change in
  FR-030 must *say so* before it is made, and why FR-011's relaxation is
  reported as a choice the operator made rather than a box the product ticked.
- **"Closed" is not a legal category.** It is this product's word for an
  admission policy, and it is true only while nothing else on the instance
  publishes outward. That is why FR-012 keeps the publishing gate in force in
  every mode, and why Q1 and Q2 below are asked rather than assumed.
- **Jurisdiction is not settled by this spec.** § 512 is United States law;
  other regimes attach their own duties to hosting. `operator.jurisdiction`
  already exists as a declared setting (`registry.rs:288-300`), and this spec
  neither reads it nor claims that any particular jurisdiction's obligations
  end at a closed door.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A private instance is set up without a legal department (Priority: P1)

Somebody installs ThunderForge for their own table. Setup asks them for the
things that make the instance work and does not ask them to nominate a person
on whom copyright notices may be served. The instance finishes setup closed:
nobody can create an account on it until its operator decides otherwise.

**Why this priority**: It is the owner's decision, and it is the case that
makes the product installable by a person rather than an organisation.

**Independent Test**: Run first-run setup on an empty database. No step asks
for a copyright-notice contact. Setup completes, and `/admin/access` shows the
instance as closed.

**Acceptance Scenarios**:

1. **Given** an empty instance, **When** an operator runs setup, **Then** no
   step asks for a copyright-notice name, address or e-mail, and setup
   completes without one.
2. **Given** setup has completed on a new instance, **Then** its access mode is
   closed.
3. **Given** a closed instance, **When** anybody tries to create an account by
   any route — the registration form, an identity provider, or a valid
   invitation — **Then** they are refused, as today.
4. **Given** a closed instance, **When** an operator opens the readiness screen,
   **Then** the copyright-notice contact is reported as *not required in this
   mode* rather than as a gap, and the screen says which mode would require it.

---

### User Story 2 - Opening an instance asks for what opening obliges (Priority: P1)

An operator decides to let strangers sign up. Before the switch takes effect,
the product tells them what an open instance obliges — a contact a rights
holder can serve a notice on, published where somebody without an account can
read it — and collects it. Only then does the instance become open.

**Why this priority**: It is the whole of the guardrail. A relaxation with no
gate on the way back up is not a relaxation, it is a hole.

**Independent Test**: On a closed instance with no `notice.*` values, try to
switch to open. It is refused, and the refusal names every missing detail.
Supply them, switch again, and the instance becomes open.

**Acceptance Scenarios**:

1. **Given** an instance that is not open and has no copyright-notice contact,
   **When** an operator switches it to open, **Then** the switch does not take
   effect and they are told which details are missing and where to set them.
2. **Given** the same instance, **When** the operator supplies every missing
   detail and switches again, **Then** the instance becomes open and accounts
   can be created.
3. **Given** an operator about to switch to open, **Then** they are shown, in
   plain language and before they confirm, what an open instance commits them
   to and that it cannot be satisfied retroactively.
4. **Given** an open instance, **When** an operator switches it to invite-only
   or closed, **Then** the switch is never refused for a missing legal detail,
   and no stored detail is deleted by the change.
5. **Given** any mode change, **Then** it is recorded with who made it, when,
   and the previous and new modes.

---

### User Story 3 - An instance that is already open is not broken by this (Priority: P2)

An instance running in the open, upgraded to a build that has this rule, keeps
running. If its copyright-notice contact is incomplete, its operator is told
prominently and repeatedly; nothing closes the instance and nobody's table goes
dark.

**Why this priority**: The alternative — a migration that closes a running
community, or one that silently lets it stay open with no notice channel — is
either a self-inflicted outage or the exact failure the guardrail exists to
prevent. It is second only because it affects instances that already exist.

**Independent Test**: Take an open instance with the `notice.*` settings unset,
upgrade it, and confirm that it is still open, that its operators see an
unmissable warning, that nobody loses access, and that no new share link can be
created.

**Acceptance Scenarios**:

1. **Given** an open instance with an incomplete copyright-notice contact,
   **When** it is upgraded to a build carrying this rule, **Then** its access
   mode is unchanged and every existing account keeps working.
2. **Given** that instance, **When** an operator opens the admin portal,
   **Then** they see an unmissable statement that the instance is open without
   a notice contact, what that means, and the two ways out: supply the contact,
   or leave open mode.
3. **Given** that instance, **When** anybody tries to create a new share link,
   **Then** it is refused exactly as it is today, naming what is missing.
4. **Given** that instance, **When** its operator supplies the contact,
   **Then** the warning clears without a restart.

---

### User Story 4 - The written rules match the built rule (Priority: P2)

The constitution's DMCA guardrail assumes sharing is always possible. After
this feature, it says what an access mode obliges, an ADR records why, and
spec 015 carries a pointer so nobody reads its FR-001 as unconditional.

**Why this priority**: The guardrail is a review checkpoint that people apply
by reading it. A checkpoint whose text contradicts the product is worse than
none, because it is applied confidently and wrongly.

**Independent Test**: Read the constitution's Development Workflow section and
spec 015's FR-001; both state the access-mode condition, and the ADR names the
decision, its date and its limits.

**Acceptance Scenarios**:

1. **Given** the constitution, **When** it is read at a design review, **Then**
   its DMCA guardrail states which legal capabilities apply in which access
   mode, with a version bump and an updated Sync Impact Report.
2. **Given** `docs/adrs/`, **Then** an ADR records the decision, the narrow
   legal argument behind it, and the paths that publish outward regardless of
   mode.
3. **Given** spec 015, **Then** its requirements are annotated where this spec
   conditions them, and nothing in this spec removes one of them.

---

### Edge Cases

- **An invitation on a closed instance.** Closed refuses a valid invitation
  (`instance_access.rs:41-45`), which is spec 035's reason for not starting a
  fresh instance closed. With FR-002, a new instance starts closed, so the
  first invitation an operator issues would fail. FR-020 covers it: issuing an
  invitation on a closed instance tells the operator, at the moment of issuing,
  that nobody can redeem it until the instance is at least invite-only, and
  offers that change in the same step.
- **The last operator locks themselves out.** Closed refuses account creation;
  it does not refuse sign-in. An existing operator can always sign in and
  change the mode.
- **A mode set by environment variable.** `THUNDERFORGE_INSTANCE_ACCESS_POLICY`
  is declared (`registry.rs:843`), and an environment-fixed value is not
  writable from the admin screen. An instance pinned to `open` by its
  environment cannot be talked out of it by a refusal in the product; the
  requirement it cannot enforce, it must report (FR-031).
- **Details removed after opening.** Clearing `notice.contact_email` on an open
  instance leaves it open with no channel. It is the same state as US3 and gets
  the same treatment: an unmissable warning, existing links untouched (that is
  deliberate in `readiness.rs:220-225`), and no new link created.
- **An unrecognised mode in the database.** Parsed as closed
  (`instance_access.rs:63-69`). Under this spec that also means no legal
  capability is required, which is the safe direction: an instance that admits
  nobody publishes nothing new.
- **A closed instance with existing share links.** Links minted before the mode
  changed keep resolving. Revoking them is an operator action, not a
  consequence of a mode change, for the same reason `may_publish_beyond_world`
  is not consulted on read: a configuration change must not become a
  data-loss event.

## Requirements *(mandatory)*

### Functional Requirements

**The mode an instance starts in**

- **FR-001**: The instance MUST keep exactly the three access modes it has
  today — open, invite-only and closed — with the meaning they have today for
  admission.
- **FR-002**: A new instance MUST start closed, and setup MUST show the
  operator the mode it will finish in and let them change it there.
- **FR-003**: An instance that already exists MUST keep the access mode it has.
  No migration or upgrade may change a running instance's mode, in either
  direction.
- **FR-004**: Every statement of the starting mode MUST agree — the seed, the
  declared default and the repair path — so an operator reading Admin →
  Instance sees what the instance actually did.

**What each mode requires**

- **FR-010**: The instance MUST NOT require a copyright-notice contact in order
  to complete setup unless the instance is being set up as open.
- **FR-011**: Readiness MUST report a capability that this instance's mode does
  not require as *not required in this mode*, naming the mode that would
  require it. It MUST NOT report it as satisfied, and MUST NOT hide it: an
  operator must be able to see the whole ladder and where they are on it.
- **FR-012**: The refusal to create a new share link while the copyright-notice
  contact is unset MUST remain in force **in every access mode**
  (`readiness::may_publish_beyond_world`). This spec relaxes when the question
  is asked, never whether an unanswered instance may publish.
- **FR-013**: Capabilities unrelated to sharing MUST NOT be affected. Mail,
  terms of service, feedback and lore synchronisation keep the requirements
  they have today.

**Changing mode**

- **FR-020**: Issuing an instance invitation while the instance is closed MUST
  tell the operator, at that moment, that nobody can redeem it until the
  instance is at least invite-only, and MUST offer to make that change.
- **FR-030**: A change to open MUST NOT take effect unless every setting behind
  `Capability::PublishBeyondWorld` holds a value. The refusal MUST name each
  missing setting and where to set it, in the words readiness already uses.
- **FR-031**: Where the mode is fixed by the environment and cannot be changed
  in the product, the instance MUST report that the requirement is unmet and
  which environment variable holds the mode, rather than silently allowing an
  open instance with no notice contact.
- **FR-032**: Before an operator confirms a change to open, the product MUST
  state plainly what an open instance obliges: a contact a rights holder can
  serve a notice on, reachable without an account; a takedown and counter-notice
  process; and a repeat-infringer policy. It MUST also state that these
  obligations are not satisfied retroactively.
- **FR-033**: A change *away from* open MUST never be refused for a missing
  legal detail, and MUST NOT delete any stored detail.
- **FR-034**: Every mode change MUST be recorded with who changed it, when, and
  the previous and new modes, as `instance_access_events` already records
  (`admin.rs:800-810`).

**An instance that is already open**

- **FR-040**: An open instance whose copyright-notice contact is incomplete
  MUST keep running, keep its mode, and keep every account working.
- **FR-041**: Its operators MUST be shown an unmissable statement of that
  condition in the admin portal, naming both ways out — supply the contact, or
  leave open mode.
- **FR-042**: The statement MUST clear without a restart once the contact is
  supplied, because the policy and the settings are both read per request
  (`admin_setup.rs:188-191`).

**The written record**

- **FR-050**: The constitution's DMCA / Content Moderation Guardrail MUST be
  amended to state which legal capabilities apply in which access mode, with a
  version bump and an updated Sync Impact Report, per its own Governance
  section.
- **FR-051**: An ADR MUST record this decision: what is claimed, what is not
  claimed, and the outward paths that do not consult the access mode.
- **FR-052**: Spec 015 MUST carry a note where this spec conditions its
  requirements. No requirement of spec 015 is removed by this spec.

**Proof**

- **FR-060**: An end-to-end test MUST prove that setup on an empty instance
  asks for no copyright-notice detail and finishes closed.
- **FR-061**: An end-to-end test MUST prove that a switch to open is refused
  while the details are missing, that the refusal names them, and that the
  switch succeeds once they are supplied.
- **FR-062**: A test MUST prove that an instance with existing users keeps its
  mode across the change that introduces this rule.
- **FR-063**: A test MUST prove that creating a share link is still refused on a
  closed instance with no copyright-notice contact — that FR-012 holds.

### Key Entities

- **Access mode**: open, invite-only or closed. Decides who may create an
  account, and — as of this spec — which legal capabilities the instance must
  carry.
- **Legal capability**: a named thing the instance can or cannot do because of
  what it has been told, as `Capability` already models: identify its operator,
  publish terms, publish content beyond a world.
- **The notice contact**: the name, e-mail address and postal address on which
  a copyright notice is served — the three `notice.*` settings.
- **Mode change record**: who changed the mode, when, from what to what; kept
  whether or not the instance is later closed again.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A new instance completes setup with **zero** steps asking for a
  copyright-notice detail, and reports its mode as closed, in 100% of runs.
- **SC-002**: An instance with at least one existing account has the same
  access mode before and after the upgrade that introduces this rule, in 100%
  of runs.
- **SC-003**: 100% of attempts to switch to open with an incomplete notice
  contact are refused, and each refusal names every missing setting.
- **SC-004**: After the missing settings are supplied, the switch to open
  succeeds and a new account can be created **within three minutes**, with no
  restart (the bound spec 035's SC-004 already holds the policy to).
- **SC-005**: 100% of attempts to create a share link on an instance with an
  incomplete notice contact are refused, in every one of the three modes.
- **SC-006**: 100% of mode changes appear in the operator-visible record with
  who, when, and both modes.
- **SC-007**: An operator reading the readiness screen on a closed instance can
  say, without leaving the page, which capability is not required and which
  mode would require it.

## Assumptions

- **"Closed" is a statement about admission, not about reach.** It is true that
  a closed instance shares nothing outside itself only while nothing else on it
  publishes outward. FR-012 keeps the existing publishing gate in force in
  every mode so that the statement stays true by construction rather than by
  assertion. Questions 1 and 2 below are where the remaining gap lives.
- **The operator is the accountable party.** This product is self-hosted. An
  operator choosing to run open without the apparatus is making a decision
  about their own exposure; the product's job is to make that decision explicit
  and hard to make by accident, not to prevent it in a build they control.
- **The existing gate is the enforcement point.** Nothing new needs to decide
  whether a share may be created; `may_publish_beyond_world` already does, and
  the surface test already closes the list of callers.
- **Nothing here changes what a takedown does.** Spec 015's flow, spec 039's
  standing ladder and spec 051's pauses are untouched.
- **This is not legal advice**, and the spec does not pretend to be. It records
  a product decision and the reasoning offered for it, so that a lawyer reading
  it later can see exactly what was and was not claimed.

## Out of Scope

- Changing what the open mode requires beyond the three `notice.*` settings.
- Any per-world or per-account sharing setting. Mode is an instance-level fact.
- Registering an agent with a copyright office, or any external filing. The
  product publishes a contact; filing is an operator's act.
- Restricting who may *read* an existing share link — see Q1, which asks
  whether that should change and deliberately does not decide it here.

## Dependencies

- **Spec 015**: the notice-and-takedown programme this conditions.
- **Spec 035**: instance access, whose policy, audit trail and admission gate
  this builds on.
- **Spec 039**: sharing attestation, whose publishing gate is the enforcement
  point FR-012 preserves.
- **Spec 040**: instance setup, whose wizard stops asking a question.
- **Spec 042**: the admin portal, where `/admin/access` and readiness live.

## Decisions (owner, 2026-09-15)

1. **The access mode decides what the instance must carry.** On a closed or
   invite-only instance there is no real sharing, so the takedown apparatus is
   not required to operate. On an open instance it is. The owner's words: "On a
   closed instance, the user doesn't need to provide DMCA takedown information,
   but only if it's a closed instance… only on an open instance is there real
   sharing."

2. **Closed is where an instance starts.** "Instances should start closed and
   then go to invite-only or open." Opening is a deliberate act, taken by
   somebody who has been told what it means.

3. **Nobody's instance is closed by an upgrade.** Existing instances keep their
   mode. This repeats spec 035's own reasoning for the `CASE` in its migration
   and is repeated here because it is the property most easily lost when a
   default changes.

4. **The constitution is amended in the same change**, because its guardrail
   assumes sharing is always possible and would otherwise be read as requiring
   the programme of every instance regardless of mode.

## Questions for the owner

1. **Q1 — Should a share link on a closed or invite-only instance still open
   for a stranger?** Today it does: four resolvers answer callers with no
   account (`anonymous.rs:1-12`, ADR-070, ADR-071). This is the one fact that
   sits awkwardly against "on closed there's no real sharing".

   | Option | Answer | Implications |
   |--------|--------|--------------|
   | A | Leave it. Mode governs admission only; the existing publishing gate is the whole protection | Nothing to build. The claim in decision 1 stays *nearly* true, and the spec says where it is not. |
   | B | **On a non-open instance, a share link resolves only for a signed-in account on that instance** | Makes "shares nothing outside itself" literally true, which is what the legal argument rests on. Costs a session check on four resolvers and changes what an existing link does when an instance closes. |
   | C | On a non-open instance, refuse to *mint* anonymous links at all; existing ones keep working | Narrower than B, no behaviour change for links already issued, but an operator who closes an instance still has old public links live. |

   **Recommendation: B.** It is the option under which the sentence in the ADR
   is true without a footnote, and the cost is one predicate on four resolvers
   that already share a module. Its one sharp edge — a link that stops working
   when an instance leaves open mode — is the correct behaviour for a link that
   was only ever meant for people at the table.

2. **Q2 — Does turning on lore synchronisation oblige an instance the way going
   open does?** Lore sync (spec 034) copies a world's lore to a repository the
   instance does not operate. Spec 015's FR-015 to FR-018 are written about
   exactly that, and `Capability::SyncLore` requires only a GitHub app
   credential today — not a notice contact.

   | Option | Answer | Implications |
   |--------|--------|--------------|
   | A | No. Lore sync publishes to a repository the *user* controls, so the user is the publisher | Consistent with spec 015 FR-017. A closed instance stays unencumbered. |
   | B | **Yes: enabling lore sync requires the notice contact, whatever the mode** | The instance is operating an outward path and can be asked to stop carrying content outward (spec 015 FR-016); it should be reachable by somebody who wants it stopped. |
   | C | Only when the target repository is public | Truest, and needs the product to know a repository's visibility and to watch it change. |

   **Recommendation: B.** Spec 015's FR-016 already obliges the platform to be
   able to deactivate the outward path on a valid notice. An instance that can
   be asked to do something should have an address at which it can be asked.

3. **Q3 — What does an operator see when readiness reports a capability their
   mode does not require?** The wording matters more than it looks: "not
   required" reads as "done" if it is rendered like every other satisfied row.
   Recommendation: a third state of its own — neither a gap nor a tick — that
   names the mode which would require it, so the ladder stays visible from the
   bottom rung.
