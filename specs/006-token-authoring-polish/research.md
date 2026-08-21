# Phase 0 Research: Token Authoring Polish

## 1. `token.rs`'s current shape vs. `WallPlugin`/`ShapePlugin`'s established pattern

**Decision**: Restructure `src/engine/src/plugins/token.rs` (currently 18 lines — just seeds an empty `TokenCache` on `Startup`) into a real plugin following `WallPlugin`'s exact shape (`src/engine/src/plugins/wall.rs`, 44 lines): `init_resource` calls for whatever token-specific resources are needed (a `SelectedToken`-equivalent already exists per spec 004; confirm/reuse), then a single `.chain()`ed `Update` system tuple: input/drag handling, resize-handle drag, rotate-handle drag, undo (if applicable), visual sync (handle spawn/despawn + sprite sync).

**Evidence**: `wall.rs`'s own doc comment states the exact independence contract Principle II requires: "Independently addable/removable... nothing outside this plugin depends on walls existing." Today, token drag/resize/rotate logic instead lives in `src/engine/src/systems/selection.rs` (`handle_token_drag`, `handle_token_resize_rotate_keyboard` from spec 004) with no equivalent plugin wiring it together — `token.rs` and `selection.rs` are two disconnected pieces where walls/shapes have one cohesive plugin each.

**Rationale**: This is exactly the state spec 004's tasks.md flagged T011 as blocking cleanly for — building real handle sprites (this spec's actual goal) on top of scattered, non-plugin-owned systems would perpetuate the same structural debt Constitution Principle II exists to prevent, per `wall.rs`'s own stated rationale.

**Migration approach**: Move (not copy) `handle_token_drag` and `handle_token_resize_rotate_keyboard` from `selection.rs` into a new `src/engine/src/systems/token.rs`, alongside new `handle_token_resize_drag`/`handle_token_rotate_drag` systems (research.md §2) and a new `sync_token_visuals` (handle spawn/despawn, mirroring `sync_wall_visuals`'s pattern at `wall.rs:577-643`). `token.rs` (the plugin file) then chains all of them, mirroring `WallPlugin::build`.

## 2. Resize/rotate handle sprites: mirror `WallHandle`'s marker + rebuild-each-pass pattern

**Decision**: Two new marker components, `TokenResizeHandle`/`TokenRotateHandle` (or a single `TokenHandle(HandleKind)` component, implementation's call), spawned/despawned each visual-sync pass exactly like `WallHandle` (`wall.rs:47`, spawn/despawn logic at `wall.rs:627-643`) — GM-gated (`is_gm.0` check, same as walls), positioned relative to the selected token's current `Transform` (corner offsets for resize, a point offset along the current facing for rotate).

**Evidence**: `wall.rs:627-643`'s exact pattern: despawn all existing handle entities every pass, then conditionally respawn for the current `selected_wall`/GM state. `shape.rs`'s corner-resize handles follow the same shape for a different geometry (rectangular corners vs. wall endpoints).

**Rationale**: Reusing this exact rebuild-each-pass approach (rather than trying to diff/update existing handle entities) matches the codebase's established convention and avoids introducing a new handle-lifecycle pattern for tokens alone — token counts are small enough that this isn't a hot path, per `wall.rs`'s own comment justifying the same tradeoff.

**Drag input**: New systems `handle_token_resize_drag`/`handle_token_rotate_drag`, structured like `handle_wall_input`'s `WallDragMode` state machine (`wall.rs:55-78`, `152` onward) but scoped to whichever handle marker was clicked — `TokenDragMode::Resizing`/`Resizing`/`Rotating` (or reuse/extend spec 004's existing token drag-mode concept if one exists in `selection.rs` already; confirm at implementation time). Resize drag continues to snap to whole grid-cell increments (reusing the existing `MIN_TOKEN_SCALE`/`MAX_TOKEN_SCALE` clamp and integer-step logic from spec 004's keyboard handler, now driven by drag distance instead of key presses). Rotate drag computes angle continuously from cursor position relative to the token center, replacing the keyboard handler's fixed 30° step for the drag path (the keyboard shortcuts, if kept per spec.md's Assumptions, keep their existing fixed-step behavior unchanged).

## 3. TokenPanel's Popover dismissal race: known partial fix, real root cause still open

**Decision**: Treat this as requiring live instrumentation (React DevTools Profiler or temporary render/effect logging) to isolate, not a guessable code-review fix — two real, confirmed fixes already landed (spec 004's final session) without fully resolving it, which itself is evidence this needs empirical isolation rather than a third guess.

**Evidence**: `apps/web/src/components/TokenPanel.tsx`'s ownership popover (`Popover.Root`/`Popover.Content`, per spec 004's tasks.md T029/T030 notes) already had two real bugs fixed: (1) the primary-checkbox lacked optimistic local state, causing a visible revert-then-snap; (2) the test's `.blur()` on the owner-input field moved focus to `document.body`, which Radix's default outside-focus dismissal treated as "left the popover" — fixed by using `Tab` instead. After both fixes, `primaryCheckbox.check()` still hangs the full test timeout in `apps/web/e2e/token-authoring.spec.ts`'s `test.skip`-ed test. spec 004's tasks.md records two unconfirmed candidate causes: the `refresh()`-triggered tokens-list re-render possibly remounting `Popover.Root` (if list item identity/order shifts across the refetch), or a timing interaction between the new optimistic-update re-render and the subsequent `refresh()`-triggered re-render.

**Rationale**: Given two prior fix attempts each addressed a real, confirmed cause without fully resolving the symptom, a third blind attempt has a low prior of success. Instrumenting the actual render sequence (React DevTools Profiler's "why did this render" / `console.trace` in `Popover.Root`'s `onOpenChange` callback, if Radix's Popover exposes one, to see exactly what triggers the close) is the appropriate research method here, not further black-box Playwright reruns.

**Alternatives considered**: Rewriting the ownership-assignment UI to avoid a Popover entirely (e.g. a modal dialog, or inline row-editing without a floating panel) — deferred as a fallback only if direct instrumentation doesn't yield a targeted fix within a reasonable time-box; the Popover pattern is otherwise consistent with this component's existing UI and shouldn't be abandoned without first understanding what's actually happening.

## 4. Quickstart full walkthrough (T039 equivalent): what's already covered vs. what needs a fresh pass

**Decision**: Most of spec 004's quickstart.md Scenarios 1-3 are already covered by automated Playwright tests written across its implementation sessions (`token-authoring.spec.ts`'s 7 passing tests). This spec's User Story 2 fix (once it un-skips the 8th test) makes the automated suite's full run *itself* the connected walkthrough spec 004's T039 asked for — a separate fully-manual click-through is not needed as a distinct deliverable, only confirmed as a byproduct of SC-003/SC-004 in this spec.

**Rationale**: Avoids treating "run the quickstart" as a separate, redundant manual QA pass when the automated suite already exercises every scenario it describes; the only genuinely new verification this spec adds is confirming the previously-skipped test now passes as part of that same full run.
