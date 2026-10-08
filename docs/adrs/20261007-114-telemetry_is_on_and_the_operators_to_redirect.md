# ADR-114: Telemetry Is On, Anonymous, and the Operator's to Redirect

**Date:** 2026-10-07
**Status:** **ACCEPTED** by the accountable owner, 2026-10-07, the day it was proposed. Not yet built: spec 086 builds it, and until then nothing is sent.
**Participants:** ThunderForgeVTT Team
**Related:** spec 086 (US6, US8, FR-002, FR-006 to FR-009, FR-021, FR-026, FR-034 to FR-036, Appendix A), spec 074 (the sealed demo), spec 037 (feedback redaction), constitution v1.5.0 Principle VII, [ADR-109](./20261002-109-an_instance_states_whether_it_publishes.md) (an instance states what it does)

---

## Problem Statement

ThunderForge is self-hosted. When it breaks on somebody else's instance,
the project hears about it only if somebody files an issue, and most people
who hit a bug do not; they leave. Spec 086 first made the landing and the
demo report by default and left the server image off unless its operator
opted in. That covers thunderforge.dev and nothing else, which is exactly
the part the owner can already see.

The owner decided on 2026-10-07: "for the base image i want TELEMETRY=true
default and i want to change our constitution to allow telemetry of
people's thunderforge instances to tell me what's going on not just my own
but they can override the otel endpoint if they want their own telemetry
else i see it and they can do false and this all goes into a disclaimer and
spec".

Reporting from an install the project does not run is a trust question
before it is an engineering one. The decision below is the set of
conditions under which the owner's default is acceptable.

## Decision

1. **Every build is on by default.** The landing, the demo, the web app and
   the self-hostable server image read one switch, `TELEMETRY`, default
   `true`. The default destination is `https://telemetry.thunderforge.dev`,
   for the server's OTLP export and for the browser config the server serves
   at `/telemetry.json` and `/demo/telemetry.json`.

2. **The operator can redirect it or turn it off, with one variable each and
   no rebuild.** `OTEL_EXPORTER_OTLP_ENDPOINT` sends the server's export to
   the operator's collector; `THUNDERFORGE_BROWSER_TELEMETRY_ENDPOINT` does
   the same for browsers. Either way nothing of that half reaches the
   project. `TELEMETRY=false` sends nothing anywhere and loads no telemetry
   code in the browser. No bundle carries an endpoint, so the served config
   is always what decides.

3. **What is sent depends on where it goes.** One function, `tier_for`,
   compares the destination with the project's default:
   - **Anonymous tier** (the project's collector): only the allow-list kept
     in code. Errors with redacted stacks, timings, counts, versions and
     bounded labels. For the server, metrics, `server.error` records and
     spans filtered to allow-listed attributes; no logs, no GraphQL
     variables, no ids, no hostnames.
   - **Operator tier** (anything else): full logs and unredacted spans,
     because the data stays on infrastructure the operator controls.

   A destination that cannot be parsed is anonymous, so a mistake can only
   send less.

4. **An install has a random id, and nothing else identifies it.** A UUIDv4
   generated on first start, stored in `instance_settings`, never
   regenerated, derived from nothing about the machine. It lets the owner
   tell one instance with fifty errors from fifty instances with one.

5. **It is disclosed wherever someone meets it,** in the same words: the
   README, `docs/guides/telemetry.md`, one line in the server's startup log
   naming the destination and both switches, a read-only panel in the admin
   settings, and the landing's and demo's **What we measure**. Spec 086's
   Appendix A holds the text; a change to what is sent changes it first.

6. **No test reports.** Every test run and e2e stack runs with
   `TELEMETRY=false`. Tests that prove telemetry capture it with in-memory
   exporters or Playwright routing.

7. **The demo's `connect-src` is a header set at serve time,** built from
   the same config the server serves, because a policy baked at build
   cannot name an operator's collector, and a header cannot loosen a
   `<meta>`.

## Alternatives Considered

### Opt-in: the operator sets an endpoint to turn it on

What spec 086 said before this revision, and the common default for
self-hosted software. It is the most comfortable for operators and gives
the owner almost nothing: the operators who opt in are the ones already
talking to the project, and the silent failures the owner wants to see are
on the instances whose operators never read that far. Rejected.

### Off by default, with a first-run prompt

The first-run wizard (spec 064) could ask. It is a decision made once, by
one admin, in a wizard full of other decisions, and the honest answer for
most is "skip". It also needs a stored setting that outranks the
environment, which makes the switch two places instead of one. Rejected;
the startup line and the admin panel tell the same admin, without asking.

### Landing and demo only

What spec 086 had decided earlier the same day. It sees the visitors to
thunderforge.dev, which the owner can already see from nginx, and none of
the instances where real games run. Rejected as the stopping point; kept as
the part that was already on.

## Consequences

- The owner sees errors and health from instances the project does not run,
  grouped by an anonymous install id.
- Every addition to the anonymous allow-list is a change to Principle VII's
  promise and is reviewed as one. The lists are constants with tests that
  enumerate them, so an addition cannot slip in through an attribute.
- The project's collector becomes a public intake for metrics as well as
  logs and traces. It needs a metric-name filter and resource-attribute
  stripping (spec 086 open item 7), and without a per-IP rate limit its
  self-hosted numbers are indicative, not exact.
- An operator who redirects receives more than the project ever does. That
  is intended: the anonymous tier limits what leaves an operator's control,
  not what the operator may see of their own instance.
- Half-redirecting (server to the operator, browsers to the project) is
  allowed, and shown as two destinations in the startup line and the admin
  panel.
- Forgetting `TELEMETRY=false` in a new test harness would report test
  traffic. FR-036 names every place it is set, and the e2e harness carries
  it beside the rate-limit bypass.

## Legal Note

This is not legal advice. The default is defensible because of its shape:
what reaches the project identifies no person (no IP address, account, id,
hostname or content), the install id is random, retention is 14 days, and
the operator controls it with one variable in either direction, told so on
first start. Before the server-image default ships to operators in the EU,
the owner checks whether the disclosure needs GDPR legitimate-interest
wording (Art. 6(1)(f)) and whether an operator-facing processing note is
wanted. That is recorded as spec 086 open item 8. It is an open item, not a
blocker.
