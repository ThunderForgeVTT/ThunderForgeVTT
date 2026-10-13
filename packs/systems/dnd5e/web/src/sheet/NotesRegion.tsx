import { useState } from "react";
import { Button } from "@thunderforge/host";
import { hintClass, textareaClass } from "../components/styles";

/**
 * Notes, written through `trait_data.notes` — the field this pack's `sheet`
 * block declares. A draft and a Save rather than a write per keystroke; keyed
 * by the stored text in the parent so a save from elsewhere replaces an
 * untouched draft.
 */
export default function NotesRegion({
  notes,
  canEdit,
  isSaving,
  onSave,
}: {
  notes: string;
  canEdit: boolean;
  isSaving: boolean;
  onSave: (notes: string) => Promise<void> | void;
}) {
  const [draft, setDraft] = useState(notes);

  if (!canEdit) {
    return notes ? (
      <p className="text-sm whitespace-pre-wrap" data-testid="dnd5e-notes">
        {notes}
      </p>
    ) : (
      <p className={hintClass} data-testid="dnd5e-notes">
        No notes.
      </p>
    );
  }

  return (
    <div className="grid gap-2">
      <label htmlFor="dnd5e-notes" className="sr-only">
        Notes
      </label>
      <textarea
        id="dnd5e-notes"
        rows={6}
        className={textareaClass}
        value={draft}
        onChange={(event) => setDraft(event.target.value)}
        data-testid="dnd5e-notes-input"
      />
      <Button
        size="sm"
        className="justify-self-start"
        disabled={isSaving || draft === notes}
        onClick={() => void onSave(draft)}
        data-testid="dnd5e-notes-save"
      >
        Update notes
      </Button>
    </div>
  );
}
