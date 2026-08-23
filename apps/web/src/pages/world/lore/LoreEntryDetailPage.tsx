import { useEffect, useState } from "react";
import { Link, Navigate, useNavigate, useParams } from "react-router-dom";
import { deleteLoreEntry, getLoreEntry, updateLoreEntry } from "@/api/lore";
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
import { LoreMarkdownEditor } from "@/pages/world/lore/LoreMarkdownEditor";
import { LoreMarkdownRenderer } from "@/pages/world/lore/LoreMarkdownRenderer";
import { LoreOwnershipBlock } from "@/pages/world/lore/LoreOwnershipBlock";
import type { LoreEntryRecord } from "@/types/lore";
import type { WorldRecord } from "@/types/world";

export interface LoreEntryDetailPageProps {
  mode: "view" | "edit";
}

/**
 * Spec 012 (T029, T036, T044, T045, T050): `/world/:id/lore/:slug/view`
 * and `.../edit` — dedicated, linkable, shareable lore entry routes.
 * Mirrors `ActorDetailPage.tsx`'s structure closely. View mode is
 * available to anyone with at least Viewer access (default); edit mode
 * requires Editor-or-Owner `myPermissionLevel` (server-enforced
 * regardless of this client-side redirect, Principle III). DM viewers
 * additionally see the ownership block (T029c) while editing.
 */
export default function LoreEntryDetailPage({ mode }: LoreEntryDetailPageProps) {
  const { id: worldId = "", slug = "" } = useParams();
  const navigate = useNavigate();
  const [world, setWorld] = useState<WorldRecord | null>(null);
  const [entry, setEntry] = useState<LoreEntryRecord | null | undefined>(undefined);
  const [title, setTitle] = useState("");
  const [content, setContent] = useState("");
  const [status, setStatus] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);
  const [copyStatus, setCopyStatus] = useState<string | null>(null);
  const { isGm: isDm } = useWorldRole(worldId, world);

  useEffect(() => {
    let active = true;
    setEntry(undefined);

    Promise.all([getWorld(worldId), getLoreEntry(worldId, slug)])
      .then(([worldResult, entryResult]) => {
        if (!active) {
          return;
        }
        setWorld(worldResult);
        setEntry(entryResult);
        if (entryResult) {
          setTitle(entryResult.title);
          setContent(entryResult.content);
        }
      })
      .catch(() => {
        if (active) {
          setEntry(null);
        }
      });

    return () => {
      active = false;
    };
  }, [worldId, slug]);

  if (entry === undefined) {
    return <Loader fullScreen label="Loading lore entry" />;
  }

  // T045: graceful not-found state — a stale slug (FR-014) or denied
  // access (FR-015) both surface as `null` from the query, never an error.
  if (!entry) {
    return (
      <Container>
        <main className="grid min-h-[60vh] place-items-center py-16">
          <Card className="grid gap-3 p-6 text-center">
            <h1 className="text-xl font-semibold">Lore entry not found</h1>
            <p className="text-muted-foreground">
              This entry doesn't exist, its link may be out of date, or you don't have access to
              it.
            </p>
            <Link to={`/world/${worldId}/compendium`} className="text-primary hover:underline">
              Back to compendium
            </Link>
          </Card>
        </main>
      </Container>
    );
  }

  // A Viewer-only caller reaching /edit is redirected to /view — the
  // server independently rejects the mutation regardless (Principle III).
  if (mode === "edit" && entry.myPermissionLevel === "VIEWER") {
    return <Navigate to={`/world/${worldId}/lore/${entry.slug}/view`} replace />;
  }

  const canEdit = entry.myPermissionLevel !== "VIEWER";
  const canDelete = entry.myPermissionLevel === "OWNER";

  const handleSave = async () => {
    setIsSaving(true);
    setStatus(null);
    try {
      const updated = await updateLoreEntry({
        loreEntryId: entry.id,
        title: title !== entry.title ? title : undefined,
        content: content !== entry.content ? content : undefined,
        expectedCurrentRevisionId: entry.currentRevisionId,
      });
      setEntry(updated);
      setTitle(updated.title);
      setContent(updated.content);
      setStatus("Saved.");
      if (updated.slug !== slug) {
        navigate(`/world/${worldId}/lore/${updated.slug}/edit`, { replace: true });
      }
    } catch (err) {
      setStatus(err instanceof Error ? err.message : "Failed to save lore entry");
    } finally {
      setIsSaving(false);
    }
  };

  const handleDelete = async () => {
    setIsDeleting(true);
    setStatus(null);
    try {
      await deleteLoreEntry(entry.id);
      navigate(`/world/${worldId}/compendium`);
    } catch (err) {
      setStatus(err instanceof Error ? err.message : "Failed to delete lore entry");
      setIsDeleting(false);
    }
  };

  const handleCopyLink = async () => {
    const url = `${window.location.origin}/world/${worldId}/lore/${entry.slug}/view`;
    try {
      await navigator.clipboard.writeText(url);
      setCopyStatus("Link copied.");
    } catch {
      setCopyStatus(url);
    }
    setTimeout(() => setCopyStatus(null), 3000);
  };

  return (
    <>
      <SEO title={`${entry.title} — ${mode === "edit" ? "Edit" : "View"}`} description="Lore entry" noindex />
      <Container className="grid max-w-3xl gap-6 py-10">
        <Button
          variant="ghost"
          size="sm"
          icon="arrow-left"
          className="justify-self-start"
          onClick={() => navigate(`/world/${worldId}/compendium`)}
        >
          Back to compendium
        </Button>

        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">Lore entry</p>
            <h1 className="text-2xl font-semibold">{entry.title}</h1>
          </div>
          <div className="flex flex-wrap gap-2">
            {canEdit && mode === "view" ? (
              <Button
                variant="secondary"
                onClick={() => navigate(`/world/${worldId}/lore/${entry.slug}/edit`)}
              >
                Edit
              </Button>
            ) : null}
            <Button variant="ghost" icon="link" onClick={() => void handleCopyLink()}>
              Copy link
            </Button>
            <Button
              asChild
              variant="ghost"
              icon="rune"
              data-testid="lore-view-history-link"
            >
              <Link to={`/world/${worldId}/lore/${entry.slug}/history`}>History</Link>
            </Button>
            {canDelete && mode === "edit" ? (
              <Button
                variant="danger"
                icon="trash"
                onClick={() => void handleDelete()}
                disabled={isDeleting}
              >
                {isDeleting ? "Deleting..." : "Delete"}
              </Button>
            ) : null}
          </div>
        </div>

        {copyStatus ? <StatusBadge variant="info">{copyStatus}</StatusBadge> : null}

        <Card className="grid gap-4 p-6">
          {mode === "edit" ? (
            <>
              <Field label="Title" htmlFor="lore-entry-title">
                <Input id="lore-entry-title" value={title} onChange={(e) => setTitle(e.target.value)} />
              </Field>
              <Field label="Content" htmlFor="lore-entry-content">
                <LoreMarkdownEditor
                  loreEntryId={entry.id}
                  worldId={worldId}
                  value={content}
                  onChange={setContent}
                  disabled={isSaving}
                />
              </Field>
              <div className="flex gap-3">
                <Button onClick={() => void handleSave()} disabled={isSaving}>
                  {isSaving ? "Saving..." : "Save"}
                </Button>
                <Button
                  variant="ghost"
                  onClick={() => navigate(`/world/${worldId}/lore/${entry.slug}/view`)}
                >
                  Cancel
                </Button>
              </div>
              {status ? (
                <StatusBadge variant={status === "Saved." ? "success" : "danger"}>{status}</StatusBadge>
              ) : null}
            </>
          ) : (
            <LoreMarkdownRenderer html={entry.renderedHtml} />
          )}
        </Card>

        {/* T036: automatically-maintained "linked from" backlink list (FR-006). */}
        <Card className="grid gap-2 p-4" data-testid="lore-linked-from">
          <h2 className="text-sm font-semibold tracking-wide text-muted-foreground uppercase">
            Linked from
          </h2>
          {entry.linkedFrom.length === 0 ? (
            <p className="text-sm text-muted-foreground italic">No other lore entries link here yet.</p>
          ) : (
            <ul className="grid gap-1">
              {entry.linkedFrom.map((source) => (
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

        {isDm && mode === "edit" ? (
          <LoreOwnershipBlock loreEntryId={entry.id} worldId={worldId} world={world} />
        ) : null}
      </Container>
    </>
  );
}
