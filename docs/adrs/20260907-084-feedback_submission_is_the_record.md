# The Submission Is the Record; the Tracker Is a Destination

- **Date**: 2026-09-07
- **Status**: Accepted
- **Spec**: `specs/037-in-app-feedback/` (FR-018, FR-021, US5)
- **Governs**: what a person is told about their report, what an operator can
  do to one, and what "delivered" is allowed to mean

## The decision

**A report that reached this instance has arrived.** Delivery to a tracker is a
second, separate act that may fail, be abandoned, or never be configured — and
none of that unsends the report.

Three consequences, and they are the whole of the decision:

1. **The author is never shown a delivery failure.** They see *received*, or
   *sent on*. Not "could not be delivered".
2. **"Stop retrying" is not "delete".** An operator can end the attempts. The
   submission stays.
3. **An instance with no destination configured still accepts reports.** They
   queue. Configuring a destination later delivers them.

## Why

The alternative — treating the tracker as the record — fails in the case that
matters most. An instance is most likely to have a broken or missing
destination *early*, which is exactly when the reports being filed are the most
valuable: first-run problems, setup confusion, the things nobody has hit yet.
A product that dropped those, or told people to file them again, would lose its
best feedback at the moment it most needed it.

And "file it again" is what a person does when told delivery failed. That
produces two reports of one problem, no new information, and an operator with
twice the queue — while the thing that actually needs fixing is a credential
the author cannot see and could not change.

## What this costs

An author cannot tell whether their report reached a maintainer. That is a real
loss and it is accepted: the alternative is telling them something they cannot
act on. When delivery does succeed, `issueUrl` appears on their own screen and
they can follow it.

It also means the operator's queue is a genuine obligation rather than a
diagnostic curiosity — reports accumulate there and somebody has to look. The
undelivered view exists for that, and shows attempt counts and reasons so the
looking is cheap.

## Alternatives considered

- **Refuse to accept feedback when no destination is configured.** Rejected:
  it makes a fresh instance silently featureless and loses first-run reports.
- **Show the author the delivery state truthfully, failures included.**
  Rejected for the duplicate-filing reason above. The information is real; the
  author is simply not the person who can act on it.
- **Delete on abandon.** Rejected: it destroys a received report because a
  *forwarding* step failed, which is the precise inversion of this decision.
