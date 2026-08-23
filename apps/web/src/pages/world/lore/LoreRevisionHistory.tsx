import { useEffect, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { getLoreEntry, getLoreEntryRevisions, restoreLoreRevision } from "@/api/lore";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { Loader } from "@/components/ui/loader/Loader";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { LoreMarkdownRenderer } from "@/pages/world/lore/LoreMarkdownRenderer";
import type { LoreEntryRecord, LoreRevisionRecord } from "@/types/lore";
import { cn } from "@/lib/utils";

/**
 * Spec 012 (T049, FR-017/018): `/world/:id/lore/:slug/history` — full
 * revision history for a lore entry (newest first), a single-revision
 * viewer, and a restore action. Every entry has at least Viewer access
 * to reach this page; restoring requires Editor/Owner, enforced
 * server-side (`restoreLoreRevision`, Principle III) regardless of any
 * client-side gating here.
 */
export default function LoreRevisionHistory() {
  const { id: worldId = "", slug = "" } = useParams();
  const navigate = useNavigate();
  const [entry, setEntry] = useState<LoreEntryRecord | null | undefined>(undefined);
  const [revisions, setRevisions] = useState<LoreRevisionRecord[] | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [isRestoring, setIsRestoring] = useState(false);

  useEffect(() => {
    let active = true;
    setEntry(undefined);
    setRevisions(null);

    getLoreEntry(worldId, slug)
      .then((entryResult) => {
        if (!active) {
          return;
        }
        setEntry(entryResult);
        if (!entryResult) {
          return Promise.resolve();
        }
        return getLoreEntryRevisions(entryResult.id).then((rows) => {
          if (active) {
            setRevisions(rows);
            setSelectedId(rows[0]?.id ?? null);
          }
        });
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

  if (entry === undefined || (entry && revisions === null)) {
    return <Loader fullScreen label="Loading revision history" />;
  }

  if (!entry) {
    return (
      <Container>
        <main className="grid min-h-[60vh] place-items-center py-16">
          <Card className="grid gap-3 p-6 text-center">
            <h1 className="text-xl font-semibold">Lore entry not found</h1>
            <Link to={`/world/${worldId}/compendium`} className="text-primary hover:underline">
              Back to compendium
            </Link>
          </Card>
        </main>
      </Container>
    );
  }

  const rows = revisions ?? [];
  const selected = rows.find((row) => row.id === selectedId) ?? rows[0] ?? null;
  const canRestore = entry.myPermissionLevel !== "VIEWER";

  const handleRestore = async () => {
    if (!selected) {
      return;
    }
    setIsRestoring(true);
    setStatus(null);
    try {
      const updated = await restoreLoreRevision(selected.id);
      const freshRevisions = await getLoreEntryRevisions(updated.id);
      setRevisions(freshRevisions);
      setSelectedId(freshRevisions[0]?.id ?? null);
      setStatus("Restored — a new revision recording this restore has been added.");
    } catch (err) {
      setStatus(err instanceof Error ? err.message : "Failed to restore revision");
    } finally {
      setIsRestoring(false);
    }
  };

  return (
    <>
      <SEO title={`${entry.title} — History`} description="Lore entry revision history" noindex />
      <Container className="grid max-w-4xl gap-6 py-10">
        <Button
          variant="ghost"
          size="sm"
          icon="arrow-left"
          className="justify-self-start"
          onClick={() => navigate(`/world/${worldId}/lore/${entry.slug}/view`)}
        >
          Back to entry
        </Button>

        <div>
          <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
            Revision history
          </p>
          <h1 className="text-2xl font-semibold">{entry.title}</h1>
        </div>

        {status ? (
          <StatusBadge variant={status.startsWith("Restored") ? "success" : "danger"}>{status}</StatusBadge>
        ) : null}

        <div className="grid gap-4 lg:grid-cols-[1fr_2fr]">
          <div className="grid content-start gap-2" data-testid="lore-revision-list">
            {rows.map((revision) => (
              <button
                key={revision.id}
                type="button"
                onClick={() => setSelectedId(revision.id)}
                data-testid={`lore-revision-row-${revision.id}`}
                className={cn(
                  "rounded-lg border border-border p-3 text-left text-sm hover:bg-muted/40",
                  selected?.id === revision.id && "bg-muted",
                )}
              >
                <p className="font-medium">{new Date(revision.createdAt).toLocaleString()}</p>
                <p className="text-xs text-muted-foreground">
                  {revision.restoredFromRevisionId ? "Restored revision" : "Edit"}
                </p>
              </button>
            ))}
          </div>

          <Card className="grid gap-4 p-6">
            {selected ? (
              <>
                <LoreMarkdownRenderer html={selected.renderedHtml} />
                {canRestore && selected.id !== rows[0]?.id ? (
                  <Button onClick={() => void handleRestore()} disabled={isRestoring} className="justify-self-start">
                    {isRestoring ? "Restoring..." : "Restore this revision"}
                  </Button>
                ) : null}
              </>
            ) : (
              <p className="text-sm text-muted-foreground">No revisions yet.</p>
            )}
          </Card>
        </div>
      </Container>
    </>
  );
}
