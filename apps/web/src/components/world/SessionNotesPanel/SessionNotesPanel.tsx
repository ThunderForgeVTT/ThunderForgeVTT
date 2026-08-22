import { useState } from "react";
import { updateWorldSessionNotes } from "@/api/world";
import { Button } from "@/components/ui/button/Button";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { Textarea } from "@/components/ui/textarea";

export interface SessionNotesPanelProps {
  worldId: string;
  notes: string | null;
  /** Only a DM/GM may edit (FR-012); everyone else sees read-only text. */
  isGm: boolean;
  onSaved: (notes: string) => void;
}

/**
 * Spec 011 (US3, FR-011/FR-012/FR-013): a single freeform per-world
 * "last session" recap on Session Setup. DM/GM-editable, read-only for
 * everyone else. Saving an empty value is a valid, explicit save — not
 * treated as "no change" (FR-013).
 */
export function SessionNotesPanel({ worldId, notes, isGm, onSaved }: SessionNotesPanelProps) {
  const [draft, setDraft] = useState(notes ?? "");
  const [isSaving, setIsSaving] = useState(false);
  const [status, setStatus] = useState<string | null>(null);

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

  if (!isGm) {
    return (
      <div data-testid="session-notes-readonly">
        {notes ? (
          <p className="text-sm whitespace-pre-wrap">{notes}</p>
        ) : (
          <p className="text-sm text-muted-foreground italic">No notes yet.</p>
        )}
      </div>
    );
  }

  return (
    <div className="grid gap-2">
      <Textarea
        value={draft}
        onChange={(event) => setDraft(event.target.value)}
        placeholder="What happened last session? Leave a recap for next time…"
        rows={5}
        data-testid="session-notes-textarea"
      />
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
          <StatusBadge variant={status === "Saved." ? "success" : "danger"}>{status}</StatusBadge>
        ) : null}
      </div>
    </div>
  );
}
