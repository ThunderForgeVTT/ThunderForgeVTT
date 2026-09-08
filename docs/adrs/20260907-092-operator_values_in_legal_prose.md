# Operator Values Are Substituted Into Compiled-In Legal Prose At Render Time

- **Date**: 2026-09-07
- **Status**: Accepted
- **Spec**: `specs/040-instance-setup/` (FR-005, FR-017, FR-018),
  `contracts/legal-rendering.md`, `research.md` § R4, § D2; spec 039 FR-055,
  FR-056
- **Follows**: ADR-091, which made the operator record settings rows
- **Governs**: how a published legal page names a real operator without the
  operator becoming an author of it

## The decision

The prose stays where it is: `legal/*.md`, read by `import.meta.glob` at build
time, compiled into the bundle. Into it, at **render** time, a **closed set of
six named tokens** resolves from instance settings.

Three rules, and they are the decision:

1. **The token set is closed and declared in code.** Six tokens, one per
   setting key, in `OPERATOR_TOKENS`. A `{{...}}` that is not in the table is
   left in the page verbatim. There is no template engine over legal text.
2. **A substituted value is rendered as text and never parsed.**
   `substituteOperatorValues` returns *alternating segments*, and only the prose
   ones reach `LegalProse`'s inline `**bold**` / `[text](url)` matcher.
3. **An unset token renders its visible marker**, not an empty string, and the
   marker is the exact text the markdown carried before the token replaced it.

And the ordering that makes rule 2 hold: **paragraphs are split before
substitution**, on the repository's own text.

## Why substitution at render time at all

`legal/terms-of-service.md` carried nine `[OPERATOR — …]` markers and
`legal/privacy-policy.md` five. `DmcaCompliancePage.tsx` had the designated
agent hard-coded as JSX — literally `Copyright Agent, ThunderForge` and
`dmca@thunderforge.example`. FR-005 says a published instance names a real
operator, and every deployed instance was publishing ours.

The rejected alternatives are what pin this in place:

- *Write the values into the markdown at setup.* Requires the running container
  to write into its own source tree — the "operator edits markdown in a
  repository" failure the spec's Context exists to reject.
- *Serve `legal/*.md` from the server so it can be templated there.* Moves
  trusted prose onto a runtime read path and forces it through the sanitizing
  comrak/ammonia pipeline, undoing a decision `legalDocuments.ts` documents at
  length. It gains nothing: substitution is the only dynamic part.
- *Free-form template variables.* An operator-authored template inside a legal
  document is an injection surface in the one document nobody re-reads.

## The trust boundary, which is the whole reason this is an ADR

`legalDocuments.ts` states the invariant that lets `LegalProse` exist at all:
this text is **ours** — in this repository, reviewed here, compiled in by Vite
at build time — which is why it may be rendered without the server's sanitizing
markdown pipeline, the one that exists for lore a player typed. `LegalProse`
renders four constructs and parses trusted input. That is a deliberate,
documented trade.

Substitution punches one hole in it: a handful of values an operator typed into
setup are rendered inside prose a lawyer signed off. **The hole is kept the size
of the values.**

Rule 2 is how. The two halves of a substituted paragraph have different trust,
and a `string` return could not express that — whoever rendered it would have to
choose *parse it all* or *parse none of it*, and both are wrong. Parse it all,
and an operator whose display name is `[click here](https://elsewhere.example)`
has published a link inside the terms of service. Parse none of it, and the
repository's own `**bold**` stops rendering in the document it was authored for.

So `substituteOperatorValues` hands back `{ kind: "prose" }` and
`{ kind: "value" }` segments, and `LegalProse` runs `inline()` over the first
and wraps the second in a plain `<span data-operator-value>`. **The shortcut was
tried first**: substitute into the paragraph string and let the existing parser
run over the result. Both files record that it was tried and rejected, and both
warn against the follow-on mistake — do not "fix" a value that renders with
visible markup by teaching `inline()` about it. The visible markup *is* the
correct rendering of that name.

Consecutive prose chunks are merged rather than emitted separately, so an unset
token in the middle of a sentence leaves the sentence as one string. Splitting it
would break a `**bold**` that spans the token — a correctness detail, not an
optimisation.

## Why the split happens before substitution

`LegalProse` splits the section body on blank lines *first*, then substitutes
into each paragraph. The order is load-bearing: **a value containing a blank
line must not be able to introduce a paragraph break into a document a lawyer
signed off.** An operator's name is a name, not layout.

`legalDocuments.ts` therefore deliberately does **not** substitute in
`sectionsOf`. Doing it earlier would give a value that power, and a section body
is a `string`, which cannot carry the prose/value distinction rule 2 depends on.
The two rules are the same rule seen from two sides, and they are the reason
substitution lives in the component rather than in the document reader.

## Why an unset token renders a marker

`legalDocuments.test.ts` has asserted since these documents were added that
`[OPERATOR — …]` markers survive rendering, on stated reasoning worth repeating
here: *a page that omits who holds your data while reading as complete is worse
than one that visibly has a blank.*

Substituting an empty string for an unconfigured instance would delete that
property silently, on exactly the pages where it matters most — a privacy policy
that names nobody but reads as finished. So an unset token renders the marker
the markdown used to carry, **word for word**, as prose. An unconfigured
instance publishes the page it published before this feature existed.

`OPERATOR_UNSET_MARKERS` is therefore a review surface, not a constant table.
Changing an entry changes what an unconfigured instance says, and it has to stay
in step with `legal/README.md`.

An **unknown** token — a typo in a legal document — is left verbatim for the
adjacent reason: it should be visible in review, not silently render nothing
where a name belongs. Not an error, not a blank.

## Six tokens, four of them in use

`OPERATOR_TOKENS` declares `operator.name`, `operator.contact_email`,
`operator.jurisdiction`, and the three `notice.*` values. The last four appear in
no document today, and that is deliberate rather than an oversight:

- The governing-law marker in `terms-of-service.md` is **prose an operator has
  to write**, not a jurisdiction name substituted into a sentence. That is
  research.md § D2's finding, and it is why `Kind::Prose` exists in the settings
  registry at all.
- The DMCA designated agent is rendered by `DmcaCompliancePage.tsx` as a
  definition list, not inside prose, which is what `resolveOperatorValue` is for
  — resolution without segmentation, with `isSet` telling the caller which case
  it got. The trust rule is unchanged: the caller renders both as plain text.

They are declared because the contract's token set is the settings' set, and
because a document that one day starts naming the agent should find the token
already resolving rather than reaching for a helper.

## Why the values are fetched anonymously, and why a failure is not an error

`publishedOperatorValues` is served on the unauthenticated GraphQL endpoint.
Spec 039's FR-056 is the reason: somebody who needs to file a copyright notice
against this instance has no account on it, and requiring one to find out who to
write to would make the designation undiscoverable by exactly the person it
exists for. The query exposes those six values and nothing else about how the
instance is configured (`contracts/legal-rendering.md` rule 4) — it is not a
window onto the settings surface.

A failed fetch renders the markers. A network failure and an unconfigured
instance therefore produce the **same page**, deliberately: the alternative is a
legal page replaced by an error box, which serves nobody who came to it looking
for a contact address.

`LegalProse` resolves the values itself when none are passed, so every surface
rendering legal text names the operator without each page remembering to — the
terms, the privacy policy, the collection sharing terms at the share step. That
is several mounts of one component on one page, so the in-flight promise is
cached at module scope and the query runs once. The `values` prop remains, so a
test can render a known state.

## Consequences

- Adding a token is a code change in `operatorTokens.ts` with a test, not a
  string somebody typed into a markdown file. Adding *general* templating over
  legal text needs the ADR `contracts/legal-rendering.md` describes — it is not
  a helper function away.
- Editing the policy is still editing a markdown file with no code change, which
  is what makes legal review actionable rather than advisory. That property
  survived this feature intact and was the constraint it was designed around.
- `LegalProse` still supports no headings, lists, tables, images, code or raw
  HTML. Needing one is a deliberate addition here, not a reason to reach for a
  markdown library — and the trusted-input invariant has to survive it.
- A new file in `legal/` is not picked up until the Vite dev server restarts,
  because `legal/` sits outside `apps/web`. Unchanged by this decision, and
  written down in `legalDocuments.ts` because the symptom looks exactly like a
  broken glob.

## What this does not solve

It does not make the documents correct for any particular operator.
Substitution fills in *who*; it does not fill in *what law applies* — that is
prose the operator writes (§ D2) — and it does not register a DMCA agent, which
spec 039 FR-055 makes the operator's own obligation and which no software can
complete for them. A fully substituted page is a page that names a real person.
It is not legal advice and it is not a reviewed document for that jurisdiction.
