import { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { copySharedItemToWorld, getMyDmWorlds, getSharedItem } from "@/api/itemShares";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { Loader } from "@/components/ui/loader/Loader";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { useAuth } from "@/hooks/useAuth";
import type { DmWorldSummary, SharedItemPreview } from "@/types/itemShare";
import { effectTypeLabel } from "@/utils/effectLabels";


/**
 * Spec 013 (T049, User Story 5): `/shared/item/:code` — a login-required
 * (but not world-membership-required) read-only preview of a shared Item,
 * with a "Copy to World" deep-clone flow. Direct mirror of
 * pages/actor-share/SharedActorPage.tsx.
 */
export default function SharedItemPage() {
  const { code = "" } = useParams();
  const navigate = useNavigate();
  const { isAuthenticated, isLoading: authLoading } = useAuth();
  const [preview, setPreview] = useState<SharedItemPreview | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [dmWorlds, setDmWorlds] = useState<DmWorldSummary[] | null>(null);
  const [selectedWorldId, setSelectedWorldId] = useState("");
  const [step, setStep] = useState<"idle" | "confirming" | "copying" | "done">("idle");
  const [copyError, setCopyError] = useState<string | null>(null);
  const [copiedWorldName, setCopiedWorldName] = useState<string | null>(null);

  useEffect(() => {
    if (!authLoading && !isAuthenticated) {
      navigate(`/login?returnTo=${encodeURIComponent(`/shared/item/${code}`)}`);
    }
  }, [authLoading, isAuthenticated, code, navigate]);

  useEffect(() => {
    if (!isAuthenticated || !code) {
      return;
    }
    let active = true;

    Promise.all([getSharedItem(code), getMyDmWorlds()])
      .then(([previewResult, worlds]) => {
        if (!active) {
          return;
        }
        setPreview(previewResult);
        setDmWorlds(worlds);
      })
      .catch((err) => {
        if (active) {
          setLoadError(err instanceof Error ? err.message : "This share link is no longer available.");
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
  }, [isAuthenticated, code]);

  if (authLoading || (isAuthenticated && isLoading)) {
    return <Loader fullScreen label="Loading shared item" />;
  }

  if (!isAuthenticated) {
    return null;
  }

  const handleConfirmCopy = async () => {
    if (!selectedWorldId) {
      return;
    }
    setStep("copying");
    setCopyError(null);
    try {
      const created = await copySharedItemToWorld(code, selectedWorldId);
      setCopiedWorldName(dmWorlds?.find((w) => w.id === created.worldId)?.name ?? "your world");
      setStep("done");
    } catch (err) {
      setCopyError(err instanceof Error ? err.message : "Failed to copy item");
      setStep("confirming");
    }
  };

  return (
    <>
      <SEO title="Shared item" description="A shared item from ThunderForge" noindex />
      <Container>
        <main className="grid min-h-[60vh] place-items-center py-16">
          {loadError || !preview ? (
            <Card className="grid w-full max-w-lg gap-4 p-6 text-center">
              <StatusBadge variant="danger">{loadError ?? "This share link is no longer available."}</StatusBadge>
              <Button onClick={() => navigate("/welcome")}>Return home</Button>
            </Card>
          ) : step === "done" ? (
            <Card className="grid w-full max-w-lg gap-4 p-6 text-center">
              <h1 className="text-2xl font-semibold">Copied!</h1>
              <p className="text-muted-foreground">
                {preview.name} was copied to {copiedWorldName}.
              </p>
              <Button onClick={() => navigate("/welcome")}>Return home</Button>
            </Card>
          ) : (
            <Card className="grid w-full max-w-lg gap-4 p-6">
              <div>
                <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">Item</p>
                <h1 className="text-2xl font-semibold">{preview.name}</h1>
                <p className="text-muted-foreground whitespace-pre-wrap">
                  {preview.description || <span className="italic">No description.</span>}
                </p>
                {preview.effects.length > 0 ? (
                  <ul className="mt-2 grid gap-1 text-sm text-muted-foreground">
                    {preview.effects.map((effect) => (
                      <li key={effect.id}>
                        <span className="font-medium text-foreground">
                          {effectTypeLabel(effect.effectType)}
                        </span>{" "}
                        — {effect.formula} → {effect.target}
                      </li>
                    ))}
                  </ul>
                ) : null}
              </div>

              {step === "idle" ? (
                <Button onClick={() => setStep("confirming")} icon="worlds">
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
                        <Button onClick={() => void handleConfirmCopy()} disabled={!selectedWorldId || step === "copying"}>
                          {step === "copying" ? "Copying..." : "Confirm copy"}
                        </Button>
                        <Button variant="ghost" onClick={() => setStep("idle")}>
                          Cancel
                        </Button>
                      </div>
                      {copyError ? <StatusBadge variant="danger">{copyError}</StatusBadge> : null}
                    </>
                  ) : (
                    <p className="text-sm text-muted-foreground">
                      You don't have DM-level access to any world yet — create or run a world first to copy this item
                      into it.
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
