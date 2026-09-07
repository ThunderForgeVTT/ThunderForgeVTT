# Contract: Evidence — what is captured, what is removed, what is shown

This is the contract FR-012 is judged against. The claim it makes is narrow and
testable: **the bytes the person sees in the review are the bytes that are
submitted, because they are the same array.** Everything below exists to make
that true rather than to make it sound true.

---

## 1. The log ring buffer

`apps/web/src/services/feedbackLogBuffer.ts`, started from `src/main.tsx`.

```ts
export type LogLevel = "error" | "warn" | "info";
export interface LogEntry { at: number; level: LogLevel; text: string }

export interface LogSnapshot {
  entries: LogEntry[];
  droppedCount: number;
  droppedOldestAt: number | null;
  byteSize: number;
  redactionCount: number;
}

/** Begin capturing. Idempotent; returns a stop function. Called once, from main.tsx. */
export function startLogCapture(): () => void;

/** The current contents. What the review renders AND what is submitted. */
export function snapshot(): LogSnapshot;
```

**Sources captured**: `console.error`, `console.warn`, `console.info`,
`window.onerror`, and `unhandledrejection`. The original console functions are
called through, always, so capture never changes what a developer sees.

**Bounds** — whichever is reached first:

| Bound | Value | Why |
|---|---|---|
| Entries | 500 | Enough for a session's worth of what went wrong |
| Total bytes | 128 KB | Fixed, so a four-hour session costs what a four-second one does |
| Per entry | 2 KB, truncated with a marker | One enormous line must not evict everything that explained it |

Eviction is oldest-first and increments `droppedCount`. **FR-011 requires the
person to be told what was kept**, so `droppedCount` and `droppedOldestAt` are
rendered in the review — a count, not a shrug.

**Never persisted.** Not `localStorage`, not `sessionStorage`, not IndexedDB,
not OPFS. The buffer dies with the tab. A shared machine inherits nothing, and
there is no stored artefact for a later change, a memory dump or a second
reader to find.

---

## 2. Redaction — at capture, before the buffer

`apps/web/src/services/feedbackRedaction.ts`, and the server's
`src/server/src/feedback/redaction.rs`. **Both read the same rule list**,
`config/feedback-redaction.json`, so a rule cannot exist on one side only — a
server-only rule would produce refusals the person could not have anticipated,
and a client-only rule would be no rule at all.

```ts
export interface Redaction { text: string; count: number; kinds: string[] }
export function redact(line: string): Redaction;
```

Applied **at push time**, so the buffer never holds a secret at any instant.

### The rule set

| Kind | Shape | Replacement |
|---|---|---|
| `bearer_token` | `Authorization: Bearer …`, or a bare `Bearer <token>` | `[redacted: bearer token]` |
| `cookie` | `Cookie:` / `Set-Cookie:` headers, and the session cookie's name wherever it appears | `[redacted: cookie]` |
| `jwt` | `xxxxx.yyyyy.zzzzz` with base64url segments | `[redacted: token]` |
| `url_credential` | `token=`, `key=`, `signature=`, `X-Amz-Signature=`, `X-Amz-Credential=` query parameters | `[redacted: signed url]` |
| `aws_key` | `AKIA…` / `ASIA…` access key ids and STS session tokens | `[redacted: storage credential]` |
| `pem` | `-----BEGIN … -----` through `-----END … -----` | `[redacted: private key]` |
| `submitter_email` | The signed-in account's own address, as the client knows it | `[redacted: email]` |

`url_credential` is not hypothetical: `storage/rustfs.rs` mints per-call STS
credentials scoped to one key, and a failing asset fetch logs the URL that
carried them.

Every replacement is **visible**. A removed thing is never simply deleted,
because the person is entitled to see that something was removed and what kind
of thing it was — and because a silent removal is indistinguishable from a
rule that did not fire.

### What is never redacted

**The message the person typed.** spec.md's Edge Cases settle it: "what a
person deliberately types is theirs, and the review step is where they see it."
The server's refusal (below) applies to attachments only.

### The server's half: refuse, never rewrite

`submitFeedback` runs the same rules over every attachment. A match is a
refusal — `FEEDBACK_CONTAINS_SECRET`, naming the kind — not an edit.

This is the rule the whole contract turns on. Editing after approval would mean
the person saw something other than what was sent, which is exactly the failure
FR-012 exists to prevent, even when the edit is an improvement. Refusing also
means a client tampered with to skip redaction cannot post a token through this
path, so the guarantee does not depend on the client behaving.

---

## 3. The screenshot

`apps/web/src/services/feedbackScreenshot.ts`.

```ts
export interface Screenshot { blob: Blob; width: number; height: number }
/** Offers capture. Resolves null when the person declines or it is unavailable. */
export async function captureScreenshot(): Promise<Screenshot | null>;
```

**Mechanism**: `navigator.mediaDevices.getDisplayMedia({ video: true })`, one
frame drawn to an `OffscreenCanvas`, encoded as PNG, **track stopped
immediately** in a `finally` so the browser's sharing indicator never outlives
the capture.

**Why not the canvas**: nothing sets `preserveDrawingBuffer` on the WebGL
context Bevy/winit creates, so `canvas.toDataURL()` returns a blank image.
`apps/web/e2e/canvas-authoring.spec.ts:623-637` and
`apps/engine-sandbox/src/main.ts:67` both record this. A screenshot built that
way would produce an empty rectangle where the map was — the failure mode that
looks like success. See research.md § R7.

**The picker is the consent step.** FR-008 requires that taking one is the
person's choice; a native, non-spoofable dialog in which they choose what to
share is stronger than any checkbox this product could render.

**Failure is not an error.** A refusal, a dismissal, or an absent API all
resolve to `null`, a plain line says the screenshot could not be taken, and
submission proceeds. spec.md's Edge Cases: "offering fails; submitting does
not."

**Server-side**: the PNG goes through the existing
`transcode::transcode_to_webp`, inheriting `MAX_UPLOAD_BYTES` and
`TranscodeError::TooLarge { max, actual }` — which is checked before any decode
work — and is stored as `.webp` like every other image in this product.

---

## 4. The review step

`apps/web/src/components/feedback/FeedbackReview.tsx`. FR-010 and SC-003 are
one component's job.

It renders, and lets the person remove, **every** part:

- the message and summary as typed;
- the context: screen, world and system name, both versions, browser;
- the log bundle **in full and scrollable**, with `entriesKept`,
  `droppedCount`, and the count of redaction markers;
- the screenshot **at a size that can be inspected** (FR-015), with the option
  to drop it and still submit.

Removing a part removes it from the payload, not from the display of the
payload. The e2e assertion for SC-003 removes the screenshot and then asserts
its absence at the destination — a review that hid something while sending it
would pass a rendering test and fail this one.

**The notice sits in this step, not after it** (FR-014): where it is going, how
visible that is (read from the host, with the observation time), and that once
sent it is committed to a repository and cannot be recalled by the instance.

---

## 5. Retention

- The instance keeps attachment bytes for **30 days**, recorded per submission
  as `attachments_expire_at` at submission time — so a later change to the
  constant cannot retroactively shorten what somebody was promised.
- The retention sweep runs on the delivery task's tick, deletes the objects
  under `feedback/` and sets `purged_at`. The rows survive their bytes, so a
  submission can still say what was attached.
- **This is independent of the destination's copy** (FR-016), which is
  permanent and is why FR-014's notice says so before anything is sent.
- `rustfs::delete_object` refuses any key not under `feedback/`. The
  restriction is inside the function, not in its callers, because
  `storage/dedupe.rs` warns that deleting a shared object "would silently blank
  the background of every other scene sharing those bytes" — and a rule kept by
  callers is a rule until somebody adds a caller.
