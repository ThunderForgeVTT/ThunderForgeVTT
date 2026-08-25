import { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import {
  copySharedAbilityToWorld,
  getMyDmWorlds,
  getSharedAbility,
} from "@/api/abilityShares";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { Loader } from "@/components/ui/loader/Loader";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { useAuth } from "@/hooks/useAuth";
import type { DmWorldSummary, SharedAbilityPreview } from "@/types/abilityShare";
import { resolveAbilityLabel, toAbilityClassificationKey } from "@/utils/abilityFacets";
import { effectTypeLabel } from "@/utils/effectLabels";


/**
 * Spec 025 (T091, US6): the read-only view behind an ability share link.
 * Mirrors `SharedItemPage`.
 *
 * Login is required, world membership is not — that is what a share link is
 * for. The preview deliberately shows nothing that identifies the source world
 * or its members (FR-033).
 *
 * Classification renders with **built-in default labels**, not facets: facets
 * belong to a game system, and this page has no world context by design.
 */
export default function SharedAbilityPage() {
  const { code = "" } = useParams();
  const navigate = useNavigate();
  const { isAuthenticated, isLoading: authLoading } = useAuth();

  const [preview, setPreview] = useState<SharedAbilityPreview | null>(null);
  const [dmWorlds, setDmWorlds] = useState<DmWorldSummary[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [step, setStep] = useState<"idle" | "confirming" | "copying" | "done">("idle");
  const [selectedWorldId, setSelectedWorldId] = useState("");
  const [copyError, setCopyError] = useState<string | null>(null);
  const [copiedWorldName, setCopiedWorldName] = useState<string | null>(null);

  useEffect(() => {
    if (authLoading) {
      return;
    }
    if (!isAuthenticated) {
      navigate(`/login?returnTo=${encodeURIComponent(`/shared/ability/${code}`)}`, {
        replace: true,
      });
    }
  }, [authLoading, isAuthenticated, code, navigate]);

  useEffect(() => {
    if (authLoading || !isAuthenticated) {
      return;
    }
    let active = true;
    Promise.all([getSharedAbility(code), getMyDmWorlds()])
      .then(([previewResult, worlds]) => {
        if (active) {
          setPreview(previewResult);
          setDmWorlds(worlds);
        }
      })
      .catch(() => {
        if (active) {
          // Revoked, moderated, and never-existed all land here with the same
          // message — deliberately indistinguishable.
          setLoadError("This share link is no longer available.");
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
  }, [code, authLoading, isAuthenticated]);

  const handleConfirmCopy = async () => {
    if (!selectedWorldId) {
      return;
    }
    setStep("copying");
    setCopyError(null);
    try {
      const created = await copySharedAbilityToWorld(code, selectedWorldId);
      setCopiedWorldName(
        dmWorlds.find((world) => world.id === created.worldId)?.name ?? "your world",
      );
      setStep("done");
    } catch (err) {
      setCopyError(err instanceof Error ? err.message : "Failed to copy ability");
      setStep("confirming");
    }
  };

  if (authLoading || isLoading) {
    return <Loader fullScreen label="Loading shared ability" />;
  }

  if (loadError || !preview) {
    return (
      <>
        <SEO title="Shared ability" description="Shared ability" noindex />
        <Container className="grid max-w-lg gap-4 py-16">
          <Card className="grid gap-3 p-6 text-center" data-testid="shared-ability-unavailable">
            <h1 className="text-lg font-semibold">Not available</h1>
            <p className="text-sm text-muted-foreground">
              {loadError ?? "This share link is no longer available."}
            </p>
          </Card>
        </Container>
      </>
    );
  }

  const classificationLabel = resolveAbilityLabel(
    undefined,
    toAbilityClassificationKey(preview.classification),
  );

  return (
    <>
      <SEO title={`${preview.name} — shared ability`} description="Shared ability" noindex />
      <Container className="grid max-w-lg gap-4 py-16">
        <Card className="grid gap-4 p-6" data-testid="shared-ability-page">
          <div>
            <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
              {classificationLabel}
            </p>
            <h1 className="text-xl font-semibold">{preview.name}</h1>
          </div>

          <p className="text-sm whitespace-pre-wrap">
            {preview.description || (
              <span className="text-muted-foreground italic">No description.</span>
            )}
          </p>

          {preview.effects.length > 0 ? (
            <div className="grid gap-1">
              <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
                Effects
              </p>
              <ul className="grid gap-1 text-sm">
                {preview.effects.map((effect) => (
                  <li key={effect.id} className="text-muted-foreground">
                    <span className="font-medium text-foreground">
                      {effectTypeLabel(effect.effectType)}
                    </span>{" "}
                    — {effect.formula} → {effect.target}
                  </li>
                ))}
              </ul>
            </div>
          ) : null}

          {step === "done" ? (
            <div className="grid gap-3" data-testid="shared-ability-copied">
              <StatusBadge>Copied to {copiedWorldName}.</StatusBadge>
              <Button variant="secondary" onClick={() => navigate("/")}>
                Return home
              </Button>
            </div>
          ) : dmWorlds.length === 0 ? (
            <p className="text-sm text-muted-foreground">
              You don&apos;t have DM-level access to any world yet, so there&apos;s nowhere to
              copy this.
            </p>
          ) : step === "idle" ? (
            <Button
              icon="worlds"
              onClick={() => setStep("confirming")}
              data-testid="shared-ability-copy-button"
            >
              Copy to World
            </Button>
          ) : (
            <div className="grid gap-2">
              <select
                className="rounded-md border border-border bg-background px-2 py-2 text-sm"
                value={selectedWorldId}
                onChange={(event) => setSelectedWorldId(event.target.value)}
                disabled={step === "copying"}
                aria-label="Destination world"
                data-testid="shared-ability-world-select"
              >
                <option value="">Select a destination world…</option>
                {dmWorlds.map((world) => (
                  <option key={world.id} value={world.id}>
                    {world.name}
                  </option>
                ))}
              </select>
              <div className="flex gap-2">
                <Button
                  onClick={() => void handleConfirmCopy()}
                  disabled={step === "copying" || !selectedWorldId}
                  data-testid="shared-ability-confirm-copy"
                >
                  Confirm copy
                </Button>
                <Button
                  variant="ghost"
                  onClick={() => setStep("idle")}
                  disabled={step === "copying"}
                >
                  Cancel
                </Button>
              </div>
              {copyError ? <StatusBadge variant="danger">{copyError}</StatusBadge> : null}
            </div>
          )}
        </Card>
      </Container>
    </>
  );
}
