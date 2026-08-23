// Spec 013 (T049, User Story 5): read-only shared-item preview + "Copy to
// World" picker, mirrors pages/actor-share/SharedActorPage.tsx. Placeholder
// shell wired up by Foundational routing (T014) — full implementation
// lands in Phase 7 (US5).

export default function SharedItemPage() {
  return (
    <main className="grid min-h-screen place-items-center bg-background p-4" data-testid="shared-item-page">
      <p className="text-muted-foreground">Shared item — coming soon.</p>
    </main>
  );
}
