# Phase 1 Data Model: DMCA Notice-and-Takedown Process

## ContentModerationAction

The single record type covering a takedown notice, its resolution, and any counter-notice — one row per lifecycle event, linked by `case_id` so a notice and its eventual counter-notice/resolution form a queryable thread.

| Field | Type | Notes |
|---|---|---|
| `id` | UUID, PK | |
| `case_id` | UUID | Groups all events (notice → counter-notice → resolution) belonging to one takedown case |
| `action_type` | enum: `notice_received`, `notice_rejected_incomplete`, `content_disabled`, `counter_notice_received`, `counter_notice_forwarded`, `content_restored`, `content_remains_disabled` | Append-only event log, not a mutable status field — preserves the full history required for repeat-infringer evaluation and audit |
| `entity_type` | enum: `world_actor`, `world_item`, `world_lore_entry` | Polymorphic reference target; extensible without migration as new compendium content types are added (see research.md R1) |
| `entity_id` | UUID | FK-less reference to the content row (see FR-013 / R1 — must outlive the referenced row) |
| `world_id` | UUID | Denormalized for compliance-staff querying/audit without joining through the entity; not a FK-cascade source |
| `account_id` | UUID, nullable | The world/content owner's account, denormalized at time of action for repeat-infringer tracking even if the account is later deleted |
| `claimant_name` | text | From the notice |
| `claimant_contact` | text | Email or mailing address provided by claimant |
| `copyrighted_work_description` | text | Statutory element: identification of the copyrighted work claimed to be infringed |
| `infringing_material_location` | text | Statutory element: identification/location of the allegedly infringing material — must resolve to a specific `entity_type`+`entity_id`, not a whole world |
| `good_faith_statement` | boolean | Statutory element, must be affirmed true to be valid |
| `accuracy_statement` | boolean | Statutory "under penalty of perjury" element, must be affirmed true to be valid |
| `signature` | text | Physical or electronic signature string |
| `validity_result` | enum: `valid`, `invalid_missing_elements`, `pending_review` | Set after FR-003 validation |
| `missing_elements` | text[], nullable | Populated when `validity_result = invalid_missing_elements`, returned to submitter |
| `counter_notice_id` | UUID, nullable, self-referencing `case_id` | Links a `counter_notice_received` event back to its case |
| `restoration_due_at` | timestamptz, nullable | Set on `counter_notice_forwarded`; when reached with no further claimant action, `content_restored` is auto-generated |
| `created_at` | timestamptz | |
| `created_by` | UUID, nullable | Internal compliance staff who processed the event, if applicable (public claimant-submitted events have no `created_by`) |

**Validation rules** (from FR-003, FR-006):
- A `notice_received` event MUST have all of: `copyrighted_work_description`, `infringing_material_location` (resolving to exactly one `entity_type`+`entity_id`), `claimant_contact`, `good_faith_statement = true`, `accuracy_statement = true`, `signature` non-empty — otherwise it is recorded as `notice_rejected_incomplete` with `missing_elements` populated (never silently dropped, so there's a record of every submission attempt).
- A `counter_notice_received` event MUST have all of: identification of the removed material and its pre-removal location, a good-faith mistake/misidentification statement, consent-to-jurisdiction affirmation, and contact information — same accept/reject pattern.

**State transitions** (per `case_id`):

```
notice_received (valid) → content_disabled
                              │
                              ├─→ [no counter-notice] → (terminal — content stays disabled)
                              │
                              └─→ counter_notice_received → counter_notice_forwarded (sets restoration_due_at)
                                                                  │
                                                                  ├─→ [restoration_due_at reached, no further claimant action] → content_restored
                                                                  └─→ [claimant files further action before due date] → content_remains_disabled
```

## ModerationVisibility (derived, not a stored table)

A read-time projection, not a persisted entity: for any `(entity_type, entity_id)`, "is this entry currently moderation-disabled?" is answered by checking whether its `case_id`'s most recent event is `content_disabled` or `content_remains_disabled` (vs. `content_restored`, or no case at all). Computed at the GraphQL resolver boundary per research.md R2 — never cached client-side as a trust boundary.

## RepeatInfringerEvaluation (derived, not a stored table)

Per `account_id`, a count of distinct `case_id`s whose latest event is `content_disabled` or `content_remains_disabled`, within the policy's configured lookback window (see research.md R3 — window length is a config value, not hardcoded). Crossing the published threshold (also config) flags the account for the review/termination process defined in the platform's published repeat-infringer policy (FR-009).

## Relationship to existing entities

- `world_actors`, `world_items`, and the lore-wiki entity from spec `012-lore-wiki` are **referenced, not modified** in schema — no new columns added to those tables. Their read paths (resolvers/repositories) gain a moderation-visibility check per research.md R2.
- `world_id` and `account_id` on `ContentModerationAction` are **not** foreign keys with cascading delete — per FR-013, moderation history must survive deletion of the world or account it references. They are plain UUID columns, denormalized at write time.
