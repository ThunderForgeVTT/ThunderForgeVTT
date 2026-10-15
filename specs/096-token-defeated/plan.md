# Implementation Plan: A Token Is Down

**Branch**: `096-token-defeated` | **Date**: 2026-10-15 | **Spec**: [spec.md](./spec.md)

## Summary

There is a new `plugins/token_defeated.rs`, shaped like
`plugins/condition_markers.rs`:

- a `TokenDefeated(bool)` component, inserted from the payload in both the
  update and spawn arms of `app.rs`;
- a `DrawnDefeated` marker;
- the skull built from sprites as child entities (`DefeatMark`);
- the art's `Sprite.color` dimmed while down and put back while up;
- a mutex snapshot mirrored for the `token_defeated()` export.

## Technical Context

- Rust, Bevy 0.20, the engine crate only. No new dependencies.
- The skull is built from rectangles, the way badges are:
  - a cranium;
  - a jaw;
  - two dark eye sockets;
  - two crossed bones with knobbed ends, turned ±45°.
- Its z is 5.6: above `MARKER_Z` 5.5, below the nameplate.
- The tint applies only to tokens with art. Swatch tokens are dimmed the
  same way through their colour. The original colour is kept in
  `DrawnDefeated` and restored from there.

## Files

- `crates/thunderforge-engine/src/payloads.rs`: the `defeated` field.
- `crates/thunderforge-engine/src/app.rs`: insert the component in both arms.
- `crates/thunderforge-engine/src/plugins/token_defeated.rs` and
  `token_defeated_tests.rs`: new.
- `crates/thunderforge-engine/src/plugins/mod.rs` and `startup.rs`:
  register the plugin.
- `crates/thunderforge-engine/src/lib.rs`: the `token_defeated` export.

## Risks

- `token_grid::size_tokens_to_grid` and the art loader write `Sprite`. The
  dim has to be reapplied after a new image arrives. Set `color` in a system
  ordered after the loader, compare against the wanted value, and write
  only on a difference.
