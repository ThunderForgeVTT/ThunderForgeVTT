import { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import {
  copySharedActorToWorld,
  getMyDmWorlds,
  getSharedActor,
} from "@/api/actorShares";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { Loader } from "@/components/ui/loader/Loader";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { useAuth } from "@/hooks/useAuth";
import type { DmWorldSummary, SharedActorPreview } from "@/types/actorShare";

/**
 * Spec 010 (US5): `/shared/actor/:code` — a read-only preview of a shared actor, with a
 * "Copy to World" deep-clone flow.
 *
 * # This page renders for a signed-out visitor
 *
 * ADR-071. It used to redirect to `/login` before fetching anything, and its
 * route used to be wrapped in `RequireAuthenticated`; both are gone. A resolver
 * that answers anonymously behind a route that redirects to login is the same
 * wall in a different place, so restoring either would undo the decision
 * without touching the server.
 *
 * The preview loads first, for anybody, from `/api/graphql/public`.
 * Authentication is asked for at exactly one point — pressing "Copy to World" —
 * because viewing and copying are different acts with different requirements.
 *
 * World membership is not required either, and never was: that is what a share
 * link is for. The preview shows nothing identifying the source world or its
 * members.
 */
export default function SharedActorPage() {
  const { code = "" } = useParams();
  const navigate = useNavigate();
  const { isAuthenticated, isLoading: authLoading } = useAuth();
  const [preview, setPreview] = useState<SharedActorPreview | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [dmWorlds, setDmWorlds] = useState<DmWorldSummary[] | null>(null);
  const [selectedWorldId, setSelectedWorldId] = useState("");
  const [step, setStep] = useState<"idle" | "confirming" | "copying" | "done">(
    "idle",
  );
  const [copyError, setCopyError] = useState<string | null>(null);
  const [copiedWorldName, setCopiedWorldName] = useState<string | null>(null);

  // No auth gate around this. A visitor with no account sees the actor.
  useEffect(() => {
    if (!code) {
      return;
    }
    let active = true;

    getSharedActor(code)
      .then((previewResult) => {
        if (active) {
          setPreview(previewResult);
        }
      })
      .catch((err) => {
        if (active) {
          // Revoked, moderated, deleted and never-existed all land here with
          // one identical sentence, deliberately (ADR-071). It is shown
          // verbatim rather than replaced, because replacing it with our own
          // wording is how the cases start reading differently.
          setLoadError(
            err instanceof Error
              ? err.message
              : "This share link is no longer available.",
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
  }, [code]);

  // The destination worlds are only needed once somebody wants to copy, and
  // only exist for a signed-in caller.
  useEffect(() => {
    if (step !== "confirming" || !isAuthenticated) {
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
  }, [step, isAuthenticated]);

  // The sign-in point, and the only one. `returnTo` brings them back here
  // rather than to a dashboard, so the link they followed still resolves to
  // what they were looking at.
  const handleCopyPressed = () => {
    if (!isAuthenticated) {
      navigate(
        `/login?returnTo=${encodeURIComponent(`/shared/actor/${code}`)}`,
      );
      return;
    }
    setStep("confirming");
  };

  if (authLoading || isLoading) {
    return <Loader fullScreen label="Loading shared actor" />;
  }

  const handleConfirmCopy = async () => {
    if (!selectedWorldId) {
      return;
    }
    setStep("copying");
    setCopyError(null);
    try {
      const created = await copySharedActorToWorld(code, selectedWorldId);
      setCopiedWorldName(
        dmWorlds?.find((w) => w.id === created.worldId)?.name ?? "your world",
      );
      setStep("done");
    } catch (err) {
      setCopyError(err instanceof Error ? err.message : "Failed to copy actor");
      setStep("confirming");
    }
  };

  return (
    <>
      <SEO
        title="Shared actor"
        description="A shared actor from ThunderForge"
        noindex
      />
      <Container>
        <main className="grid min-h-[60vh] place-items-center py-16">
          {loadError || !preview ? (
            <Card className="grid w-full max-w-lg gap-4 p-6 text-center">
              <StatusBadge variant="danger">
                {loadError ?? "This share link is no longer available."}
              </StatusBadge>
              <Button onClick={() => navigate("/welcome")}>Return home</Button>
            </Card>
          ) : step === "done" ? (
            <Card className="grid w-full max-w-lg gap-4 p-6 text-center">
              <h1 className="text-2xl font-semibold">Copied!</h1>
              <p className="text-muted-foreground">
                {preview.label} was copied to {copiedWorldName}.
              </p>
              <Button onClick={() => navigate("/welcome")}>Return home</Button>
            </Card>
          ) : (
            <Card className="grid w-full max-w-lg gap-4 p-6">
              <div>
                <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
                  {preview.isNpc ? "NPC" : "Player Character"}
                </p>
                <h1 className="text-2xl font-semibold">{preview.label}</h1>
                <p className="text-muted-foreground">
                  Type: {preview.actorType}
                </p>
                {preview.gameSystemId ? (
                  <p className="text-muted-foreground">
                    Game system: {preview.gameSystemId}
                  </p>
                ) : null}
              </div>

              {step === "idle" ? (
                <Button onClick={handleCopyPressed} icon="worlds">
                  Copy to World
                </Button>
              ) : (
                <div className="grid gap-3">
                  {dmWorlds && dmWorlds.length > 0 ? (
                    <>
                      <select
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
                      {copyError ? (
                        <StatusBadge variant="danger">{copyError}</StatusBadge>
                      ) : null}
                    </>
                  ) : (
                    <p className="text-sm text-muted-foreground">
                      You don't have DM-level access to any world yet — create
                      or run a world first to copy this actor into it.
                    </p>
                  )}
                </div>
              )}
            </Card>
          )}
        </main>
      </Container>
    </>
  );
}
