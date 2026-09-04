# Legal text

**This directory is the review surface.** Every piece of user-facing legal
prose the product publishes is a markdown file here, so it can be read, revised
and signed off by someone who does not read TypeScript.

## Why the files are here rather than in the components

They used to be JSX, spread across `apps/web/src/pages/legal/` and
`apps/web/src/components/legal/`. That is a fine place for a page and a poor
place for a policy: a reviewer cannot read it, a diff of it is noisy with
markup, and "what exactly does our DMCA page say" required opening a React
component and mentally stripping the tags.

The application reads these files at build time. Editing one changes what the
product says, with no code change — which is the property that makes a legal
review actionable rather than advisory.

## What is here

| File | Where it appears | Status |
|---|---|---|
| `dmca-policy.md` | `/legal/dmca` | **Needs legal review before launch** |
| `notice-attestations.md` | The takedown and counter-notice forms | **Needs legal review before launch** — statutory |

`notice-attestations.md` is read differently from the others, and the
difference matters if you edit it. Its `##` headings are **stable identifiers**
that the forms look text up by, not decoration: rename one and the form
referencing it fails rather than rendering a live checkbox beside a blank
affirmation. `legalDocuments.test.ts` asserts every identifier the forms use
still resolves, so a rename fails in CI and not in front of a submitter.

Reword the text under a heading freely. Change a heading only alongside the
code that references it.

## What is deliberately *not* here

- **Structured data**: the designated agent's name, address and electronic
  contact. Those are instance configuration, not prose, and they render as a
  definition list the page owns. They also carry a pre-launch placeholder.
- **The notice and counter-notice forms' field labels.** "Location of the
  allegedly infringing material" is UI, not policy. The statutory attestations
  those forms ask a submitter to affirm *are* policy, and they are here, in
  `notice-attestations.md`.
- **Per-system licence and attribution text.** That lives in each pack's
  `system.json` under its `legal` block, because it belongs to the pack and
  travels with it. `packs/systems/*/system.json` — eight of them, all
  populated.
- **The project's own licence.** `LICENSE` at the repository root, AGPL-3.0.

## Open items

- **No Terms of Service and no Privacy Policy exist.** Checked 2026-09-04:
  nothing in the application references either. That is a gap to close before
  a public instance, not an omission from this directory.
- **Nothing here has been reviewed by a lawyer.** All of it was drafted
  in-repo. The agent designation on the DMCA page already carries a "configure
  before launch" placeholder; this text needs the same gate.
- **The designated agent's mailing address and electronic contact are
  placeholders.** `[Configure via instance legal/compliance settings before
  launch]` and `dmca@thunderforge.example`. These are instance configuration
  rather than prose, which is why they are not in this directory — but they
  block launch just as hard.
- **`notice-attestations.md` is a plain-language rendering of statutory
  elements, not a quotation of them.** 17 U.S.C. § 512(c)(3)(A) governs the
  notice elements and § 512(g)(3) the counter-notice elements. Whether the
  rendering is sufficient is exactly the kind of question the review exists to
  answer.
