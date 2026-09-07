# Contract: Submitting feedback, and seeing what became of it

One mutation and two queries. Every field here concerns **the caller's own**
submissions; there is no field by which one account can read another's, and the
operator surface in `delivery.md` is a separate object with an administrator
check rather than a wider version of these.

```graphql
enum FeedbackKind {
  FEATURE_REQUEST
  ISSUE
  GENERAL
}

"""
An attachment the person has approved. The bytes are inline because the review
step showed exactly these bytes and nothing may re-derive them server-side —
see research.md § R6.
"""
input FeedbackAttachmentInput {
  kind: FeedbackAttachmentKind!
  "Base64. Logs are UTF-8 text; a screenshot is a PNG the client encoded."
  content: String!
  "What the client's ring buffer kept and dropped. Logs only."
  entriesKept: Int
  entriesDropped: Int
  "How many redaction markers the client's filter left. Logs only."
  redactionCount: Int
}

enum FeedbackAttachmentKind { LOGS, SCREENSHOT }

input SubmitFeedbackInput {
  kind: FeedbackKind!
  "Required for every kind. What the person typed; never redacted."
  message: String!
  "A one-line title. Required for ISSUE and FEATURE_REQUEST, absent for GENERAL."
  summary: String
  "The route the person was on. Absent is valid — an error state may have none."
  screenPath: String
  "Absent on any screen with no world. The game system is NOT accepted here."
  worldId: UUID
  "The client build, from the Vite define. \"unknown build\" when absent."
  clientVersion: String!
  "Coarse browser family and platform. Never the full User-Agent, never an address."
  browser: String!
  "Only what the person approved in the review. An empty list is normal."
  attachments: [FeedbackAttachmentInput!]!
}

type FeedbackSubmission {
  id: UUID!
  kind: FeedbackKind!
  message: String!
  summary: String
  createdAt: String!
  "PENDING until delivered; the person is never shown a failure (FR-018)."
  deliveryState: FeedbackDeliveryState!
  "Present once delivered or adopted."
  issueUrl: String
  "OPEN or CLOSED, as last read back. Null before delivery."
  issueState: FeedbackIssueState
  "When that state was last observed. Shown with it, never without it."
  issueStateCheckedAt: String
  attachments: [FeedbackAttachmentSummary!]!
  "When this instance's copies of the attachments expire. The destination's copy is unaffected."
  attachmentsExpireAt: String!
}

enum FeedbackDeliveryState { PENDING, DELIVERED, ABANDONED }
enum FeedbackIssueState { OPEN, CLOSED }

type FeedbackAttachmentSummary {
  id: UUID!
  kind: FeedbackAttachmentKind!
  byteSize: Int!
  entriesKept: Int
  entriesDropped: Int
  redactionCount: Int!
  "Set once this instance's copy has been removed. The row outlives its bytes."
  purgedAt: String
}

"""
What the person must be told before they submit. Read at form-open time so the
notice is on screen while they decide, not after.
"""
type FeedbackDestinationNotice {
  "False when nothing is configured. Submission still works (FR-030)."
  configured: Boolean!
  "Owner/name, when configured. Shown so a person knows where it is going."
  repository: String
  "Determined from the host, never assumed. Null means never successfully checked."
  isPublic: Boolean
  "When that was observed. Rendered with it — visibility changes without telling us."
  visibilityCheckedAt: String
}

extend type Query {
  "Everything the calling account has submitted, newest first."
  mySubmissions: [FeedbackSubmission!]!

  "The pre-submission notice. Requires a session; discloses no credential."
  feedbackDestinationNotice: FeedbackDestinationNotice!
}

extend type Mutation {
  """
  Record a submission. Returns after the database write and before any
  delivery attempt (FR-018), so this never fails because GitHub is down.
  """
  submitFeedback(input: SubmitFeedbackInput!): FeedbackSubmission!
}
```

## Rules

1. **The mutation records and returns.** It writes the submission, its
   attachment rows and its stored objects in one transaction, and it does not
   call the destination. FR-018 is this rule and nothing else.
2. **A session is required.** Anonymous submission is out of scope by spec.md's
   Assumptions, and the resolver refuses without one — it does not fall back to
   an IP-keyed path.
3. **`gameSystemId` is not an input.** The server resolves it from `worldId`.
   A client that could name the system could name a different world's.
4. **`attachments` are taken as given, validated, and never regenerated.** The
   server runs the redaction rule set over them (`attachments.md`) and
   **refuses** on a match with `extensions.code = "FEEDBACK_CONTAINS_SECRET"`.
   It never edits an attachment: the person approved these bytes, and different
   bytes arriving is the broken promise FR-012 exists to prevent.
5. **Rate limited per account** at 5 per 10 minutes, refused with
   `extensions.code = "FEEDBACK_RATE_LIMITED"` and a message saying when to
   retry. The client keeps the draft — FR-006's second clause is a client
   behaviour with its own e2e assertion, not a side effect of the error.
6. **`mySubmissions` filters on the caller's `user_id`.** There is no argument
   by which it could return another account's rows, and adding one would be a
   different feature with a different review (FR-022).
7. **`feedbackDestinationNotice` discloses nothing sensitive.** It reads the
   destination row, which has no credential column by design. When nothing is
   configured it says `configured: false` and the form still submits.
8. **Attachment bytes are never returned by these fields.** A summary carries
   sizes and counts; the bytes are fetched from
   `GET /feedback-assets/{attachment_id}`, which is wrapped in
   `require_authenticated_user` like every other asset route in this product
   and additionally checks that the caller owns the submission or is an
   administrator.

## Refusal shapes

| Situation | Result |
|---|---|
| No session | Refused as any authenticated mutation is |
| Sixth submission in ten minutes | `FEEDBACK_RATE_LIMITED`; the draft survives; the message says when |
| An attachment contains a secret shape | `FEEDBACK_CONTAINS_SECRET`, naming the kind found; nothing is written; the draft survives |
| A screenshot larger than `MAX_UPLOAD_BYTES` | Refused by the existing `TranscodeError::TooLarge { max, actual }` path, before any decode work |
| `worldId` names a world the caller is not a member of | The field is dropped and the submission succeeds — a report about a world you have left is still a report, and refusing it would lose feedback to protect nothing |
| No destination configured | Succeeds. `deliveryState: PENDING`, forever if need be (FR-030) |
| The destination is unreachable | Succeeds. The person is told it arrived, because it did — at the instance, which is what FR-018 makes the record |

## What is deliberately absent

- **No `deleteFeedback`.** A person cannot recall what has been delivered
  (FR-014 says so before they submit), and a mutation that deletes the
  instance's copy while the issue stands would be a control that appears to do
  something it cannot.
- **No editing a submitted item.** The tracker is where the work happens;
  spec.md's Out of Scope says so.
- **No thread, no reply, no comment.** US5 reports state. A support inbox is a
  different product.
- **No `feedbackAttachmentContent` field.** Bytes go over an asset route with a
  content type, not through a GraphQL string, matching every other asset in
  this codebase.
