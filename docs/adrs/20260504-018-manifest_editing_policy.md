# ADR-018: Manifest Editing Policy

**Status:** Accepted

**Decision Date:** 2026-05-04

## Context

Phase 2 introduces a manifest viewer and editor for MVP metadata such as realm naming and interface pack selection. There was no existing persisted manifest file, so the admin surface needs a controlled persistence strategy that does not turn into arbitrary file editing.

## Decision

ThunderForgeVTT will persist system manifest metadata in `config/manifest.json` beneath the configured data root and only allow a small whitelist of editable keys through the admin API.

- the server creates the manifest file if it does not exist
- schema-version metadata remains read-only
- mutations edit one manifest key at a time through server validation

## Consequences

### Positive

1. Manifest editing becomes durable without adding a new service or table.
2. A whitelist keeps the MVP editor predictable and safe.
3. Operators can change supported settings without manual file edits on disk.

### Negative

1. The manifest is not a general-purpose configuration store.
2. New editable keys require explicit server allowlisting.

## Alternatives Considered

1. **Store manifest values in PostgreSQL** — rejected because the feature is file-oriented metadata and a small JSON document is sufficient for this phase.
2. **Allow arbitrary JSON edits from the UI** — rejected because it makes validation, compatibility, and security much harder.

## Security Implications

- Only whitelisted keys may be changed from the UI.
- Read-only keys protect versioning and internal contract metadata from accidental admin edits.
- Manifest persistence stays under the app-managed config directory rather than user-supplied paths.
