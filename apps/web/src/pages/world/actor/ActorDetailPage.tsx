import { useEffect, useState } from "react";
import { Link, Navigate, useNavigate, useParams } from "react-router-dom";
import { createActorShareLink, revokeActorShareLink } from "@/api/actorShares";
import { getActor, updateActor } from "@/api/actors";
import { getWorld } from "@/api/world";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { Field } from "@/components/ui/field/Field";
import { Input } from "@/components/ui/input";
import { Loader } from "@/components/ui/loader/Loader";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { useWorldRole } from "@/hooks/useWorldRole";
import { ActorOwnershipBlock } from "@/pages/world/actor/ActorOwnershipBlock";
import type { WorldActorRecord } from "@/types/actor";
import type { WorldRecord } from "@/types/world";

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
  const [isNpc, setIsNpc] = useState(true);
  const [status, setStatus] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const [shareLink, setShareLink] = useState<string | null>(null);
  const [shareLinkId, setShareLinkId] = useState<string | null>(null);
  const [isSharing, setIsSharing] = useState(false);
  const [isRevoking, setIsRevoking] = useState(false);
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
      const updated = await updateActor({ actorId, label, isNpc });
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
            </div>
          )}
        </Card>

        {isDm && mode === "edit" ? (
          <ActorOwnershipBlock actorId={actorId} worldId={worldId} world={world} />
        ) : null}
      </Container>
    </>
  );
}
