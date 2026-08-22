# Contract: Compendium NPCs Tab (UI contract — no new GraphQL surface)

This tab introduces **no new GraphQL queries, mutations, or types**. It is a UI-only relocation and reshaping of spec 010's existing NPC catalog capability. This file documents the client-side contract between the Compendium page's components, since that's the only new interface this feature adds.

## Reused GraphQL operations (unchanged, spec 010)

- `worldActors(worldId: ID!): [GraphQLWorldActor!]!` — the NPCs tab's data source (filtered client-side to `isNpc === true`, same as spec 010's `NpcCatalog`).
- `createActor(input: CreateActorInput!): GraphQLWorldActor!` — DM/GM-only, used by the tab's "Add NPC" control.
- `updateActor(input: UpdateActorInput!): GraphQLWorldActor!` — used when the preview panel's "Edit" action is followed through on the full `/world/:id/actor/:id/edit` route (not from the panel itself — the panel only links out).

## Component contract

```text
<WorldCompendiumPage worldId>
  owns: selectedActorId: string | null
  renders: <Tabs> [NPCs, Items, Abilities] </Tabs>

  NPCs tab content:
    <NpcCompendiumTab
      worldId
      onSelect(actorId: string): void      # sets selectedActorId in the parent
      selectedActorId: string | null       # for row-highlight styling
    />
    <ActorPreviewPanel
      actor: WorldActorRecord | null       # looked up from the tab's already-fetched roster
      onClose(): void                       # clears selectedActorId
    />
```

- `NpcCompendiumTab` is `NpcCatalog` (spec 010) with one behavioral change: clicking a row calls `onSelect(actor.id)` instead of navigating via `<Link>`. The row's own inline "View"/"Edit" buttons (spec 010) are preserved as direct-navigation shortcuts that still work independent of selection.
- `ActorPreviewPanel` is presentation-only — it receives an already-resolved `WorldActorRecord` (or `null`) as a prop; it performs no data fetching of its own (research.md §3). It renders: name, description (or "No description" empty state), classification (NPC/PC), actor type, game system (if set); a "View" link to `/world/:id/actor/:id/view` (always shown when an actor is selected); an "Edit" link to `.../edit` (shown only when `actor.myPermissionLevel !== "VIEWER"`, matching `ActorDetailPage`'s existing gating logic).
- When `selectedActorId` refers to a row that search has since filtered out of view, the panel remains open and unchanged (edge case in spec.md) — filtering only affects the table's visible rows, not the selection.

## Placeholder tabs

`ComingSoonTab` takes a single `label: string` prop and renders a static "{label} — coming soon" message. No props, state, or data fetching. Used for Items and Abilities in this pass (FR-008).
