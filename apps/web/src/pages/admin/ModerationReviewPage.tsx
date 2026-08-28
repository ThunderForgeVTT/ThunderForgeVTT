import { useEffect, useState } from "react";
import {
  getModerationHistoryForAccount,
  getRepeatInfringerFlags,
  resolveModerationCase,
} from "@/api/moderation";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { Loader } from "@/components/ui/loader/Loader";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import type { ModerationCaseRecord } from "@/types/moderation";
import type { SeoConfig } from "@/types/seo";

export const moderationReviewPageSeo: SeoConfig = {
  title: "Content moderation review",
  description: "Review DMCA takedown case history and repeat-infringer flags.",
  canonicalPath: "/admin/moderation",
  noindex: true,
};

/**
 * Spec 015 (US3, T033): compliance-staff-only surface listing accounts
 * flagged as repeat infringers (FR-011) and, per selected account, the
 * full moderation case history behind that flag.
 */
export default function ModerationReviewPage() {
  const [flaggedAccountIds, setFlaggedAccountIds] = useState<string[] | null>(
    null,
  );
  const [selectedAccountId, setSelectedAccountId] = useState<string | null>(
    null,
  );
  const [history, setHistory] = useState<ModerationCaseRecord[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    getRepeatInfringerFlags()
      .then((ids) => {
        if (active) {
          setFlaggedAccountIds(ids);
        }
      })
      .catch((err) => {
        if (active) {
          setError(
            err instanceof Error
              ? err.message
              : "Failed to load repeat-infringer flags",
          );
        }
      });
    return () => {
      active = false;
    };
  }, []);

  const loadHistory = async (accountId: string) => {
    setSelectedAccountId(accountId);
    setHistory(null);
    try {
      const cases = await getModerationHistoryForAccount(accountId);
      setHistory(cases);
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to load case history",
      );
    }
  };

  const handleResolve = async (caseId: string) => {
    try {
      await resolveModerationCase(caseId, "CONTENT_REMAINS_DISABLED");
      if (selectedAccountId) {
        await loadHistory(selectedAccountId);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to resolve case");
    }
  };

  return (
    <>
      <SEO {...moderationReviewPageSeo} />
      <Container className="grid gap-6 py-10">
        <div>
          <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
            Compliance
          </p>
          <h1 className="text-2xl font-semibold">Content moderation review</h1>
        </div>

        {error ? <StatusBadge variant="danger">{error}</StatusBadge> : null}

        <Card className="grid gap-3 p-6">
          <h2 className="text-lg font-semibold">Repeat-infringer flags</h2>
          {flaggedAccountIds === null ? (
            <Loader label="Loading flags" />
          ) : flaggedAccountIds.length === 0 ? (
            <p className="text-sm text-muted-foreground">
              No accounts currently flagged.
            </p>
          ) : (
            <ul className="grid gap-2">
              {flaggedAccountIds.map((accountId) => (
                <li
                  key={accountId}
                  className="flex items-center justify-between gap-3"
                >
                  <code className="text-sm">{accountId}</code>
                  <Button
                    variant="secondary"
                    size="sm"
                    onClick={() => void loadHistory(accountId)}
                  >
                    View history
                  </Button>
                </li>
              ))}
            </ul>
          )}
        </Card>

        {selectedAccountId ? (
          <Card className="grid gap-3 p-6">
            <h2 className="text-lg font-semibold">
              Case history — {selectedAccountId}
            </h2>
            {history === null ? (
              <Loader label="Loading case history" />
            ) : history.length === 0 ? (
              <p className="text-sm text-muted-foreground">
                No cases found for this account.
              </p>
            ) : (
              <ul className="grid gap-3">
                {history.map((moderationCase) => (
                  <li
                    key={moderationCase.caseId}
                    className="grid gap-1 border-b pb-3 last:border-0"
                  >
                    <div className="flex items-center justify-between gap-3">
                      <code className="text-sm">{moderationCase.caseId}</code>
                      <StatusBadge variant="warning">
                        {moderationCase.currentStatus}
                      </StatusBadge>
                    </div>
                    <p className="text-xs text-muted-foreground">
                      {moderationCase.entityType} · {moderationCase.entityId}
                    </p>
                    {moderationCase.currentStatus ===
                    "COUNTER_NOTICE_FORWARDED" ? (
                      <Button
                        variant="danger"
                        size="sm"
                        className="justify-self-start"
                        onClick={() =>
                          void handleResolve(moderationCase.caseId)
                        }
                      >
                        Block restoration (content remains disabled)
                      </Button>
                    ) : null}
                  </li>
                ))}
              </ul>
            )}
          </Card>
        ) : null}
      </Container>
    </>
  );
}
