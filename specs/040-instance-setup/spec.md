# Feature Specification: Instance Setup and Configuration

**Feature Branch**: `040-instance-setup`

**Created**: 2026-09-07

**Status**: Draft — **specification only, not being built yet.** Written
alongside 036–039.

**Input**: Project owner: "Maybe we make instance setup its own spec to house everything about the instance — from whether you want to use SMTP for mailing for users, what the instance owner email is in case of support needs, being able to set up GitHub App credentials on a global or specific feature scale. The entire whole setup as a custom individual spec."

## Context

### Configuration is being decided one field at a time, in whatever spec needed it

There is no single answer to "what is configurable about this deployment, and
where do I set it?" There are five answers, in five places, arrived at
separately:

- **The realm manifest** — `realm_name`, `interface_pack_id`, `asset_pack_id`,
  `support_email`, `welcome_message`, `default_game_system_id`, seeded from
  `config/realm-defaults.json` and editable through a whitelist in
  `admin::editable_manifest_keys`.
- **Instance access policy** — open, invite-only or closed (spec 035).
- **OAuth providers** — database rows, with environment variables winning for
  the fields they set (spec 007, ADR-041).
- **Lore sync's GitHub App** — `SYNC_GITHUB_APP_*`, environment only
  (`repo_host.rs`).
- **Whatever the next feature invents.** Spec 037 proposes
  `FEEDBACK_GITHUB_APP_*` and a global form; spec 039 needs an operator
  identity and a notice contact.

Each was a reasonable local decision. Together they are a deployment nobody
can describe.

### First run asks for almost nothing

`admin_setup_basic` takes a username, an email and a password. That is the
entire ceremony by which a deployment becomes an instance. Everything else —
who runs it, how to reach them, where notices go, whether it can send mail —
is either a default nobody revisited (`support_email` still says
`stewards@thunderforge.local`), a marker in a markdown file
(`[OPERATOR]` in `legal/terms-of-service.md`), or absent.

**An operator running a container will never edit markdown in a repository.**
If the only way to become contactable is a source edit, the instance is
uncontactable, and it will publish terms naming nobody.

### There is no mail

Stated plainly because several specs already promise otherwise: **no mail
subsystem exists anywhere in this codebase.** No SMTP, no mailer, no queue, no
template. Meanwhile spec 039 says a person is told when a strike is recorded,
when their account is disabled and when an appeal resolves; spec 037 tells a
submitter what became of their feedback; spec 035's invitations arrive by some
means. Every one of those is currently a promise with nothing behind it.

Configuring mail without providing mail would repeat the mistake, so this
feature owns both: the settings **and** the capability to deliver a message,
with a test that proves it works before somebody depends on it. What each
feature chooses to send remains that feature's business.

### What this spec owns, so other specs stop restating it

**This spec owns collecting and editing instance configuration.** Other specs
depend on it: spec 039 requires that a notice contact exists before anything
may be published; spec 037 requires a feedback destination. Neither owns the
setup screen, and neither should describe how credentials resolve.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - From empty database to a real instance, in one pass (Priority: P1)

Somebody deploys ThunderForge and opens it. Instead of a bare account form,
they are walked through what this deployment needs to be: their administrator
account, who operates it, how to reach them, where copyright notices go, and
whether it can send mail. They finish with an instance that is usable and
contactable, without editing a file.

**Why this priority**: it is the feature. Every other story here is this one
being maintained afterwards.

**Independent Test**: bring up an empty database, complete setup, and confirm
the instance is usable and that its published pages name a real operator and a
real contact.

**Acceptance Scenarios**:

1. **Given** an uninitialised instance, **When** an operator opens it, **Then**
   they are guided through setup rather than shown a bare account form.
2. **Given** setup, **When** it asks for what the instance needs, **Then** it
   collects the administrator account, the operator identity, the notice
   contact, the support address and the mail settings in one pass.
3. **Given** completed setup, **When** anyone views the published legal pages,
   **Then** they name the operator and the contact that were entered — with no
   placeholder text and no file edited.
4. **Given** setup, **When** a required value is left blank or filled with an
   obvious placeholder, **Then** it is refused rather than published.
5. **Given** an operator who wants to defer optional things, **When** they
   skip them, **Then** setup completes and the instance says what is still
   unset.

---

### User Story 2 - Everything set at setup can be changed afterwards (Priority: P1)

An operator changes the support address, the notice contact, the mail server
or a credential months later, from the admin surface, without redeploying.

**Why this priority**: a setup wizard whose answers are frozen is a worse
version of the file it replaced. Equal to US1 because a wrong value entered
once must not be permanent.

**Independent Test**: change each value from the admin surface and confirm the
instance uses the new one without a restart.

**Acceptance Scenarios**:

1. **Given** a configured instance, **When** an administrator changes a value,
   **Then** it takes effect without redeploying or restarting.
2. **Given** a change to something that matters — the notice contact, the
   operator identity, a credential — **When** it is made, **Then** it is
   recorded: who, when, and what it was before.
3. **Given** a value fixed by the environment, **When** an administrator views
   it, **Then** they can see that it is fixed and where it comes from, rather
   than editing a field that silently will not take.

---

### User Story 3 - One precedence rule, everywhere (Priority: P1)

An operator setting a value in the environment and an operator setting it in
the admin screens get the same answer to "which one wins?" for every setting in
the product, and can always see which source a live value came from.

**Why this priority**: spec 007 settled this for OAuth — the environment wins
for the fields it sets — and every later subsystem has been free to decide
again. One rule stated once is the difference between configuration and
folklore.

**Independent Test**: set the same value in both places for several different
settings and confirm the same source wins each time, and that the interface
says so.

**Acceptance Scenarios**:

1. **Given** a value set in both the environment and the instance's own
   settings, **When** it is resolved, **Then** the environment wins — for every
   setting, not only for the ones that decided it first.
2. **Given** any live setting, **When** an administrator inspects it, **Then**
   they are told which source it came from.
3. **Given** a setting fixed by the environment, **When** an administrator
   tries to change it, **Then** they are told it is fixed and where, rather
   than being allowed to make a change that does nothing.

---

### User Story 4 - The instance can send mail, and knows whether it can (Priority: P2)

An operator configures a mail server and sends a test message to themselves.
It arrives, and the instance is now a place that can tell somebody something.
An operator who configures nothing gets an instance that says plainly which
features are affected, rather than one that silently drops messages.

**Why this priority**: several shipped and specified features assume delivery
exists. P2 rather than P1 because an instance is usable without mail, so long
as it is honest about it.

**Independent Test**: configure a mail server, send a test message, receive it;
then remove the configuration and confirm the affected features say what they
cannot do.

**Acceptance Scenarios**:

1. **Given** mail settings, **When** the operator asks to test them, **Then** a
   message is sent and the outcome is reported — success, or a reason a person
   can act on.
2. **Given** wrong settings, **When** they are tested, **Then** the failure
   names what to fix and never prints a password.
3. **Given** no mail configured, **When** a feature would send something,
   **Then** it does not silently discard it: the operator can see that a
   message could not be delivered, and features that depend on delivery say so.
4. **Given** a message the instance failed to deliver, **When** an operator
   looks, **Then** they can see that it failed and why, without the contents
   of somebody's message being exposed to anyone it was not for.

---

### User Story 5 - GitHub credentials at two scales (Priority: P2)

> **The scope vocabulary is spec 037's**, and its contract is the fuller
> statement: `specs/037-in-app-feedback/contracts/github-app-scopes.md`.
> Spec 037 needed a *second* GitHub application before this spec existed, which
> is where `AppScope`, per-field sources and the fall-through rules were first
> written down. This spec owns the **resolution rule** — an application resolves
> whole, never field by field (FR-021) — and that rule overrode the field-at-a-
> time resolution 037's delivery originally shipped. Read both; where they
> disagree, this one is later. (Spec 037 T064.)

An operator either registers one application for everything, or one per
subsystem, or a mixture — and understands what they have chosen while they are
choosing it.

**Why this priority**: lore sync needs it today, feedback needs it next, and
the resolution rule must be decided once rather than three times.

**Independent Test**: configure only a global app and confirm every subsystem
uses it; add a subsystem-specific one and confirm it wins for that subsystem
alone.

**Acceptance Scenarios**:

1. **Given** only global credentials, **When** any GitHub-using subsystem needs
   an application, **Then** it uses the global one.
2. **Given** both global and subsystem-specific credentials, **When** a
   subsystem resolves, **Then** the specific one wins and the global one still
   serves everything else.
3. **Given** an operator setting global credentials, **When** they set them,
   **Then** they are told which subsystems this application will act for.
4. **Given** a partially-specified subsystem application, **When** it resolves,
   **Then** the operator can see which values came from which source rather
   than a half-configured application being silently completed.
5. **Given** any credential problem, **When** it is reported, **Then** the
   report names the variable or field and never a value, a fragment or a
   length.

---

### User Story 6 - An instance says what it is not ready for (Priority: P2)

An administrator can see, in one place, what this instance can and cannot do
given how it is configured — and each gap names what to set.

**Why this priority**: the alternative is discovering a missing setting at the
moment somebody needed it to work. It is also what lets other specs depend on
a configured state rather than assuming one.

**Independent Test**: bring up an instance with several things unset and
confirm each is listed with what it disables and how to fix it.

**Acceptance Scenarios**:

1. **Given** an instance with no notice contact, **When** anyone tries to
   publish content beyond a world, **Then** it is refused and the
   administrator is told what is missing (spec 039 depends on this).
2. **Given** an instance with no mail, **When** an administrator looks at
   readiness, **Then** they see which features are limited and why.
3. **Given** a fully configured instance, **When** an administrator looks,
   **Then** they see it, rather than an absence of complaints.
4. **Given** a readiness report, **When** it is displayed, **Then** it contains
   no secret, no fragment of one, and no length.

---

### Edge Cases

- An instance is upgraded and a new required setting appears that no existing
  deployment has. It must not break on start, and must ask rather than assume.
- Setup is abandoned half-way and the browser is closed.
- Two people open setup at the same moment on a fresh deployment.
- The environment sets a value that the database also holds from before it was
  set — the resolved answer must be unambiguous and visible.
- A credential is valid at setup and revoked later. The failure belongs to the
  subsystem, and the diagnostic belongs here.
- Mail settings that connect but are rejected by the receiving side.
- An operator removes the only administrator account.
- A container is redeployed with a fresh environment and an existing database.
- An operator wants no configuration UI at all and to set everything by
  environment — a common posture for an automated deployment.

## Requirements *(mandatory)*

### Functional Requirements

**First run**

- **FR-001**: An uninitialised instance MUST present a guided setup rather than
  a bare account form.
- **FR-002**: Setup MUST collect, in one pass: the first administrator's
  account, the operator identity, the contact for copyright notices, the
  support address, and mail delivery settings.
- **FR-002a**: Setup MUST also take the first administrator through enrolling
  a second factor, and MUST NOT complete without it. Spec 041 owns that flow
  and its rule that an administrator always holds a second factor; this
  requirement exists so that 040's own definition of "setup is finished"
  agrees with it.
- **FR-003**: Setup MUST distinguish what it requires from what it merely
  offers, and MUST complete without the optional parts.
- **FR-004**: A required value that is blank or an obvious placeholder MUST be
  refused rather than published.
- **FR-005**: On completion, the instance's published pages MUST name the
  operator and the contact that were entered, with no placeholder text and no
  file edited.
- **FR-006**: Setup MUST be resumable if abandoned, and MUST NOT be completable
  twice.

**Editing afterwards**

- **FR-007**: Everything collected at setup MUST be editable afterwards from
  the administration surface, without redeploying or restarting.
- **FR-008**: A change to the operator identity, the notice contact or any
  credential MUST be recorded: who changed it, when, and what it was before.
- **FR-009**: An administrator MUST NOT be offered an editable field for a
  value the environment has fixed; they MUST be told it is fixed and where it
  comes from.

**One precedence rule**

- **FR-010**: For every setting in the product, a value set in the environment
  MUST win over a value stored by the instance — the rule spec 007 set for
  OAuth providers, applied universally rather than per subsystem.
- **FR-011**: Every live setting MUST be able to report which source it came
  from.
- **FR-012**: A new configurable setting MUST inherit this rule rather than
  choosing its own.

**Mail**

- **FR-013**: An operator MUST be able to configure mail delivery and to send a
  test message that proves it works before anything depends on it.
- **FR-014**: A failed test MUST name what to fix and MUST NEVER print a
  password or any fragment of one.
- **FR-015**: With no mail configured, the instance MUST NOT silently discard
  messages: failures MUST be visible to an operator, and features that depend
  on delivery MUST say what they cannot do.
- **FR-016**: A delivery failure record MUST be visible to an operator without
  exposing the contents of a message to anyone it was not addressed to.
- **FR-017**: The instance MUST be usable without mail configured; mail MUST
  limit features rather than prevent operation.

**GitHub applications**

- **FR-018**: An operator MUST be able to configure one application used by
  every subsystem that talks to GitHub, and applications specific to a
  subsystem.
- **FR-019**: A subsystem-specific application MUST win for that subsystem, and
  the global one MUST continue to serve subsystems that have none.
- **FR-020**: Wherever global credentials are set or offered, the operator MUST
  be told which subsystems the application will act for.
- **FR-021**: Where a resolved application draws on more than one source, the
  operator MUST be able to see which value came from where.
- **FR-022**: Credentials MUST be validated when configured rather than at
  first use.
- **FR-023**: No diagnostic anywhere MUST print a credential, a fragment of
  one, or its length.
- **FR-024**: Existing environment configuration MUST keep working unchanged;
  this feature MUST NOT require a working deployment to be reconfigured.

**Readiness**

- **FR-025**: An administrator MUST be able to see what the instance can and
  cannot do given its configuration, with each gap naming what to set.
- **FR-026**: An instance with no notice contact MUST refuse operations that
  publish content beyond a world, and MUST say why.
- **FR-027**: A readiness report MUST contain no secret, no fragment of one and
  no length.
- **FR-028**: An upgrade that introduces a new required setting MUST NOT
  prevent an existing instance from starting; it MUST ask.

### Key Entities

- **Instance configuration**: everything that decides what this deployment is —
  identity, contacts, delivery, credentials — with each value's source.
- **Operator record**: who runs this instance and how to reach them, including
  where copyright notices go.
- **Credential set**: one application's identity and secret, scoped either to
  the instance as a whole or to one subsystem.
- **Delivery settings**: how this instance sends a message, and whether it can.
- **Readiness**: the derived answer to what this instance can currently do.
- **Configuration change record**: who changed what, when, and from what.

## Success Criteria *(mandatory)*

- **SC-001**: An operator goes from an empty database to a usable, contactable
  instance in under 10 minutes without editing a file.
- **SC-002**: Every value collected at setup can be changed afterwards without
  a redeploy.
- **SC-003**: For every setting in the product, the same precedence rule
  applies, and an administrator can see the source of any live value.
- **SC-004**: An operator can prove mail works before any feature relies on it,
  and no test failure ever displays a password.
- **SC-005**: An instance with no mail configured runs, and every feature that
  needs mail says what it cannot do.
- **SC-006**: An operator can configure one application for everything, or one
  per subsystem, and in both cases is told what each application will act for.
- **SC-007**: No configuration screen, diagnostic or log displays a credential,
  a fragment of one, or its length — demonstrated by attempting to find one.
- **SC-008**: An instance missing a required setting says so before somebody
  discovers it at the moment they needed it.
- **SC-009**: An existing deployment upgrades without reconfiguration and
  without failing to start.

## Assumptions

- **The environment wins.** Taken from spec 007 and ADR-041 and generalised
  rather than re-decided.
- **Configuration is per instance, not per world.** World settings are a
  different thing and stay where they are.
- **Mail means SMTP first.** It is what a self-hoster has; other delivery
  mechanisms are a later addition and nothing here precludes one.
- **This spec provides the delivery capability, not the messages.** What each
  feature sends, and when, belongs to that feature.
- **The realm manifest's existing editable keys stay editable**, and this
  feature gathers them into the same surface rather than replacing them.
- **An automated deployment can set everything by environment** and never open
  a configuration screen. Setup must not be the only way in.
- **Secrets are stored the way the instance already stores secrets.** Choosing
  a secrets-management system is not this feature's decision.

## Out of Scope

- Per-world or per-table configuration.
- Multi-tenant hosting: one deployment, one instance, one operator.
- An external secrets manager, key vault or configuration service.
- Changing what any existing setting means. This feature gathers and
  standardises; the meanings stay.
- Sending any particular message. Specs 035, 037 and 039 own their own
  messages; this owns whether a message can be sent at all.
- Migrating existing deployments' environment configuration into the database.
  Environment configuration keeps working exactly as it does.
