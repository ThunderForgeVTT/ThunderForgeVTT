# Specification Quality Checklist: The Sharing Attestation

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-09-07
**Feature**: [spec.md](../spec.md)

## Content Quality

- [x] No implementation details (languages, frameworks, APIs)
- [x] Focused on user value and business needs
- [x] Written for non-technical stakeholders
- [x] All mandatory sections completed

## Requirement Completeness

- [x] No [NEEDS CLARIFICATION] markers remain
- [x] Requirements are testable and unambiguous
- [x] Success criteria are measurable
- [x] Success criteria are technology-agnostic (no implementation details)
- [x] All acceptance scenarios are defined
- [x] Edge cases are identified
- [x] Scope is clearly bounded
- [x] Dependencies and assumptions identified

## Feature Readiness

- [x] All functional requirements have clear acceptance criteria
- [x] User scenarios cover primary flows
- [x] Feature meets measurable outcomes defined in Success Criteria
- [x] No implementation details leak into specification

## Notes

- **The premise was checked before the spec was written**, and it changed what
  the spec is. The instinct was to write a policy feature; the policy already
  exists — `legal/terms-of-service.md` and `legal/collection-sharing-terms.md`
  both already say what the owner described, and the moderation program that
  enforces it is built and complete. What did not exist is any record, any
  enforcement, and any coverage of three of the four publishing paths. The
  spec is therefore about evidence and reach, not words.

- **FR-006 is the requirement the position rests on.** "You are the accountable
  owner" is a claim about somebody else's act, and a claim about somebody
  else's act needs evidence. Displaying terms and keeping nothing produces all
  the friction of a legal control and none of its value.

- **FR-011 and FR-014 are the ones most likely to be quietly dropped.** The
  work looks done once the dialog appears on all four paths, and it is not:
  today the confirmation lives entirely in `WorldCollectionsPage.tsx`, so the
  publishing operation itself has never required anything. Constitution
  Principle III already says authorization belongs at the data boundary rather
  than in a component; this is the same rule applied to an agreement.

- **FR-023 is the one with no obviously right answer.** What happens to copies
  already adopted into other worlds is a real tension: leaving them live means
  a takedown did not take anything down, and deleting them silently punishes
  somebody who acted in good faith. The Assumptions section proposes disabling
  with notice rather than deletion. It deserves the first `/speckit-clarify`
  question and probably an ADR.

- **Not legal advice, and the spec says so at the top.** `legal/README.md`
  already marks the instance's prose as needing review before launch. This
  feature makes the words operative; it does not make them correct, and
  nothing here should be read as having decided that they are.

## Re-validation after clarification (2026-09-07)

One answer folded in; all items still pass.

- **FR-023 is decided: disabled, not deleted**, with FR-023a–FR-023d spelling
  out what that means — stops being usable and served, the adopter is told and
  is not accused, their own surrounding work is untouched, and restoration
  rides the process that already exists rather than a request.

- **The restoration half is the part that would have been missed.** The
  moderation program already restores lazily once a forwarded counter-notice
  passes its waiting period; without FR-023d an adopted copy would have been
  the one thing that stayed dark after the source came back, and nobody would
  have noticed until an adopter complained.

- **FR-023c is small and load-bearing.** "Disable the copy" is easy to
  implement as "disable the thing that contains the copy", which would take an
  adopter's own work down with it. Saying so is cheaper than discovering it.

## Re-validation after the three-strike clarification (2026-09-07)

All items still pass. Four things worth recording:

- **"Three strikes" is the behaviour the existing counting was already built
  for.** `moderation::repeat_infringer_threshold()` defaults to **3**, over a
  365-day lookback, counting *upheld, non-restored* cases. So FR-027 does not
  invent a definition of "strike"; it names the one already implemented, which
  is why an accusation is not a strike and a successful counter-notice removes
  one.

- **What is genuinely missing is the consequence, not the counting.** The
  threshold feeds exactly one thing today: `repeatInfringerFlags`, an admin
  query that surfaces accounts for human review. Nothing acts on it. Disable,
  window, export, appeal and deletion are all net-new — and no account
  deletion of any kind exists in the codebase today.

- **FR-034 is the one that stops this being cruel by accident.** An appeal
  unresolved at day thirty pauses the deletion. Without it, an account is
  deleted because the instance was slow, which is the instance punishing
  somebody for its own backlog. FR-035 is the same instinct: a strike ageing
  out while an account is disabled should restore it without the person having
  to ask.

- **FR-039 exists because the obvious implementation locks the door from the
  inside.** Automatic disablement applied to the last remaining administrator
  would leave nobody able to administer the instance, including to undo it.

- **FR-032 is a promise about remedies, not a feature.** Downloading your data
  and appealing are not alternatives. `exportMyData` already exists under
  ADR-011, so the download half is largely a matter of keeping it reachable
  while the account is otherwise shut.

## Re-validation after the self-hosting clarification (2026-09-07)

All items still pass. The spec has grown a third link and is no longer only
about sharing; the branch name is kept, and the Context now says so.

- **The ToS is already written for a self-hosted instance.** Its own header
  says "the operator of the instance is the party offering the service", it
  carries `[OPERATOR]` markers throughout, and it already directs a user's
  questions to the operator. So the position exists — for the people *using* an
  instance. What does not exist is the statement to the person *running* one.

- **First-run setup is the only moment this can be said.** An operator never
  signs up for anything; they clone a repository and run a binary. There is no
  other point at which a human being becomes an operator, and `admin_setup` is
  where that happens. A statement placed anywhere else is a statement nobody
  reads, which is the same failure the sharing terms already avoided by
  appearing at the share button rather than on a policy page.

- **FR-048 is the honest one.** The project cannot act on content in an
  instance it does not run — no access, no takedown, no standing. The spec's
  job is to make sure nothing in the product implies otherwise and that
  somebody looking for a remedy is pointed at the party who actually has one.

- **The AGPL does a different job.** It disclaims warranty and liability for
  the *software*; the operator statement says who carries the obligations of
  running a *service*. Recorded in Assumptions so nobody concludes the licence
  already covers this.

- **FR-047 catches the quiet failure.** An instance deployed without its
  `[OPERATOR]` markers filled in publishes terms naming nobody — which is worse
  than publishing none, because it reads as though somebody is accountable.

## Re-validation after the instance-setup clarification (2026-09-07)

All items still pass.

- **This closes FR-047 properly rather than warning about it.** The earlier
  requirement said an instance with unfilled `[OPERATOR]` markers must tell its
  administrator; FR-050 to FR-052 remove the failure mode instead — setup
  collects the operator identity and notice contact, the instance stores them,
  and the legal pages render them. An operator running a container should not
  have to edit markdown in a repository to become contactable, and today that
  is exactly what the placeholders require.

- **FR-053 is the enforcement worth arguing for.** An instance with nobody to
  notify may be played on and may not publish. It is softer than refusing to
  boot and harder than a warning, and it lines up with the rest of the spec:
  everything here gates *publishing*, not playing.

- **`instance_identity` is not the right table and should not be reused.** It
  holds one UUID and exists for spec 034's binding records, with a documented
  hole about database copies. Operator identity is a different thing with a
  different lifetime and different consequences when wrong.

- **FR-055 keeps a legal obligation where it belongs.** Registering a
  designated agent, where the jurisdiction requires one, is the operator's own
  act. Collecting a contact field must not read as having done it for them.
