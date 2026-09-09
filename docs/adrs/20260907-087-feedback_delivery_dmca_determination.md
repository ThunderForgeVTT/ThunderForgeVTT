# Feedback Delivery Is Not a Public Repository of User Content

- **Date**: 2026-09-07
- **Status**: Accepted
- **Spec**: `specs/037-in-app-feedback/` (FR-014, FR-030)
- **Relates**: the constitution's DMCA guardrail (spec 015, ADR-043), which
  requires an explicit determination before a feature exposes one world's
  content beyond that world

## The decision

Forwarding a feedback report to an operator's issue tracker **is not** the
"centralized public repository" the DMCA guardrail is about, and this feature
does not require the notice-and-takedown program to be operational before it
ships.

The determination rests on three things being true, and the feature is built so
that they stay true:

1. **The operator chooses the destination.** It is their repository, configured
   by them. This product does not host it, aggregate it, or index it.
2. **The person is told where it goes, and whether that is public, before they
   submit.** Visibility is read from the host rather than assumed. Unknown
   renders as a warning, not as reassurance — the safe reading of a destination
   nobody could check is not "private".
3. **Only what was approved is sent.** The review shows the message, the
   context, the log bundle and any screenshot, and what it shows is what
   leaves.

## Why this is not the guardrail's case

The guardrail exists for a specific shape: one instance's user-authored content
becoming visible to other instances or to the public *through this product*.
Feedback is none of that. A report is authored deliberately, about the software
rather than as campaign content, addressed by its author to the people running
their instance, and delivered to a destination those people own.

The closest thing to user content that can travel is a **screenshot** — which
may contain a map, a token, or a character sheet. That is why point 2 exists
and why the visibility warning is not decoration: a person attaching a
screenshot to a report bound for a public repository is publishing it, and they
are told so at the moment they can still decline.

## What would change this

If this product ever forwarded feedback to a destination **it** owns rather
than one the operator configured — a central bug tracker, an aggregated
telemetry sink — this determination does not carry, and the guardrail applies.
That would be a new decision, not an extension of this one.

## Alternatives considered

- **Block screenshots entirely.** Rejected: evidence is what makes a report
  actionable, and the person deciding what is in their own screenshot is
  better placed than a rule.
- **Require the takedown program before shipping feedback.** Rejected on the
  determination above, recorded here so that the reasoning is reviewable rather
  than assumed.
