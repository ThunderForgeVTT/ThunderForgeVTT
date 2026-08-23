# Quickstart: System Pack Legal & Attribution Compliance

## Prerequisites

- Server running locally with the updated `packs/systems/dnd5e/system.json` (containing a populated `legal` object, see data-model.md's example).
- Web app running locally with `GameSystemContext`/`SystemManifest` updated to include `legal`.

## Scenario 1 — Manifest validation rejects a pack missing `legal` (User Story 2, FR-007)

1. Take a copy of `packs/systems/dnd5e/system.json` and remove the `legal` key entirely.
2. Attempt to load it through the pack loader/validator (`cargo test` against `validate_legal`, or boot the server pointed at the modified pack directory).
3. **Expect**: load fails with a clear "missing `legal`" validation error; the pack does not become available to select for a world.
4. Restore `legal` but empty out `attributionText` to `""`.
5. **Expect**: load fails with an "empty `legal.attributionText`" validation error (SC-003).

## Scenario 2 — GM sees the correct notice at system-selection time (User Story 1)

1. As a GM, start creating a new world (or open an existing world's system-change flow).
2. Select "5E System Core" (the `dnd5e` pack).
3. **Expect**: before/at confirming the selection, `SystemLegalNotice` renders the pack's `attributionText` and `disclaimer` from `research/system_dnd5e.json`'s `legal` object (data-model.md).
4. Confirm the selection completes normally afterward — the notice is informational, not a blocking gate requiring explicit re-consent (unless product later decides otherwise; not required by this spec).

## Scenario 3 — Notice remains reachable afterward (User Story 1, FR-005)

1. From an existing world already using the `dnd5e` pack, navigate to world settings.
2. Open the new `SystemSettingsPanel` (or equivalent existing settings location).
3. **Expect**: the same `SystemLegalNotice` content from Scenario 2 is displayed here too, independent of when the world was created.

## Scenario 4 — Two different systems show two different notices (User Story 1, Scenario 3)

1. Using two test worlds on two different system packs (e.g. `dnd5e` and any second pack with a populated `legal` object, even a minimal placeholder for this test), open each world's settings panel.
2. **Expect**: each shows its own `attributionText`/`disclaimer`/`trademarkRestrictions` — no bleed-through of one system's notice into the other.

## Scenario 5 — Author a `legal` object for a new hypothetical pack from the contract alone (User Story 2)

1. Without reading any implementation source, read only `contracts/manifest-legal-schema.md`.
2. Author a `legal` object for a made-up test system pack.
3. **Expect**: the object validates against Scenario 1's validator without needing any additional undocumented fields.

## Manual verification checklist

- [ ] `dnd5e`'s `system.json` has a `legal` object matching (or improving on) `research/system_dnd5e.json`'s.
- [ ] ADR 027 documents the `legal` field's shape and rationale (FR-002).
- [ ] `tsc`/build passes with the new `SystemManifestLegal` type.
- [ ] Native `cargo check`/`cargo test` passes for the server-side validator addition.
