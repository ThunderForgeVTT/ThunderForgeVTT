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

## What is deliberately *not* here

- **Structured data**: the designated agent's name, address and electronic
  contact. Those are instance configuration, not prose, and they render as a
  definition list the page owns. They also carry a pre-launch placeholder.
- **The notice and counter-notice forms.** Their field labels are UI. The
  statutory attestations a submitter agrees to are prose and belong here; they
  are listed as an open item below rather than claimed as done.
- **Per-system licence and attribution text.** That lives in each pack's
  `system.json` under its `legal` block, because it belongs to the pack and
  travels with it. `packs/systems/*/system.json` — eight of them, all
  populated.
- **The project's own licence.** `LICENSE` at the repository root, AGPL-3.0.

## Open items

- **No Terms of Service and no Privacy Policy exist.** Checked 2026-09-04:
  nothing in the application references either. That is a gap to close before
  a public instance, not an omission from this directory.
- **The statutory attestations in the two notice forms are still in TSX.**
  `TakedownNoticeForm.tsx` and `CounterNoticeForm.tsx` each carry one
  under-penalty-of-perjury statement. They should move here.
- **`dmca-policy.md` has not been reviewed by a lawyer.** It was drafted
  in-repo. The agent designation on the same page already carries a
  "configure before launch" placeholder; this text needs the same gate.
