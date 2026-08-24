import { useEffect, useState } from "react";
import { Link, Navigate, useNavigate, useParams } from "react-router-dom";
import {
  calculateMaxWishPoints,
  CharacterSheet as GenieCharacterSheet,
  GENIE_CONDITIONS,
  type GenieAbilityData,
  type GenieProficiencyData,
  type GenieResourceData,
} from "@thunderforge/genie";
import { updateActorSystemData } from "@/api/actorSystemData";
import { createActorShareLink, revokeActorShareLink } from "@/api/actorShares";
import { getActor, setActorAvailability, unclaimActor, updateActor } from "@/api/actors";
import { getWorld } from "@/api/world";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { Field } from "@/components/ui/field/Field";
import { Input } from "@/components/ui/input";
import { Loader } from "@/components/ui/loader/Loader";
import { Textarea } from "@/components/ui/textarea";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { useActorSystemData } from "@/hooks/useActorSystemData";
import { useUpdateTraitData } from "@/hooks/useUpdateActorData";
import { useWorldRole } from "@/hooks/useWorldRole";
import { ActorInventoryPanel } from "@/pages/world/actor/ActorInventoryPanel";
import { ActorOwnershipBlock } from "@/pages/world/actor/ActorOwnershipBlock";
import type { WorldActorRecord } from "@/types/actor";
import type { WorldRecord } from "@/types/world";

const DEFAULT_GENIE_ABILITIES: GenieAbilityData = { might: 0, cunning: 0, spirit: 0 };
const DEFAULT_GENIE_PROFICIENCIES: GenieProficiencyData = { trained_skills: [] };

/**
 * Spec 018 (US1/US4/US6): the Genie system's character sheet — abilities,
 * skills, and the conditions track (as CharacterSheet's own "Conditions"
 * tab) — plus a GM/owner condition-editing control. `useActorSystemData`/
 * `useUpdateTraitData` are the same hooks dnd5e's sheet already uses;
 * genie's `ability_data`/`proficiency_data`/`trait_data` shapes are
 * whatever this actor's system data row already holds (or sensible
 * defaults if it has none yet).
 */
function GenieActorSheet({ actor, canEdit }: { actor: WorldActorRecord; canEdit: boolean }) {
  const { data, refetch } = useActorSystemData(actor.id, "genie");
  const { updateTraits, isPending } = useUpdateTraitData(actor.id, "genie");

  const abilityData = (data?.ability_data as GenieAbilityData | undefined) ?? DEFAULT_GENIE_ABILITIES;
  const proficiencyData =
    (data?.proficiency_data as GenieProficiencyData | undefined) ?? DEFAULT_GENIE_PROFICIENCIES;
  const activeConditions: string[] = Array.isArray(data?.trait_data?.active_conditions)
    ? (data!.trait_data!.active_conditions as string[])
    : [];
  const level: number = typeof data?.trait_data?.level === "number" ? data.trait_data.level : 1;
  const resourceData: GenieResourceData = (data?.resource_data as GenieResourceData | undefined) ?? {
    current_wish_points: 0,
    max_wish_points: calculateMaxWishPoints(level),
    current_health: 1,
    max_health: 1,
  };

  const handleAbilityChange = async (ability: keyof GenieAbilityData, value: number) => {
    await updateActorSystemData(actor.id, "genie", "ability_data", {
      ...abilityData,
      [ability]: value,
    });
    await refetch();
  };

  const toggleCondition = async (key: string) => {
    const next = activeConditions.includes(key)
      ? activeConditions.filter((c) => c !== key)
      : [...activeConditions, key];
    await updateTraits({ ...(data?.trait_data ?? {}), active_conditions: next });
    await refetch();
  };

  const handleLevelChange = async (newLevel: number) => {
    await updateTraits({ ...(data?.trait_data ?? {}), level: newLevel });
    const newMaxWishPoints = calculateMaxWishPoints(newLevel);
    await updateActorSystemData(actor.id, "genie", "resource_data", {
      ...resourceData,
      max_wish_points: newMaxWishPoints,
      current_wish_points: Math.min(resourceData.current_wish_points, newMaxWishPoints),
    });
    await refetch();
  };

  const handleResourceChange = async (field: keyof GenieResourceData, value: number) => {
    await updateActorSystemData(actor.id, "genie", "resource_data", {
      ...resourceData,
      [field]: value,
    });
    await refetch();
  };

  return (
    <Card className="grid gap-4 p-4" data-testid="genie-actor-sheet">
      <GenieCharacterSheet
        character={{
          id: actor.id,
          name: actor.label,
          abilityData,
          proficiencyData,
          activeConditions,
          level,
          resourceData,
        }}
        isEditable={canEdit}
        onAbilityChange={(ability, value) => void handleAbilityChange(ability, value)}
        onLevelChange={(newLevel) => void handleLevelChange(newLevel)}
        onResourceChange={(field, value) => void handleResourceChange(field, value)}
      />
      {canEdit ? (
        <div className="grid gap-2 border-t pt-4" data-testid="genie-condition-editor">
          <h3 className="text-sm font-semibold tracking-wide text-muted-foreground uppercase">
            Conditions (edit)
          </h3>
          {GENIE_CONDITIONS.map((condition) => (
            <label key={condition.key} className="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                checked={activeConditions.includes(condition.key)}
                disabled={isPending}
                onChange={() => void toggleCondition(condition.key)}
              />
              {condition.label}
            </label>
          ))}
        </div>
      ) : null}
    </Card>
  );
}

export interface ActorDetailPageProps {
  mode: "view" | "edit";
}

/**
 * Spec 010 (US4): `/world/:id/actor/:actorId/view` and `.../edit` —
 * dedicated, linkable actor detail routes. View mode is available to
 * anyone with at least Viewer access (default); edit mode requires
 * Editor-or-Owner `myPermissionLevel` (server-enforced regardless of this
 * client-side redirect, FR-011). DM viewers additionally see the
 * ownership block (US3) and a "Share" action (US5, Owner-level only).
 */
export default function ActorDetailPage({ mode }: ActorDetailPageProps) {
  const { id: worldId = "", actorId = "" } = useParams();
  const navigate = useNavigate();
  const [world, setWorld] = useState<WorldRecord | null>(null);
  const [actor, setActor] = useState<WorldActorRecord | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [label, setLabel] = useState("");
  const [description, setDescription] = useState("");
  const [isNpc, setIsNpc] = useState(true);
  const [status, setStatus] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const [shareLink, setShareLink] = useState<string | null>(null);
  const [shareLinkId, setShareLinkId] = useState<string | null>(null);
  const [isSharing, setIsSharing] = useState(false);
  const [isRevoking, setIsRevoking] = useState(false);
  const [isUpdatingClaim, setIsUpdatingClaim] = useState(false);
  const { isGm: isDm } = useWorldRole(worldId, world);

  useEffect(() => {
    let active = true;
    setIsLoading(true);

    Promise.all([getWorld(worldId), getActor(worldId, actorId)])
      .then(([worldResult, actorResult]) => {
        if (!active) {
          return;
        }
        setWorld(worldResult);
        setActor(actorResult);
        if (actorResult) {
          setLabel(actorResult.label);
          setDescription(actorResult.description ?? "");
          setIsNpc(actorResult.isNpc);
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
  }, [worldId, actorId]);

  if (isLoading) {
    return <Loader fullScreen label="Loading actor" />;
  }

  if (!actor) {
    return (
      <Container>
        <main className="grid min-h-[60vh] place-items-center py-16">
          <Card className="grid gap-3 p-6 text-center">
            <h1 className="text-xl font-semibold">Actor not found</h1>
            <p className="text-muted-foreground">
              This actor doesn't exist or you don't have access to it.
            </p>
            <Link to={`/world/${worldId}/staging`} className="text-primary hover:underline">
              Back to staging
            </Link>
          </Card>
        </main>
      </Container>
    );
  }

  // FR-011: a Viewer-only caller reaching /edit is redirected to /view —
  // the server independently rejects the mutation regardless (Principle III).
  if (mode === "edit" && actor.myPermissionLevel === "VIEWER") {
    return <Navigate to={`/world/${worldId}/actor/${actorId}/view`} replace />;
  }

  const canEdit = actor.myPermissionLevel !== "VIEWER";
  const canShare = actor.myPermissionLevel === "OWNER";

  const handleSave = async () => {
    setIsSaving(true);
    setStatus(null);
    try {
      const updated = await updateActor({ actorId, label, isNpc, description });
      setActor(updated);
      setStatus("Saved.");
    } catch (err) {
      setStatus(err instanceof Error ? err.message : "Failed to save actor");
    } finally {
      setIsSaving(false);
    }
  };

  const handleShare = async () => {
    setIsSharing(true);
    setStatus(null);
    try {
      const link = await createActorShareLink(actorId);
      const url = `${window.location.origin}/shared/actor/${link.shareCode}`;
      setShareLink(url);
      setShareLinkId(link.id);
      await navigator.clipboard.writeText(url).catch(() => {});
    } catch (err) {
      setStatus(err instanceof Error ? err.message : "Failed to create share link");
    } finally {
      setIsSharing(false);
    }
  };

  const handleToggleAvailability = async (available: boolean) => {
    setIsUpdatingClaim(true);
    setStatus(null);
    try {
      const updated = await setActorAvailability(actorId, available);
      setActor(updated);
    } catch (err) {
      setStatus(err instanceof Error ? err.message : "Failed to update availability");
    } finally {
      setIsUpdatingClaim(false);
    }
  };

  const handleUnclaim = async () => {
    setIsUpdatingClaim(true);
    setStatus(null);
    try {
      const updated = await unclaimActor(actorId);
      setActor(updated);
    } catch (err) {
      setStatus(err instanceof Error ? err.message : "Failed to unclaim character");
    } finally {
      setIsUpdatingClaim(false);
    }
  };

  const handleRevokeShare = async () => {
    if (!shareLinkId) {
      return;
    }
    setIsRevoking(true);
    setStatus(null);
    try {
      await revokeActorShareLink(shareLinkId);
      setShareLink(null);
      setShareLinkId(null);
      setStatus("Share link revoked.");
    } catch (err) {
      setStatus(err instanceof Error ? err.message : "Failed to revoke share link");
    } finally {
      setIsRevoking(false);
    }
  };

  return (
    <>
      <SEO
        title={`${actor.label} — ${mode === "edit" ? "Edit" : "View"}`}
        description="Actor detail"
        noindex
      />
      <Container className="grid max-w-2xl gap-6 py-10">
        <Button
          variant="ghost"
          size="sm"
          icon="arrow-left"
          className="justify-self-start"
          onClick={() => navigate(`/world/${worldId}/staging`)}
        >
          Back to world
        </Button>

        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
              {actor.isNpc ? "NPC" : "Player Character"}
            </p>
            <h1 className="text-2xl font-semibold">{actor.label}</h1>
          </div>
          <div className="flex gap-2">
            {canEdit && mode === "view" ? (
              <Button
                variant="secondary"
                onClick={() => navigate(`/world/${worldId}/actor/${actorId}/edit`)}
              >
                Edit
              </Button>
            ) : null}
            {canShare ? (
              <Button variant="secondary" icon="link" onClick={() => void handleShare()} disabled={isSharing}>
                {isSharing ? "Sharing..." : "Share"}
              </Button>
            ) : null}
          </div>
        </div>

        {shareLink ? (
          <Card className="grid gap-2 p-4">
            <p className="text-sm text-muted-foreground">Share link copied to clipboard:</p>
            <Input readOnly value={shareLink} data-testid="share-link-input" />
            <Button
              variant="ghost"
              size="sm"
              className="justify-self-start"
              onClick={() => void handleRevokeShare()}
              disabled={isRevoking}
            >
              {isRevoking ? "Revoking..." : "Revoke link"}
            </Button>
          </Card>
        ) : null}

        <Card className="grid gap-4 p-6">
          {mode === "edit" ? (
            <>
              <Field label="Name" htmlFor="actor-label">
                <Input id="actor-label" value={label} onChange={(e) => setLabel(e.target.value)} />
              </Field>
              <Field label="Description" htmlFor="actor-description">
                <Textarea
                  id="actor-description"
                  value={description}
                  onChange={(e) => setDescription(e.target.value)}
                  placeholder="A short description of this actor…"
                  rows={4}
                />
              </Field>
              <label className="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={!isNpc}
                  onChange={(e) => setIsNpc(!e.target.checked)}
                />
                This is a player character
              </label>
              <div className="flex gap-3">
                <Button onClick={() => void handleSave()} disabled={isSaving}>
                  {isSaving ? "Saving..." : "Save"}
                </Button>
                <Button
                  variant="ghost"
                  onClick={() => navigate(`/world/${worldId}/actor/${actorId}/view`)}
                >
                  Cancel
                </Button>
              </div>
              {status ? <StatusBadge variant={status === "Saved." ? "success" : "danger"}>{status}</StatusBadge> : null}
            </>
          ) : (
            <div className="grid gap-2">
              <p className="text-sm text-muted-foreground">
                Classification: {actor.isNpc ? "Non-Player Character" : "Player Character"}
              </p>
              <p className="text-sm text-muted-foreground">Type: {actor.actorType}</p>
              {actor.gameSystemId ? (
                <p className="text-sm text-muted-foreground">Game system: {actor.gameSystemId}</p>
              ) : null}
              <p className="text-sm whitespace-pre-wrap">
                {actor.description || (
                  <span className="text-muted-foreground italic">No description.</span>
                )}
              </p>
            </div>
          )}
        </Card>

        {/* Spec 012 (T037, FR-006): lore entries that reference this actor. */}
        <Card className="grid gap-2 p-4" data-testid="actor-lore-linked-from">
          <h2 className="text-sm font-semibold tracking-wide text-muted-foreground uppercase">
            Linked from (lore)
          </h2>
          {actor.loreLinkedFrom.length === 0 ? (
            <p className="text-sm text-muted-foreground italic">No lore entries link here yet.</p>
          ) : (
            <ul className="grid gap-1">
              {actor.loreLinkedFrom.map((source) => (
                <li key={source.id}>
                  <Link
                    to={`/world/${worldId}/lore/${source.slug}/view`}
                    className="text-sm text-primary hover:underline"
                  >
                    {source.title}
                  </Link>
                </li>
              ))}
            </ul>
          )}
        </Card>

        {actor.gameSystemId === "genie" ? (
          <GenieActorSheet actor={actor} canEdit={canEdit && mode === "edit"} />
        ) : null}

        <ActorInventoryPanel actorId={actorId} worldId={worldId} canManage={canEdit} />

        {/* Spec 017 (T028, US3): GM-only, PC-only "available for claiming"
            control plus who currently has this character claimed. */}
        {isDm && !actor.isNpc ? (
          <Card className="grid gap-3 p-4" data-testid="actor-claim-block">
            <h2 className="text-sm font-semibold tracking-wide text-muted-foreground uppercase">
              Player claiming
            </h2>
            {actor.claimedBy ? (
              <div className="flex flex-wrap items-center justify-between gap-3">
                <p className="text-sm text-muted-foreground">
                  Claimed by <span className="font-medium text-foreground">{actor.claimedBy.username}</span>
                </p>
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={() => void handleUnclaim()}
                  disabled={isUpdatingClaim}
                >
                  {isUpdatingClaim ? "Un-claiming..." : "Un-claim"}
                </Button>
              </div>
            ) : (
              <label className="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={actor.availableForClaim}
                  disabled={isUpdatingClaim}
                  onChange={(e) => void handleToggleAvailability(e.target.checked)}
                />
                Available for a joining player to claim
              </label>
            )}
          </Card>
        ) : null}

        {isDm && mode === "edit" ? (
          <ActorOwnershipBlock actorId={actorId} worldId={worldId} world={world} />
        ) : null}
      </Container>
    </>
  );
}
