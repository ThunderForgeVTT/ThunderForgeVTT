import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import {
  type AccountNotice,
  readMyStanding,
  type Standing,
  type Strike,
} from "@/api/standing";
import { SEO } from "@/components/seo/SEO";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { Loader } from "@/components/ui/loader/Loader";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import type { SeoConfig } from "@/types/seo";

export const standingPageSeo: SeoConfig = {
  title: "Account standing",
  description:
    "Your strikes, when each stops counting, and what you were told.",
  canonicalPath: "/settings/standing",
  noindex: true,
};

const KIND_NAMES: Record<string, string> = {
  WORLD_ACTOR: "An actor",
  WORLD_ITEM: "An item",
  WORLD_ABILITY: "An ability",
  WORLD_LORE_ENTRY: "A lore entry",
};

function formatDate(iso: string): string {
  return new Date(iso).toLocaleDateString(undefined, {
    year: "numeric",
    month: "long",
    day: "numeric",
  });
}

/** Where the content a strike was for lives, when it has a page by id. */
function contentPath(strike: Strike): string | null {
  switch (strike.entityType) {
    case "WORLD_ITEM":
      return `/world/${strike.worldId}/item/${strike.entityId}/view`;
    case "WORLD_ACTOR":
      return `/world/${strike.worldId}/actor/${strike.entityId}/view`;
    case "WORLD_ABILITY":
      return `/world/${strike.worldId}/ability/${strike.entityId}/view`;
    default:
      return null;
  }
}

/**
 * The words of a notice, rendered from its kind and payload. Not stored on the
 * server on purpose, so this is the one place they live.
 */
function noticeText(notice: AccountNotice): string {
  const payload = notice.payload ?? {};
  const count = Number(payload.strikeCount ?? 0);
  switch (notice.kind) {
    case "strike_recorded": {
      const suspendAt = Number(payload.suspendPublishingAt ?? 0);
      const threshold = Number(payload.threshold ?? 0);
      const agesOut =
        typeof payload.agesOutAt === "string"
          ? formatDate(payload.agesOutAt)
          : "the end of the lookback";
      return (
        `A takedown against something you shared was upheld, and it counts as ` +
        `strike ${count}. Sharing pauses at ${suspendAt} and the account is ` +
        `disabled at ${threshold}. This strike stops counting on ${agesOut}, ` +
        `or sooner if a counter-notice succeeds.`
      );
    }
    case "publishing_suspended":
      return (
        `Sharing is paused: ${count} strikes are counting. Your worlds are ` +
        `untouched — you can still play, edit and read everything you have ` +
        `made. Sharing comes back when a strike stops counting or a ` +
        `counter-notice succeeds.`
      );
    default:
      return "A notice about your account.";
  }
}

/**
 * Spec 039 US5 (FR-029): where a person stands, at any time — each strike,
 * what it was, when it stops counting — and every notice they were sent
 * (FR-028). Half of the promise that nobody reaches the third strike without
 * having been told about the first two.
 */
export function StandingPage() {
  const [standing, setStanding] = useState<Standing | null>(null);
  const [notices, setNotices] = useState<AccountNotice[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    readMyStanding()
      .then((result) => {
        if (active) {
          setStanding(result.standing);
          setNotices(result.notices);
        }
      })
      .catch((cause) => {
        if (active) {
          setError(
            cause instanceof Error
              ? cause.message
              : "Your standing could not be loaded",
          );
        }
      });
    return () => {
      active = false;
    };
  }, []);

  return (
    <>
      <SEO {...standingPageSeo} />
      <Container>
        <div className="grid gap-6 py-8">
          <header className="grid gap-1">
            <h1 className="text-2xl font-semibold">Account standing</h1>
            <p className="text-sm text-muted-foreground">
              Takedowns upheld against things you have shared, and what they
              mean for sharing. Nothing here affects playing, editing or reading
              your own worlds.
            </p>
          </header>

          {error ? <StatusBadge variant="danger">{error}</StatusBadge> : null}
          {!standing && !error ? (
            <Loader label="Loading your standing" />
          ) : null}

          {standing ? (
            <Card className="grid gap-3 p-6" data-testid="standing-summary">
              <div className="flex flex-wrap items-center justify-between gap-3">
                <h2 className="text-lg font-semibold">
                  {standing.strikeCount === 1
                    ? "1 strike"
                    : `${standing.strikeCount} strikes`}
                </h2>
                {standing.mayPublish ? (
                  <StatusBadge
                    variant="success"
                    data-testid="standing-may-publish"
                  >
                    Sharing is available
                  </StatusBadge>
                ) : (
                  <StatusBadge
                    variant="warning"
                    data-testid="standing-suspended"
                  >
                    Sharing is paused
                  </StatusBadge>
                )}
              </div>
              <p className="text-sm text-muted-foreground">
                Sharing pauses at {standing.suspendPublishingAt} strikes and the
                account is disabled at {standing.threshold}. A strike stops
                counting on its own after the lookback, or when a counter-notice
                succeeds — file one from the page of the content that was taken
                down.
              </p>

              {standing.strikes.length > 0 ? (
                <ul className="grid gap-2" data-testid="standing-strikes">
                  {standing.strikes.map((strike) => {
                    const path = contentPath(strike);
                    return (
                      <li
                        key={strike.caseId}
                        className="grid gap-1 rounded border p-3 text-sm"
                        data-testid="standing-strike"
                      >
                        <p>
                          {KIND_NAMES[strike.entityType] ?? "Content"}, taken
                          down on {formatDate(strike.recordedAt)}.{" "}
                          {path ? <Link to={path}>Open it</Link> : null}
                        </p>
                        <p className="text-xs text-muted-foreground">
                          Stops counting on {formatDate(strike.agesOutAt)} ·
                          case <code>{strike.caseId}</code>
                        </p>
                      </li>
                    );
                  })}
                </ul>
              ) : null}
            </Card>
          ) : null}

          {standing ? (
            <Card className="grid gap-3 p-6">
              <h2 className="text-lg font-semibold">What you were told</h2>
              {notices.length === 0 ? (
                <p className="text-sm text-muted-foreground">Nothing yet.</p>
              ) : (
                <ul className="grid gap-2" data-testid="standing-notices">
                  {notices.map((notice) => (
                    <li
                      key={notice.id}
                      className="grid gap-1 border-b pb-2 text-sm last:border-0"
                      data-testid="standing-notice"
                      data-kind={notice.kind}
                    >
                      <p>{noticeText(notice)}</p>
                      <p className="text-xs text-muted-foreground">
                        {formatDate(notice.createdAt)}
                      </p>
                    </li>
                  ))}
                </ul>
              )}
            </Card>
          ) : null}
        </div>
      </Container>
    </>
  );
}

export default StandingPage;
