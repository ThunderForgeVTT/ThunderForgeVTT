# Redaction Is a Capture-Time Client Filter; the Server Refuses, Never Rewrites

- **Date**: 2026-09-07
- **Status**: Accepted
- **Spec**: `specs/037-in-app-feedback/` (FR-012, US2), `contracts/attachments.md`
- **Governs**: where secrets are removed from a feedback bundle, and what the
  server does when it finds one anyway

## The decision

**The client redacts as it captures.** Log lines are filtered before they enter
the buffer, so what the review screen shows is what will be sent.

**The server refuses.** If a submitted payload still matches a rule, the whole
submission is rejected with `FEEDBACK_CONTAINS_SECRET` naming the *kind*
matched. The server never edits the payload.

Both halves read one list: `config/feedback-redaction.json`.

## Why the server refuses instead of cleaning up

Because the review step is a promise. A person is shown exactly what will
leave, and they approve it. If the server then rewrote the bundle, what was
delivered would not be what was approved — and the review would be theatre.

Refusal is also the only honest answer to a bundle the client should have
filtered. Reaching the server unredacted means the client's filter did not run
or does not match what the server matches; both are bugs, and quietly patching
the output would hide them permanently.

## Why the client filters at capture rather than at send

A secret that never enters the buffer cannot leak from it. The buffer lives in
the page for the length of a session and can be read by anything else running
there; filtering at send would leave a window in which the unfiltered text
exists in memory for no reason.

## Why one list, in a config file

The client and the server must agree, and two implementations of "what a
secret looks like" drift toward whichever was edited last. A file both read
makes disagreement impossible rather than unlikely.

## What this costs

**False positives are the client's problem to survive.** A redacted marker like
`[redacted: bearer token]` contains the words the bearer rule matches, so a
correctly filtered bundle can look like an unfiltered one to a naive server
check. That case is a test rather than a footnote, because it would silently
refuse every properly filtered submission.

The rule list is also a closed set — seven kinds — and will miss a secret shape
nobody enumerated. That is accepted over a cleverer heuristic, which
`research.md` § R15 records rejecting for the same reason the settings
validators did: a smarter matcher refuses somebody's real data.

## Alternatives considered

- **Server-side scrubbing.** Rejected: breaks the review's promise.
- **Client-side only, server trusts it.** Rejected: the client is the
  untrusted half, and FR-012 is not a courtesy.
- **Entropy-based detection.** Rejected: unexplainable refusals, and a
  base64-encoded map is high entropy too.
