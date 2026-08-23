import { useEffect, useState } from "react";
import { Link, Navigate, useNavigate, useParams } from "react-router-dom";
import { createItemShareLink, revokeItemShareLink } from "@/api/itemShares";
import { deleteItem, getItem, updateItem } from "@/api/items";
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
import { useWorldRole } from "@/hooks/useWorldRole";
import { ItemEffectEditor } from "@/pages/world/item/ItemEffectEditor";
import { ItemOwnershipBlock } from "@/pages/world/item/ItemOwnershipBlock";
import type { WorldItemRecord } from "@/types/item";
import type { WorldRecord } from "@/types/world";

export interface ItemDetailPageProps {
  mode: "view" | "edit";
}

/**
 * Spec 013 (T028, User Story 1): `/world/:id/item/:itemId/view` and
 * `.../edit` — dedicated, linkable item detail routes, mirrors
 * ActorDetailPage.tsx. View mode is available to anyone with at least
 * Viewer access (default, FR-008); edit mode requires Editor-or-Owner
 * `myPermissionLevel` (server-enforced regardless of this client-side
 * redirect, per Principle III). DM viewers additionally see the
 * ownership block (FR-003) and a "Share" action (US5, Owner-level only).
 */
export default function ItemDetailPage({ mode }: ItemDetailPageProps) {
  const { id: worldId = "", itemId = "" } = useParams();
  const navigate = useNavigate();
  const [world, setWorld] = useState<WorldRecord | null>(null);
  const [item, setItem] = useState<WorldItemRecord | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [status, setStatus] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);
  const [shareLink, setShareLink] = useState<string | null>(null);
  const [shareLinkId, setShareLinkId] = useState<string | null>(null);
  const [isSharing, setIsSharing] = useState(false);
  const [isRevoking, setIsRevoking] = useState(false);
  const { isGm: isDm } = useWorldRole(worldId, world);

  useEffect(() => {
    let active = true;
    setIsLoading(true);

    Promise.all([getWorld(worldId), getItem(itemId)])
      .then(([worldResult, itemResult]) => {
        if (!active) {
          return;
        }
        setWorld(worldResult);
        setItem(itemResult);
        if (itemResult) {
          setName(itemResult.name);
          setDescription(itemResult.description ?? "");
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
  }, [worldId, itemId]);

  if (isLoading) {
    return <Loader fullScreen label="Loading item" />;
  }

  if (!item) {
    return (
      <Container>
        <main className="grid min-h-[60vh] place-items-center py-16">
          <Card className="grid gap-3 p-6 text-center">
            <h1 className="text-xl font-semibold">Item not found</h1>
            <p className="text-muted-foreground">
              This item doesn't exist or you don't have access to it.
            </p>
            <Link to={`/world/${worldId}/compendium`} className="text-primary hover:underline">
              Back to Compendium
            </Link>
          </Card>
        </main>
      </Container>
    );
  }

  // A Viewer-only caller reaching /edit is redirected to /view — the
  // server independently rejects the mutation regardless (Principle III).
  if (mode === "edit" && item.myPermissionLevel === "VIEWER") {
    return <Navigate to={`/world/${worldId}/item/${itemId}/view`} replace />;
  }

  const canEdit = item.myPermissionLevel !== "VIEWER";
  const canShare = item.myPermissionLevel === "OWNER";

  const handleSave = async () => {
    setIsSaving(true);
    setStatus(null);
    try {
      const updated = await updateItem({ itemId, name, description });
      setItem(updated);
      setStatus("Saved.");
    } catch (err) {
      setStatus(err instanceof Error ? err.message : "Failed to save item");
    } finally {
      setIsSaving(false);
    }
  };

  const handleDelete = async () => {
    setIsDeleting(true);
    setStatus(null);
    try {
      await deleteItem(itemId);
      navigate(`/world/${worldId}/compendium`);
    } catch (err) {
      setStatus(err instanceof Error ? err.message : "Failed to delete item");
      setIsDeleting(false);
    }
  };

  const handleShare = async () => {
    setIsSharing(true);
    setStatus(null);
    try {
      const link = await createItemShareLink(itemId);
      const url = `${window.location.origin}/shared/item/${link.shareCode}`;
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
      await revokeItemShareLink(shareLinkId);
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
      <SEO title={`${item.name} — ${mode === "edit" ? "Edit" : "View"}`} description="Item detail" noindex />
      <Container className="grid max-w-2xl gap-6 py-10">
        <Button
          variant="ghost"
          size="sm"
          icon="arrow-left"
          className="justify-self-start"
          onClick={() => navigate(`/world/${worldId}/compendium`)}
        >
          Back to Compendium
        </Button>

        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">Item</p>
            <h1 className="text-2xl font-semibold">{item.name}</h1>
          </div>
          {/* Share/Copy controls (US5) */}
          <div className="flex gap-2">
            {canEdit && mode === "view" ? (
              <Button variant="secondary" onClick={() => navigate(`/world/${worldId}/item/${itemId}/edit`)}>
                Edit
              </Button>
            ) : null}
            {canShare ? (
              <Button variant="secondary" icon="link" onClick={() => void handleShare()} disabled={isSharing}>
                {isSharing ? "Sharing..." : "Share"}
              </Button>
            ) : null}
            {mode === "edit" && item.myPermissionLevel === "OWNER" ? (
              <Button variant="danger" onClick={() => void handleDelete()} disabled={isDeleting}>
                {isDeleting ? "Deleting..." : "Delete"}
              </Button>
            ) : null}
          </div>
        </div>

        {shareLink ? (
          <Card className="grid gap-2 p-4">
            <p className="text-sm text-muted-foreground">Share link copied to clipboard:</p>
            <Input readOnly value={shareLink} data-testid="item-share-link-input" />
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
              <Field label="Name" htmlFor="item-name">
                <Input id="item-name" value={name} onChange={(e) => setName(e.target.value)} />
              </Field>
              <Field label="Description" htmlFor="item-description">
                <Textarea
                  id="item-description"
                  value={description}
                  onChange={(e) => setDescription(e.target.value)}
                  placeholder="A short description of this item…"
                  rows={4}
                />
              </Field>
              <div className="flex gap-3">
                <Button onClick={() => void handleSave()} disabled={isSaving}>
                  {isSaving ? "Saving..." : "Save"}
                </Button>
                <Button variant="ghost" onClick={() => navigate(`/world/${worldId}/item/${itemId}/view`)}>
                  Cancel
                </Button>
              </div>
              {status ? (
                <StatusBadge variant={status === "Saved." ? "success" : "danger"}>{status}</StatusBadge>
              ) : null}
            </>
          ) : (
            <div className="grid gap-2">
              <p className="text-sm whitespace-pre-wrap">
                {item.description || <span className="text-muted-foreground italic">No description.</span>}
              </p>
            </div>
          )}
        </Card>

        <ItemEffectEditor
          itemId={itemId}
          effects={item.effects}
          onChanged={(effects) => setItem((current) => (current ? { ...current, effects } : current))}
        />

        {isDm && mode === "edit" ? <ItemOwnershipBlock itemId={itemId} worldId={worldId} world={world} /> : null}
      </Container>
    </>
  );
}
