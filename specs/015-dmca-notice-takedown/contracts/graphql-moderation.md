# Contract: Content Moderation / DMCA (new)

## Shape

```graphql
enum ModerationEntityType {
  WORLD_ACTOR
  WORLD_ITEM
  WORLD_LORE_ENTRY
}

enum ModerationActionType {
  NOTICE_RECEIVED
  NOTICE_REJECTED_INCOMPLETE
  CONTENT_DISABLED
  COUNTER_NOTICE_RECEIVED
  COUNTER_NOTICE_FORWARDED
  CONTENT_RESTORED
  CONTENT_REMAINS_DISABLED
}

type GraphQLModerationAction {
  id: ID!
  caseId: ID!
  actionType: ModerationActionType!
  entityType: ModerationEntityType!
  entityId: ID!
  worldId: ID!
  validityResult: String
  missingElements: [String!]
  restorationDueAt: String
  createdAt: String!
}

type GraphQLModerationCase {
  caseId: ID!
  entityType: ModerationEntityType!
  entityId: ID!
  worldId: ID!
  currentStatus: ModerationActionType!    # latest event's actionType
  events: [GraphQLModerationAction!]!
}

input SubmitTakedownNoticeInput {
  entityType: ModerationEntityType!
  entityId: ID!
  claimantName: String!
  claimantContact: String!
  copyrightedWorkDescription: String!
  infringingMaterialLocation: String!
  goodFaithStatement: Boolean!
  accuracyStatement: Boolean!
  signature: String!
}

input SubmitCounterNoticeInput {
  caseId: ID!
  removedMaterialDescription: String!
  goodFaithMistakeStatement: Boolean!
  consentToJurisdiction: Boolean!
  contactInformation: String!
  signature: String!
}

type Mutation {
  # Public — no auth required. Rejects with validationErrors (not a GraphQL error)
  # when statutory elements are missing (FR-003); never silently drops a submission.
  submitTakedownNotice(input: SubmitTakedownNoticeInput!): GraphQLModerationCase!

  # Requires the caller to be the owning GM/account for the case's world.
  submitCounterNotice(input: SubmitCounterNoticeInput!): GraphQLModerationCase!

  # Compliance-staff-only. Manually resolves a case outside the automatic
  # restoration-timer path (e.g. claimant filed further legal action).
  resolveModerationCase(caseId: ID!, resolution: ModerationActionType!): GraphQLModerationCase!
}

type Query {
  # Resolver-boundary check used by content read paths (world_actors, world_items,
  # world_lore_entry resolvers) per research.md R2 — not typically called directly
  # by clients; documented here because it defines the enforcement contract every
  # content resolver must honor.
  moderationStatus(entityType: ModerationEntityType!, entityId: ID!): ModerationActionType

  # Compliance-staff-only.
  moderationCase(caseId: ID!): GraphQLModerationCase
  moderationHistoryForAccount(accountId: ID!): [GraphQLModerationCase!]!
  repeatInfringerFlags: [ID!]!   # account IDs currently over threshold
}
```

## Enforcement contract (non-negotiable, per Constitution Principle III)

Every existing and future GraphQL resolver that reads a `world_actor`, `world_item`, or `world_lore_entry` (single-entity or list) MUST check `moderationStatus` for that entity and, when the result is `CONTENT_DISABLED` or `CONTENT_REMAINS_DISABLED`:
- **List queries**: exclude the entity entirely.
- **Single-entity queries**: return a moderation placeholder (entity `id` + a `moderated: true` flag) instead of the real field values, to every caller including the owning GM — the web client renders `ModeratedContentBanner` in this case, with a counter-notice CTA if the caller is the owner.

This check happens at the same server-side boundary as existing ownership/permission checks (ADR-009, ADR-013, ADR-023, ADR-028) — never left to the client.
