// Spec 013 (T028, User Story 1): view/edit for a single Item, mirrors
// ActorDetailPage.tsx. This is a placeholder shell wired up by Foundational
// routing (T014) — full implementation lands in Phase 3 (US1).

export interface ItemDetailPageProps {
  mode: "view" | "edit";
}

export default function ItemDetailPage(_props: ItemDetailPageProps) {
  return (
    <main className="grid min-h-screen place-items-center bg-background p-4" data-testid="item-detail-page">
      <p className="text-muted-foreground">Item detail — coming soon.</p>
    </main>
  );
}
