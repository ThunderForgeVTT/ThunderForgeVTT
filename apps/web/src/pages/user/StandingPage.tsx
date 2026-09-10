import { type FormEvent, useCallback, useEffect, useState } from "react";
import { Link } from "react-router-dom";
import {
  type AccountNotice,
  fileAppeal,
  readMyStanding,
  type Standing,
  type Strike,
  type Termination,
} from "@/api/standing";
import { CounterNoticeForm } from "@/components/legal/CounterNoticeForm";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
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

/** The download — the same export the account settings offer (FR-032). */
const DOWNLOAD_PATH = "/api/user/data/export?format=zip";

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
  const dateOf = (key: string, otherwise: string) =>
    typeof payload[key] === "string"
      ? formatDate(payload[key] as string)
      : otherwise;
  switch (notice.kind) {
    case "strike_recorded": {
      const suspendAt = Number(payload.suspendPublishingAt ?? 0);
      const threshold = Number(payload.threshold ?? 0);
      return (
        `A takedown against something you shared was upheld, and it counts as ` +
        `strike ${count}. Sharing pauses at ${suspendAt} and the account is ` +
        `disabled at ${threshold}. This strike stops counting on ` +
        `${dateOf("agesOutAt", "the end of the lookback")}, or sooner if a ` +
        `counter-notice succeeds.`
      );
    }
    case "publishing_suspended":
      return (
        `Sharing is paused: ${count} strikes are counting. Your worlds are ` +
        `untouched — you can still play, edit and read everything you have ` +
        `made. Sharing comes back when a strike stops counting or a ` +
        `counter-notice succeeds.`
      );
    case "account_disabled":
      // FR-036: that deletion is irreversible, in the first sentence.
      return (
        `This account will be permanently and irreversibly deleted on ` +
        `${dateOf("deletionDueAt", "the date shown above")}` +
        `${payload.requiresHuman ? ", once an administrator confirms it" : ""}. ` +
        `It was disabled after ${count} strikes. Until then you can download ` +
        `everything you have, appeal, or file a counter-notice against any ` +
        `strike — none of these uses up another.`
      );
    case "appeal_resolved":
      return payload.upheld
        ? "Your appeal was upheld. The account is restored, the deletion is " +
            "cancelled, and the strike it overturned no longer counts."
        : `Your appeal was not upheld. The account will be deleted on ` +
            `${dateOf("deletionDueAt", "the date already set")}` +
            `${payload.requiresHuman ? ", once an administrator confirms it" : ""}.`;
    case "account_restored":
      return (
        "Your account is restored. Fewer strikes are counting than the " +
        "number that disables an account, so the deletion is cancelled — " +
        "nobody had to ask."
      );
    case "actor_rescued": {
      const names = Array.isArray(payload.characters)
        ? (payload.characters as string[]).join(", ")
        : "your characters";
      return (
        `The world "${String(payload.sourceWorldName ?? "")}" was deleted ` +
        `with its creator's account. Before it went, ${names} ` +
        `${Array.isArray(payload.characters) && payload.characters.length === 1 ? "was" : "were"} ` +
        `moved to a world of yours, in a collection named after it.`
      );
    }
    default:
      return "A notice about your account.";
  }
}

/**
 * The window, and both remedies. A person reading this has thirty days and two
 * things they can do; the page says both, and says that neither costs the
 * other (FR-032).
 */
function WindowCard({
  termination,
  onAppealed,
}: {
  termination: Termination;
  onAppealed: () => void;
}) {
  const [statement, setStatement] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [sending, setSending] = useState(false);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setSending(true);
    setError(null);
    try {
      await fileAppeal(statement);
      onAppealed();
    } catch (cause) {
      setError(
        cause instanceof Error ? cause.message : "The appeal was not filed",
      );
    } finally {
      setSending(false);
    }
  };

  return (
    <Card className="grid gap-4 p-6" data-testid="standing-window">
      <div className="grid gap-1">
        <h2 className="text-lg font-semibold">This account is disabled</h2>
        <p className="text-sm">
          It will be <strong>permanently deleted</strong> on{" "}
          <time
            dateTime={termination.deletionDueAt}
            data-testid="standing-deletion-date"
          >
            {formatDate(termination.deletionDueAt)}
          </time>
          {termination.requiresHuman
            ? ", once an administrator confirms it"
            : ""}
          . Deletion cannot be undone. Until then you can download your data,
          appeal below, or file a counter-notice against any strike in the list
          — and using one does not use up another.
        </p>
      </div>

      <div className="grid gap-2">
        <h3 className="font-semibold">Download your data</h3>
        <p className="text-sm text-muted-foreground">
          Everything the account holds — worlds, characters, items, abilities,
          lore and collections. Downloading does not shorten or end the window.
        </p>
        <a
          href={DOWNLOAD_PATH}
          download
          className="justify-self-start text-sm underline"
          data-testid="standing-download"
        >
          Download your data
        </a>
      </div>

      <div className="grid gap-2">
        <h3 className="font-semibold">Appeal</h3>
        {termination.appealState === "none" ? (
          <form className="grid gap-2" onSubmit={(e) => void submit(e)}>
            <label htmlFor="standing-appeal" className="text-sm">
              Say why the decision is wrong. An administrator reads it.
            </label>
            <textarea
              id="standing-appeal"
              className="min-h-24 rounded border p-2 text-sm"
              value={statement}
              maxLength={5000}
              onChange={(event) => setStatement(event.target.value)}
            />
            <Button
              type="submit"
              size="sm"
              className="justify-self-start"
              disabled={sending || statement.trim().length === 0}
            >
              File appeal
            </Button>
            {error ? <p className="text-xs text-destructive">{error}</p> : null}
          </form>
        ) : termination.appealState === "open" ? (
          <p className="text-sm" data-testid="standing-appeal-open">
            Your appeal is with an administrator. While it is open nothing is
            deleted — even after the date.
          </p>
        ) : (
          <p className="text-sm" data-testid="standing-appeal-rejected">
            Your appeal was not upheld
            {termination.appealNote ? `: ${termination.appealNote}` : ""}. The
            date above stands.
          </p>
        )}
      </div>
    </Card>
  );
}

/**
 * Spec 039 US5 and US7 (FR-029, FR-031): where a person stands, at any time —
 * each strike, what it was, when it stops counting, every notice they were
 * sent — and, when disabled, the window and both remedies. The one page a
 * disabled account can reach.
 */
export function StandingPage() {
  const [standing, setStanding] = useState<Standing | null>(null);
  const [notices, setNotices] = useState<AccountNotice[]>([]);
  const [error, setError] = useState<string | null>(null);
  // One counter-notice form open at a time: the form's fields carry fixed
  // ids, and a disabled account can reach its strikes only from here.
  const [counterNoticeFor, setCounterNoticeFor] = useState<string | null>(null);
  const [filedFor, setFiledFor] = useState<string | null>(null);

  const load = useCallback(() => {
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

  useEffect(() => load(), [load]);

  return (
    <>
      <SEO {...standingPageSeo} />
      <Container>
        <div className="grid gap-6 py-8">
          <header className="grid gap-1">
            <h1 className="text-2xl font-semibold">Account standing</h1>
            <p className="text-sm text-muted-foreground">
              Takedowns upheld against things you have shared, and what they
              mean for your account.
            </p>
          </header>

          {error ? <StatusBadge variant="danger">{error}</StatusBadge> : null}
          {filedFor ? (
            <StatusBadge variant="success" data-testid="standing-counter-filed">
              Counter-notice filed for case {filedFor}. While it is under review
              that case does not count.
            </StatusBadge>
          ) : null}
          {!standing && !error ? (
            <Loader label="Loading your standing" />
          ) : null}

          {standing?.disabled && standing.termination ? (
            <WindowCard
              termination={standing.termination}
              onAppealed={() => void load()}
            />
          ) : null}

          {standing ? (
            <Card className="grid gap-3 p-6" data-testid="standing-summary">
              <div className="flex flex-wrap items-center justify-between gap-3">
                <h2 className="text-lg font-semibold">
                  {standing.strikeCount === 1
                    ? "1 strike"
                    : `${standing.strikeCount} strikes`}
                </h2>
                {standing.disabled ? (
                  <StatusBadge variant="danger" data-testid="standing-disabled">
                    Account disabled
                  </StatusBadge>
                ) : standing.mayPublish ? (
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
                    const path = standing.disabled ? null : contentPath(strike);
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
                        {/* Decided 2026-09-10: a disabled account keeps the
                            counter-notice — the statutory route back — and
                            its content pages are out of reach, so it lives
                            here. */}
                        {standing.disabled ? (
                          counterNoticeFor === strike.caseId ? (
                            <CounterNoticeForm
                              caseId={strike.caseId}
                              onSubmitted={() => {
                                setCounterNoticeFor(null);
                                setFiledFor(strike.caseId);
                                void load();
                              }}
                            />
                          ) : (
                            <Button
                              size="sm"
                              variant="secondary"
                              className="justify-self-start"
                              onClick={() => setCounterNoticeFor(strike.caseId)}
                            >
                              File a counter-notice
                            </Button>
                          )
                        ) : null}
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
