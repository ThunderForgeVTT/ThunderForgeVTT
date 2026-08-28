import DOMPurify from "dompurify";
import { marked } from "marked";
import { lazy, Suspense, useMemo, useState } from "react";
import { updateWorldSessionNotes } from "@/api/world";
import { Button } from "@/components/ui/button/Button";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";

// Lazy: CodeMirror is only needed by a GM actually editing notes, not by
// every visitor of Session Setup (see MarkdownCodeEditor.tsx's own docs).
const MarkdownCodeEditor = lazy(() => import("./MarkdownCodeEditor"));

export interface SessionNotesPanelProps {
  worldId: string;
  notes: string | null;
  /** Only a DM/GM may edit (FR-012); everyone else sees read-only text. */
  isGm: boolean;
  onSaved: (notes: string) => void;
}

/** Renders `source` (Markdown) to sanitized HTML — `marked` doesn't
 * sanitize its own output, and session notes are GM-authored content
 * every world member reads, so a compromised/malicious GM account could
 * otherwise stored-XSS every player who opens this panel. */
function renderNotesMarkdown(source: string): string {
  return DOMPurify.sanitize(marked.parse(source, { async: false }));
}

/**
 * Spec 011 (US3, FR-011/FR-012/FR-013): a single freeform per-world
 * "last session" recap on Session Setup. DM/GM-editable via a Markdown
 * code editor (CodeMirror + `@codemirror/lang-markdown`), rendered as
 * sanitized HTML for everyone (including the GM's own live preview) —
 * matching the "code editor markdown" editing experience over a plain
 * textarea. Read-only for non-GM members. Saving an empty value is a
 * valid, explicit save — not treated as "no change" (FR-013).
 */
export function SessionNotesPanel({
  worldId,
  notes,
  isGm,
  onSaved,
}: SessionNotesPanelProps) {
  const [draft, setDraft] = useState(notes ?? "");
  const [isSaving, setIsSaving] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [showPreview, setShowPreview] = useState(false);

  const handleSave = async () => {
    setIsSaving(true);
    setStatus(null);
    try {
      const updated = await updateWorldSessionNotes(worldId, draft);
      onSaved(updated.sessionNotes ?? "");
      setStatus("Saved.");
    } catch (err) {
      setStatus(err instanceof Error ? err.message : "Failed to save notes");
    } finally {
      setIsSaving(false);
    }
  };

  const renderedDraft = useMemo(() => renderNotesMarkdown(draft), [draft]);
  const renderedNotes = useMemo(
    () => renderNotesMarkdown(notes ?? ""),
    [notes],
  );

  if (!isGm) {
    return (
      <div data-testid="session-notes-readonly">
        {notes ? (
          <div
            className="prose prose-sm max-w-none text-sm dark:prose-invert"
            dangerouslySetInnerHTML={{ __html: renderedNotes }}
          />
        ) : (
          <p className="text-sm text-muted-foreground italic">No notes yet.</p>
        )}
      </div>
    );
  }

  return (
    <div className="grid gap-2">
      <div className="flex items-center justify-between">
        <span className="text-xs font-medium text-muted-foreground">
          Markdown
        </span>
        <Button
          type="button"
          size="sm"
          variant="ghost"
          onClick={() => setShowPreview((v) => !v)}
          data-testid="session-notes-preview-toggle"
        >
          {showPreview ? "Edit" : "Preview"}
        </Button>
      </div>

      {showPreview ? (
        <div
          className="prose prose-sm min-h-[10rem] max-w-none rounded-md border border-border bg-background p-3 text-sm dark:prose-invert"
          data-testid="session-notes-preview"
          dangerouslySetInnerHTML={{ __html: renderedDraft }}
        />
      ) : (
        <div
          className="overflow-hidden rounded-md border border-border"
          data-testid="session-notes-editor"
        >
          <Suspense
            fallback={<div className="h-[200px] animate-pulse bg-muted" />}
          >
            <MarkdownCodeEditor
              value={draft}
              onChange={setDraft}
              placeholder="What happened last session? Leave a recap for next time… (Markdown supported)"
            />
          </Suspense>
        </div>
      )}

      <div className="flex items-center gap-3">
        <Button
          type="button"
          size="sm"
          onClick={() => void handleSave()}
          disabled={isSaving}
          data-testid="session-notes-save-button"
        >
          {isSaving ? "Saving..." : "Save"}
        </Button>
        {status ? (
          <StatusBadge variant={status === "Saved." ? "success" : "danger"}>
            {status}
          </StatusBadge>
        ) : null}
      </div>
    </div>
  );
}
