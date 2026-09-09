# Feedback Attachments Are Stored Unshared, So They Can Expire

- **Date**: 2026-09-07
- **Status**: Accepted
- **Spec**: `specs/037-in-app-feedback/` (FR-016), `contracts/attachments.md` § 5
- **Governs**: where a feedback attachment is written, and why not through the
  path everything else uses

## The decision

Feedback attachments are stored at `feedback/{submission_id}/{attachment_id}`
and **never** through `storage::dedupe::object_holding`, which every other
uploaded asset uses.

They are kept for 30 days, then deleted. The window is stamped onto the
submission when it is made.

## Why not the deduplicating path

Because deduplication makes deletion unsafe. Content-addressed storage means
one object can be referenced by several rows, so deleting it on one row's
expiry could remove bytes another row still points at — and *not* deleting it
means the expiry is a lie.

Feedback is the only content in this product with a promised end date. Every
other asset is deleted when its owner deletes it, which the dedupe layer
handles by refcount. A time-based expiry does not fit that model, and bending
the dedupe layer to fit would put a deletion path into the layer whose entire
job is to avoid one.

Storing unshared costs duplicate bytes when two people attach identical logs.
That is a rounding error against a 30-day window, and it buys a delete that is
simply a delete.

## Why 30 days, stamped rather than computed

Stamped, because shortening the constant later must not retroactively shorten
what somebody was already promised. The value on the row is what the person was
told before they submitted; a computed window would silently move under them.

30 days is long enough to survive an outage, a holiday and a slow triage, and
short enough that this instance is not an indefinite archive of screenshots of
other people's sessions.

## What this costs

**A report can outlive its evidence.** A submission whose attachments expire
before delivery succeeded fails permanently as `ATTACHMENTS_EXPIRED` — the
report survives (ADR-084) but can no longer be sent on. That is visible in the
operator's queue with its expiry date, which is the only mitigation available:
somebody has to look before the window closes.

## Alternatives considered

- **Dedupe with refcounting and time-based sweep.** Rejected: it puts a
  deletion path into the layer built to avoid one, for a rounding error in
  bytes.
- **No expiry.** Rejected: an instance would accumulate screenshots of user
  sessions forever, which is a liability nobody asked for.
- **Expire on delivery.** Rejected: delivery may never happen, and the evidence
  is exactly what makes an undelivered report worth keeping.
