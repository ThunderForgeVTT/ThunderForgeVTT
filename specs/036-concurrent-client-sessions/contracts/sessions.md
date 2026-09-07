# Contract: Sessions a person can see and end

Adds one query and two mutations to the existing GraphQL surface. Every field
here concerns **the caller's own** sessions; there is no field by which one
account can see or end another's, and adding one would be a different feature
with a different review.

```graphql
type UserSession {
  id: UUID!
  createdAt: String!
  lastSeenAt: String!
  expiresAt: String!
  "Coarse origin, e.g. \"Chrome on Linux\". Never an address."
  clientDescription: String
  "True for the session making this request."
  isCurrent: Boolean!
  "True while this session holds the play field."
  holdsPlayField: Boolean!
}

extend type Query {
  "Every live session for the calling account, newest first."
  mySessions: [UserSession!]!
}

extend type Mutation {
  "End one of the caller's own sessions. Ending the current one signs it out."
  endSession(sessionId: UUID!): Boolean!

  "End every session for the calling account, including this one."
  endAllSessions: Boolean!
}
```

## Rules

1. `mySessions` returns only live sessions — not revoked, not expired.
2. `endSession` accepts only a session belonging to the caller. A session id
   belonging to somebody else returns the same result as one that does not
   exist: no field here may be used to discover whether a session id is real.
   This follows spec 027's FR-011 rule that a dead link gives one message
   whatever the cause.
3. `endAllSessions` includes the caller's own session and is the control a
   person uses after losing a device.
4. Ending a session closes the live streams it holds (FR-010) and releases the
   play-field claim if it held one.
5. A password change ends every session except the acting one and needs no
   field here — it happens on the existing password-change path (FR-008).

## What is deliberately absent

- No "end all other sessions" convenience. `mySessions` plus `endSession` says
  it precisely, and a third mutation would be a third thing to keep correct.
- No IP address, geolocation or device fingerprint, on the same reasoning
  spec 035 used for access events: the record describes the act, not the
  person.
