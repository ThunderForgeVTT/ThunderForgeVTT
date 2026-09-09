# Feedback: what is captured, and what happens to it

Spec 037. A person reports a problem from inside the product; the instance
keeps it; the instance may then send it on to a tracker. Those are three
separate things and the middle one is the one that matters.

## The instance is the record

**A report that reached this server has arrived.** Delivery to a tracker is a
separate step that can fail, be abandoned, or never be configured at all — and
none of that unsends the report.

This is why the author is never shown a delivery failure. They are shown
*received*, or *sent on*. Telling somebody "your report failed to deliver"
invites them to file it again: two reports of one problem, no new information,
and the thing that actually needs doing belongs to the operator.

## What is captured

Only what the person is shown before they send it. There is a review step, and
what it displays is what leaves.

| Piece | Where it comes from |
|---|---|
| The message and kind | Typed |
| The screen, client version, browser | Read from the session |
| Logs | The in-page buffer, **redacted at capture** |
| A screenshot | Only if the person takes one, through the browser's own picker |

### Redaction happens in the client, before the review

`config/feedback-redaction.json` is one list read by both halves. The client
filters as it captures; the server **refuses** a payload that still matches,
and never rewrites one. A server that quietly cleaned up a submission would be
delivering something other than what the person approved, and the review step
would be a lie.

The rule kinds are: PEM blocks, cookies, bearer tokens, JWTs, URL credentials,
AWS keys, and the submitter's own email address.

A refusal names the *kind* it matched — `bearer_token` — and never quotes the
payload, because an error message is one of the places a credential gets logged
next.

## Where attachments live, and for how long

Under `feedback/{submission_id}/{attachment_id}` in this instance's object
store, **never** through the deduplicating path — content-addressed storage
would let one person's expiry keep another person's bytes alive.

**They are kept for 30 days** (`RETENTION_DAYS`), then a sweep deletes the
objects and stamps `attachments_purged_at`. That window is written onto the
submission when it is made, so shortening the constant later cannot
retroactively shorten what somebody was promised.

Once purged, an undelivered report can no longer be sent on — it fails as
`ATTACHMENTS_EXPIRED`, which is reported rather than retried forever.

Reading them back: `GET /api/feedback-assets/{attachment_id}`, for the person
who filed the submission or an administrator, with `no-store` and a sandbox
CSP. One of these objects is a screenshot of somebody's session.

## Delivery, and the failure that matters

The pass ticks every 30 seconds and walks a backoff curve. Two rules are worth
knowing:

1. **Search before create, but only after an *ambiguous* failure.** A 4xx means
   the host declined and created nothing, so searching would spend a request
   for no information. A transport failure or a 5xx means the request may have
   succeeded with the answer lost — and *that* is when the retry searches for
   the delivery key it stamped into the body, so one report cannot become two
   issues.
2. **Attachments are committed before the issue that references them.** The
   other order leaves a broken link for as long as the second request takes,
   and forever if it never lands.

Delivery is exercised end to end against `scripts/github-stub.mjs`, which the
server reaches through `GITHUB_API_BASE` — the same configuration an operator
running GitHub Enterprise sets. There is no test branch in the delivery path.

## What an operator sees, and does not

**Admin → Legal** carries the undelivered queue: attempts, next attempt, and a
closed vocabulary of failure reasons. It shows **no message and no
attachment**, and the server does not send them — an operator debugging their
own credentials should not have to read a bug report to do it.

*Stop retrying* is not *delete*. The submission stays, because a report that
was received is received whether or not it was ever forwarded.

## What a person sees

**Settings → Feedback you have sent**: what was sent, where it went if
anywhere, and the date this instance's copies of the evidence are deleted.

## Before submitting

The dialog says where the report will go and **whether that destination is
public** — determined from the host, never assumed. Unknown renders as a
warning rather than as reassurance: the safe reading of a destination nobody
could check is not "private".
