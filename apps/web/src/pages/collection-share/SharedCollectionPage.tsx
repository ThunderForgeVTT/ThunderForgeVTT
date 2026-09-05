import { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import {
  copySharedCollectionToWorld,
  getMyDmWorlds,
  getSharedCollection,
} from "@/api/collections";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { Loader } from "@/components/ui/loader/Loader";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { useAuth } from "@/hooks/useAuth";
import { memberTypeLabel } from "@/types/collection";
import type {
  CopyReceipt,
  DmWorldSummary,
  SharedCollectionPreview,
} from "@/types/collection";

/**
 * Spec 026 (T032): `/collection/:shareCode`.
 *
 * # This page renders for a signed-out visitor
 *
 * That is the whole point of it, and it is the one way it differs from the
 * three share pages already shipped. `SharedItemPage`, `SharedActorPage` and
 * `SharedAbilityPage` all redirect to `/login` before they fetch anything;
 * their routes are wrapped in `RequireAuthenticated`. Copying this page's
 * structure from any of them without changing that would produce a link that
 * cannot be opened by the person it was sent to, which is the entire feature.
 *
 * So the preview loads first, for anybody, from `/api/graphql/public`
 * (FR-009a, ADR-070). Authentication is asked for at exactly one point —
 * pressing "Copy to a world" — because viewing and copying are different acts
 * with different requirements (FR-009b).
 */
export default function SharedCollectionPage() {
  const { shareCode = "" } = useParams();
  const navigate = useNavigate();
  const { isAuthenticated, isLoading: authLoading } = useAuth();

  const [preview, setPreview] = useState<SharedCollectionPreview | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  const [dmWorlds, setDmWorlds] = useState<DmWorldSummary[] | null>(null);
  const [selectedWorldId, setSelectedWorldId] = useState("");
  const [step, setStep] = useState<"idle" | "choosing" | "copying" | "done">(
    "idle",
  );
  const [copyError, setCopyError] = useState<string | null>(null);
  const [receipt, setReceipt] = useState<CopyReceipt | null>(null);
  const [copiedWorldName, setCopiedWorldName] = useState<string | null>(null);

  // No auth gate around this. A visitor with no account sees the collection.
  useEffect(() => {
    if (!shareCode) {
      return;
    }
    let active = true;

    getSharedCollection(shareCode)
      .then((result) => {
        if (active) {
          setPreview(result);
        }
      })
      .catch((err: unknown) => {
        if (active) {
          // The server answers an unknown code, a revoked link and a deleted
          // collection with one identical sentence, deliberately (FR-010). It
          // is shown verbatim rather than replaced, because replacing it with
          // our own wording is how the three cases start reading differently.
          setLoadError(
            err instanceof Error
              ? err.message
              : "This link is no longer available.",
          );
        }
      })
      .finally(() => {
        if (active) {
          setIsLoading(false);
        }
      });

    return () => {
      active = false;
    };
  }, [shareCode]);

  // The destination worlds are only needed once somebody wants to copy, and
  // only exist for a signed-in caller.
  useEffect(() => {
    if (step !== "choosing" || !isAuthenticated || dmWorlds !== null) {
      return;
    }
    let active = true;
    getMyDmWorlds()
      .then((worlds) => {
        if (active) {
          setDmWorlds(worlds);
        }
      })
      .catch((err: unknown) => {
        if (active) {
          setCopyError(
            err instanceof Error ? err.message : "Could not load your worlds.",
          );
        }
      });
    return () => {
      active = false;
    };
  }, [step, isAuthenticated, dmWorlds]);

  const handleCopyPressed = () => {
    if (!isAuthenticated) {
      // The sign-in point, and the only one. `returnTo` brings them back here
      // rather than to a dashboard, so the link they followed still resolves
      // to what they were looking at.
      navigate(
        `/login?returnTo=${encodeURIComponent(`/collection/${shareCode}`)}`,
      );
      return;
    }
    setStep("choosing");
  };

  const handleConfirmCopy = async () => {
    if (!selectedWorldId) {
      return;
    }
    setStep("copying");
    setCopyError(null);
    try {
      const result = await copySharedCollectionToWorld(
        shareCode,
        selectedWorldId,
      );
      setReceipt(result);
      setCopiedWorldName(
        dmWorlds?.find((w) => w.id === selectedWorldId)?.name ?? "your world",
      );
      setStep("done");
    } catch (err: unknown) {
      // Surfaced verbatim: the server's refusals here name the reason (a
      // destination with no scene to put a displaced actor in, a revoked
      // link), and a generic "copy failed" would throw that away.
      setCopyError(
        err instanceof Error
          ? err.message
          : "The collection could not be copied.",
      );
      setStep("choosing");
    }
  };

  if (isLoading || authLoading) {
    return <Loader fullScreen label="Loading shared collection" />;
  }

  return (
    <>
      <SEO
        title={
          preview ? `Shared collection: ${preview.name}` : "Shared collection"
        }
        description="A shared content collection from ThunderForge"
        noindex
      />
      <Container>
        <main className="grid min-h-[60vh] place-items-center py-16">
          {loadError || !preview ? (
            <Card className="grid w-full max-w-lg gap-4 p-6 text-center">
              <StatusBadge variant="danger">
                {loadError ?? "This link is no longer available."}
              </StatusBadge>
              <Button onClick={() => navigate("/")}>Go to ThunderForge</Button>
            </Card>
          ) : step === "done" && receipt ? (
            <CopyReceiptCard
              receipt={receipt}
              collectionName={preview.name}
              worldName={copiedWorldName ?? "your world"}
              onDone={() => navigate("/worlds")}
            />
          ) : (
            <Card className="grid w-full max-w-2xl gap-5 p-6">
              <div>
                <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
                  Collection
                </p>
                <h1 className="text-2xl font-semibold">{preview.name}</h1>
                {preview.description ? (
                  <p className="text-muted-foreground whitespace-pre-wrap">
                    {preview.description}
                  </p>
                ) : null}
              </div>

              {preview.countsByType.length > 0 ? (
                <ul
                  className="flex flex-wrap gap-2"
                  data-testid="collection-counts"
                >
                  {preview.countsByType.map((entry) => (
                    <li
                      key={entry.memberType}
                      className="rounded-full border border-input px-3 py-1 text-sm text-muted-foreground"
                    >
                      {entry.count}{" "}
                      {memberTypeLabel(entry.memberType, entry.count)}
                    </li>
                  ))}
                </ul>
              ) : null}

              <ul
                className="grid gap-1 text-sm"
                data-testid="collection-members"
              >
                {preview.members.map((member, index) => (
                  <li
                    key={`${member.memberType}-${index}`}
                    className="flex items-baseline gap-2"
                  >
                    <span className="text-xs text-muted-foreground uppercase">
                      {memberTypeLabel(member.memberType)}
                    </span>
                    <span className="text-foreground">{member.name}</span>
                  </li>
                ))}
              </ul>

              {/*
                FR-022: a number, never a name. Naming a taken-down artifact in
                the sentence explaining that it was taken down would defeat the
                takedown, so the server sends a count and this says only that.
              */}
              {preview.withheldCount > 0 ? (
                <p className="text-sm text-muted-foreground">
                  {preview.withheldCount === 1
                    ? "1 item in this collection is unavailable and will not be copied."
                    : `${preview.withheldCount} items in this collection are unavailable and will not be copied.`}
                </p>
              ) : null}

              {step === "idle" ? (
                <div className="grid gap-2">
                  <Button onClick={handleCopyPressed} icon="worlds">
                    Copy to a world
                  </Button>
                  {!isAuthenticated ? (
                    <p className="text-sm text-muted-foreground">
                      You can read this collection without an account. Copying
                      it into a world of your own needs one.
                    </p>
                  ) : null}
                </div>
              ) : (
                <div className="grid gap-3">
                  {dmWorlds === null ? (
                    <p className="text-sm text-muted-foreground">
                      Loading your worlds…
                    </p>
                  ) : dmWorlds.length > 0 ? (
                    <>
                      <select
                        aria-label="Destination world"
                        value={selectedWorldId}
                        onChange={(e) => setSelectedWorldId(e.target.value)}
                        className="h-9 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50"
                      >
                        <option value="">Choose a world...</option>
                        {dmWorlds.map((world) => (
                          <option key={world.id} value={world.id}>
                            {world.name}
                          </option>
                        ))}
                      </select>
                      <div className="flex gap-3">
                        <Button
                          onClick={() => void handleConfirmCopy()}
                          disabled={!selectedWorldId || step === "copying"}
                        >
                          {step === "copying" ? "Copying..." : "Confirm copy"}
                        </Button>
                        <Button variant="ghost" onClick={() => setStep("idle")}>
                          Cancel
                        </Button>
                      </div>
                    </>
                  ) : (
                    <p className="text-sm text-muted-foreground">
                      You don't run any world yet — create one first to copy
                      this collection into it.
                    </p>
                  )}
                  {copyError ? (
                    <StatusBadge variant="danger">{copyError}</StatusBadge>
                  ) : null}
                </div>
              )}
            </Card>
          )}
        </main>
      </Container>
    </>
  );
}

/**
 * What arrived, and what did not.
 *
 * The fidelity notes are shown, not summarised away. They are the honest half
 * of the copy: a reference that pointed outside the collection, tokens that
 * stayed behind, a member withheld by moderation. A recipient who is not told
 * has to discover the difference by playing.
 */
function CopyReceiptCard({
  receipt,
  collectionName,
  worldName,
  onDone,
}: {
  receipt: CopyReceipt;
  collectionName: string;
  worldName: string;
  onDone: () => void;
}) {
  return (
    <Card className="grid w-full max-w-2xl gap-4 p-6">
      <div>
        <h1 className="text-2xl font-semibold">Copied</h1>
        <p className="text-muted-foreground">
          {collectionName} was copied into {worldName}. It is yours now — edits
          you make here reach nothing in the world it came from.
        </p>
      </div>

      <ul className="grid gap-1 text-sm" data-testid="copy-receipt-created">
        {receipt.created.map((record) => (
          <li key={record.id} className="flex items-baseline gap-2">
            <span className="text-xs text-muted-foreground uppercase">
              {memberTypeLabel(record.memberType)}
            </span>
            <span className="text-foreground">{record.name}</span>
          </li>
        ))}
      </ul>

      {receipt.fidelityNotes.length > 0 ? (
        <div className="grid gap-1" data-testid="copy-receipt-notes">
          <p className="text-sm font-medium">What did not come across</p>
          <ul className="grid gap-1 text-sm text-muted-foreground">
            {receipt.fidelityNotes.map((note, index) => (
              <li key={index}>{note}</li>
            ))}
          </ul>
        </div>
      ) : null}

      <Button onClick={onDone}>Go to my worlds</Button>
    </Card>
  );
}
