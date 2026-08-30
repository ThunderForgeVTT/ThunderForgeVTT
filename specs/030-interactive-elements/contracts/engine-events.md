# Contract: The Engine Dispatch Seam

How an activation crosses from the interaction plugin to the subsystem that
performs it, inside the engine.

## The event

`InteractionPlugin` writes one Bevy event per activation, carrying the effect
identifier, its configuration, the subject, and who activated it. It is written
only after permission has resolved.

Contributing plugins add a system that reads these events and handles the
identifiers they declared, ignoring the rest.

## Why an event and not a call

Constitution II: cross-plugin communication happens through Bevy events or
shared resources, never through direct calls into another plugin's private
systems. It is also the only shape under which `InteractionPlugin` compiles and
runs with every contributor removed — FR-039, tested by US7.

A handler needs `Commands` and arbitrary `Query` access to do its work, which a
boxed trait object cannot express without a service locator that fights the
ECS. "Several systems care that a thing happened" is what an event is for.

## Ordering

- Contributors run in the same schedule stage; two contributors handling the
  same event is a collision, prevented at registry assembly (FR-042).
- An effect that changes canvas state does so through the owning plugin's
  existing systems — a door effect sets door state the way the wall plugin
  already does, rather than reaching into wall geometry itself.

## What the interaction plugin owns

Placement and hit-testing, trigger detection (click, and region entry by
comparing previous against current containment), permission resolution,
`once` bookkeeping, and writing the event.

## What it must not own

Any knowledge of what an effect does. The textual check is part of the
contract: the words "light", "door" and "sound" do not appear in its logic.

## Region entry

Detected on token movement, comparing previous and current containment, so
entry fires once per crossing rather than continuously while inside (FR-030).

Movement while the scene is being prepared does not fire (FR-032). A GM
dragging a token in preparation and in play is the same gesture, so the
distinction cannot be inferred — the engine must be told the scene's mode. That
signal is part of this contract, not an implementation detail to settle later.

## What crosses to the server

Nothing directly. The engine dispatches locally for responsiveness, and the
authoritative change is the server mutation that produced the activation. The
engine is not a second authority on whether a door may open — Principle III —
and a client whose optimistic dispatch disagreed with the server's answer
reconciles to the server.
