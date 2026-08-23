import { useState } from "react";
import { CounterNoticeForm } from "@/components/legal/CounterNoticeForm";
import { Card } from "@/components/ui/card/Card";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";

export interface ModeratedContentBannerProps {
  /** The disabling case's id (`moderationCaseId` on the placeholder response). */
  caseId: string | null;
  /** Owner-only: shows the counter-notice flow (FR-005). */
  isOwner: boolean;
  /** Set once a counter-notice for this case has been forwarded. */
  restorationDueAt?: string | null;
}

/**
 * Spec 015 (FR-005): rendered in place of an actor/item/lore entry's
 * detail view once the server reports it as a moderation placeholder
 * (`moderated: true`). Only the entry's owner sees the counter-notice
 * flow — everyone else just sees that the content was disabled.
 */
export function ModeratedContentBanner({ caseId, isOwner, restorationDueAt }: ModeratedContentBannerProps) {
  const [filedCaseId, setFiledCaseId] = useState<string | null>(null);

  return (
    <Card className="grid gap-3 p-6">
      <StatusBadge variant="danger">Content disabled</StatusBadge>
      <p className="text-sm text-muted-foreground">
        This content was disabled in response to a DMCA takedown notice and is currently
        unavailable.
      </p>
      {restorationDueAt ? (
        <p className="text-sm text-muted-foreground">
          A counter-notice was filed. Unless the claimant files a court action, this content
          is scheduled to be restored on{" "}
          <strong>{new Date(restorationDueAt).toLocaleDateString()}</strong>.
        </p>
      ) : null}
      {isOwner && !restorationDueAt && caseId ? (
        <div className="grid gap-2">
          <p className="text-sm">
            If you believe this was disabled by mistake or misidentification, you may file a
            counter-notice.
          </p>
          <CounterNoticeForm caseId={caseId} onSubmitted={(record) => setFiledCaseId(record.caseId)} />
          {filedCaseId ? (
            <p className="text-xs text-muted-foreground">
              Counter-notice filed. Case reference: <code>{filedCaseId}</code>.
            </p>
          ) : null}
        </div>
      ) : null}
    </Card>
  );
}
