import { useEffect, useState } from "react";
import { Navigate, useNavigate, useParams } from "react-router-dom";
import { deleteAbility, getAbility, setAbilityGmOnly, updateAbility } from "@/api/abilities";
import { getGameSystemManifest } from "@/api/gameSystems";
import { getWorld } from "@/api/world";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { Field } from "@/components/ui/field/Field";
import { Input } from "@/components/ui/input";
import { Loader } from "@/components/ui/loader/Loader";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { Textarea } from "@/components/ui/textarea";
import { ModeratedContentBanner } from "@/components/world/ModeratedContentBanner";
import { useWorldRole } from "@/hooks/useWorldRole";
import type { AbilityClassification, WorldAbilityRecord } from "@/types/ability";
import type { WorldRecord } from "@/types/world";
import {
  ABILITY_CLASSIFICATION_KEYS,
  resolveAbilityLabel,
  toAbilityClassificationKey,
  type AbilityFacetsLookup,
} from "@/utils/abilityFacets";

export interface AbilityDetailPageProps {
  mode: "view" | "edit";
}

/**
 * Spec 025 (T029): the ability view/edit route, mirroring `ItemDetailPage`.
 * One component serves both modes; the router supplies `mode`.
 *
 * Notable differences from the item version:
 *   * classification renders and edits through the system's facet labels
 *     (FR-012);
 *   * a DM-only GM-only toggle (FR-024c) with a visible badge (FR-024d) —
 *     deliberately separate from the Save form, because visibility is DM-gated
 *     while editing only needs Editor;
 *   * the effect editor is NOT mounted here yet (US2/T040 adds it, gated on
 *     `canEdit` — unlike the item version, which renders it for VIEWERs).
 *
 * Share controls arrive with US6, which is gated on the DMCA determination.
 */
export default function AbilityDetailPage({ mode }: AbilityDetailPageProps) {
  const { id: worldId = "", abilityId = "" } = useParams();
  const navigate = useNavigate();

  const [world, setWorld] = useState<WorldRecord | null>(null);
  const [ability, setAbility] = useState<WorldAbilityRecord | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [classification, setClassification] = useState<AbilityClassification>("SPELL");
  const [status, setStatus] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);
  const [isTogglingVisibility, setIsTogglingVisibility] = useState(false);
  const [facets, setFacets] = useState<AbilityFacetsLookup | undefined>(undefined);

  const { isGm: isDm } = useWorldRole(worldId, world);

  useEffect(() => {
    let active = true;
    setIsLoading(true);
    setLoadError(null);

    Promise.all([getWorld(worldId), getAbility(abilityId)])
      .then(([worldResponse, abilityResponse]) => {
        if (!active) {
          return;
        }
        setWorld(worldResponse);
        setAbility(abilityResponse);
        setName(abilityResponse.name);
        setDescription(abilityResponse.description ?? "");
        setClassification(abilityResponse.classification);
      })
      .catch((err) => {
        if (active) {
          // FR-025: a GM-only ability errors identically to a nonexistent one,
          // so this message must not try to distinguish them.
          setLoadError(err instanceof Error ? err.message : "Failed to load ability");
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
  }, [worldId, abilityId]);

  useEffect(() => {
    const gameSystemId = world?.gameSystemId;
    if (!gameSystemId) {
      setFacets(undefined);
      return;
    }
    let active = true;
    getGameSystemManifest(gameSystemId)
      .then((manifest) => {
        if (active) {
          setFacets(manifest.abilityFacets as AbilityFacetsLookup | undefined);
        }
      })
      .catch(() => {
        if (active) {
          setFacets(undefined);
        }
      });
    return () => {
      active = false;
    };
  }, [world?.gameSystemId]);

  if (isLoading) {
    return <Loader fullScreen label="Loading ability" />;
  }

  if (!ability) {
    return (
      <Container className="grid max-w-2xl gap-4 py-10">
        <Card className="grid gap-3 p-6" data-testid="ability-not-found">
          <h1 className="text-lg font-semibold">Ability not found</h1>
          <p className="text-sm text-muted-foreground">
            {loadError ?? "This ability does not exist, or you do not have access to it."}
          </p>
          <Button
            variant="secondary"
            className="justify-self-start"
            onClick={() => navigate(`/world/${worldId}/compendium?tab=abilities`)}
          >
            Back to Compendium
          </Button>
        </Card>
      </Container>
    );
  }

  const canEdit = ability.myPermissionLevel !== "VIEWER";

  // Client-side convenience only — the server re-enforces on every mutation.
  if (mode === "edit" && !canEdit) {
    return <Navigate to={`/world/${worldId}/ability/${abilityId}/view`} replace />;
  }

  if (ability.moderated) {
    return (
      <Container className="grid max-w-2xl gap-4 py-10">
        <ModeratedContentBanner
          caseId={ability.moderationCaseId}
          isOwner={ability.myPermissionLevel === "OWNER"}
        />
      </Container>
    );
  }

  const classificationLabel = resolveAbilityLabel(
    facets,
    toAbilityClassificationKey(ability.classification),
  );

  const handleSave = async () => {
    setIsSaving(true);
    setStatus(null);
    try {
      const updated = await updateAbility({
        abilityId,
        name: name.trim() || ability.name,
        description: description.trim() || null,
        classification,
        // Blank the field explicitly rather than relying on a null, which
        // is indistinguishable from "omitted" over the wire.
        clearDescription: description.trim() === "",
      });
      setAbility(updated);
      setStatus("Saved.");
    } catch (err) {
      setStatus(err instanceof Error ? err.message : "Failed to save ability");
    } finally {
      setIsSaving(false);
    }
  };

  const handleDelete = async () => {
    setIsDeleting(true);
    setStatus(null);
    try {
      await deleteAbility(abilityId);
      navigate(`/world/${worldId}/compendium?tab=abilities`);
    } catch (err) {
      setStatus(err instanceof Error ? err.message : "Failed to delete ability");
      setIsDeleting(false);
    }
  };

  const handleToggleGmOnly = async () => {
    setIsTogglingVisibility(true);
    setStatus(null);
    try {
      const updated = await setAbilityGmOnly(abilityId, !ability.gmOnly);
      setAbility(updated);
      setStatus(updated.gmOnly ? "Hidden from players." : "Visible to players.");
    } catch (err) {
      setStatus(err instanceof Error ? err.message : "Failed to change visibility");
    } finally {
      setIsTogglingVisibility(false);
    }
  };

  return (
    <>
      <SEO
        title={`${ability.name} — ${mode === "edit" ? "Edit" : "View"}`}
        description="Ability detail"
        noindex
      />
      <Container className="grid max-w-2xl gap-6 py-10">
        <Button
          variant="ghost"
          size="sm"
          icon="arrow-left"
          className="justify-self-start"
          onClick={() => navigate(`/world/${worldId}/compendium?tab=abilities`)}
        >
          Back to Compendium
        </Button>

        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
              {classificationLabel}
            </p>
            <h1 className="text-2xl font-semibold">
              {ability.name}
              {ability.gmOnly ? (
                <span
                  className="ml-2 rounded bg-muted px-2 py-0.5 align-middle text-xs font-normal text-muted-foreground"
                  data-testid="ability-gm-only-badge"
                  title="Hidden from players"
                >
                  GM-only
                </span>
              ) : null}
            </h1>
          </div>
          <div className="flex gap-2">
            {canEdit && mode === "view" ? (
              <Button
                variant="secondary"
                onClick={() => navigate(`/world/${worldId}/ability/${abilityId}/edit`)}
                data-testid="ability-edit-button"
              >
                Edit
              </Button>
            ) : null}
            {/* FR-024c: DM-only. Ability-level Owner is deliberately not
                sufficient, so this is gated on isDm, not canEdit. */}
            {isDm ? (
              <Button
                variant="secondary"
                onClick={() => void handleToggleGmOnly()}
                disabled={isTogglingVisibility}
                data-testid="ability-gm-only-toggle"
              >
                {ability.gmOnly ? "Reveal to players" : "Make GM-only"}
              </Button>
            ) : null}
            {mode === "edit" && ability.myPermissionLevel === "OWNER" ? (
              <Button
                variant="danger"
                onClick={() => void handleDelete()}
                disabled={isDeleting}
                data-testid="ability-delete-button"
              >
                Delete
              </Button>
            ) : null}
          </div>
        </div>

        <Card className="grid gap-4 p-5">
          {mode === "edit" ? (
            <>
              <Field label="Name" htmlFor="ability-name">
                <Input
                  id="ability-name"
                  value={name}
                  onChange={(event) => setName(event.target.value)}
                  disabled={isSaving}
                  data-testid="ability-name-input"
                />
              </Field>
              <Field label="Type" htmlFor="ability-classification">
                <select
                  id="ability-classification"
                  className="w-full rounded-md border border-border bg-background px-2 py-2 text-sm"
                  value={classification}
                  onChange={(event) =>
                    setClassification(event.target.value as AbilityClassification)
                  }
                  disabled={isSaving}
                  data-testid="ability-classification-select"
                >
                  {ABILITY_CLASSIFICATION_KEYS.map((key) => (
                    <option key={key} value={key.toUpperCase()}>
                      {resolveAbilityLabel(facets, key)}
                    </option>
                  ))}
                </select>
              </Field>
              <Field label="Description" htmlFor="ability-description">
                <Textarea
                  id="ability-description"
                  rows={4}
                  value={description}
                  onChange={(event) => setDescription(event.target.value)}
                  disabled={isSaving}
                  data-testid="ability-description-input"
                />
              </Field>
              <div className="flex items-center gap-3">
                <Button
                  onClick={() => void handleSave()}
                  disabled={isSaving}
                  data-testid="ability-save-button"
                >
                  Save
                </Button>
                <Button
                  variant="ghost"
                  onClick={() => navigate(`/world/${worldId}/ability/${abilityId}/view`)}
                  disabled={isSaving}
                >
                  Cancel
                </Button>
                {status ? <StatusBadge>{status}</StatusBadge> : null}
              </div>
            </>
          ) : (
            <>
              <p className="text-sm whitespace-pre-wrap" data-testid="ability-description">
                {ability.description || (
                  <span className="text-muted-foreground italic">No description.</span>
                )}
              </p>
              {status ? <StatusBadge>{status}</StatusBadge> : null}
            </>
          )}
        </Card>
      </Container>
    </>
  );
}
