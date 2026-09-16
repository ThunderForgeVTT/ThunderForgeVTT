import { useEffect, useState } from "react";
import { useResetOnChange } from "@/hooks/useResetOnChange";
import { createLoreEntry, getLoreEntry, updateLoreEntry } from "@/api/lore";
import { getWorldPlayState } from "@/api/playPause";
import {
  COMPENDIUM_OVERVIEW_DEFAULT_CONTENT,
  COMPENDIUM_OVERVIEW_SLUG,
  COMPENDIUM_OVERVIEW_TITLE,
} from "@/api/compendiumOverview";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Loader } from "@/components/ui/loader/Loader";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { LoreMarkdownEditor } from "@/pages/world/lore/LoreMarkdownEditor";
import type { LoreEntryRecord } from "@/types/lore";

export interface CompendiumOverviewSettingsCardProps {
  worldId: string;
}

/**
 * Spec 021: the Compendium's header description used to be a hardcoded
 * sentence. It's now the content of a reserved lore entry
 * (COMPENDIUM_OVERVIEW_SLUG) that the GM edits here, in the same
 * CodeMirror Markdown editor lore entries use — so the compendium's
 * "what is this place" blurb can actually say whatever the GM wants,
 * with real Markdown instead of one fixed line.
 */
export function CompendiumOverviewSettingsCard({
  worldId,
}: CompendiumOverviewSettingsCardProps) {
  const [entry, setEntry] = useState<LoreEntryRecord | null | undefined>(
    undefined,
  );
  const [content, setContent] = useState("");
  const [isSaving, setIsSaving] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [paused, setPaused] = useState(false);

  // Reset during render rather than at the top of the effect below: this
  // is state derived from the arguments, and doing it in the effect commits
  // one render pairing the new key with the previous key's data.
  useResetOnChange(worldId, () => {
    setEntry(undefined);
    setPaused(false);
  });

  useEffect(() => {
    let active = true;

    getLoreEntry(worldId, COMPENDIUM_OVERVIEW_SLUG)
      .then((existing) => {
        if (!active) {
          return;
        }
        if (existing) {
          setEntry(existing);
          setContent(existing.content);
          return;
        }
        // Lazily create the reserved entry on first visit — the Markdown
        // editor needs a real loreEntryId (pasted-image uploads, `[[`
        // link autocomplete both hang off it).
        //
        // Not while an operator has paused play (spec 051): the write is
        // refused, and a refused write sends the page to the pause notice,
        // so merely opening settings would have taken a member out of a
        // world whose pages FR-024 keeps open. A failed read of the play
        // state is not news about a pause, so it creates as before.
        return getWorldPlayState(worldId)
          .then((state) => state.paused)
          .catch(() => false)
          .then((isPaused) => {
            if (!active) return;
            if (isPaused) {
              setPaused(true);
              setEntry(null);
              return;
            }
            return createLoreEntry({
              worldId,
              title: COMPENDIUM_OVERVIEW_TITLE,
              content: COMPENDIUM_OVERVIEW_DEFAULT_CONTENT,
            }).then((created) => {
              if (active) {
                setEntry(created);
                setContent(created.content);
              }
            });
          });
      })
      .catch((err) => {
        if (active) {
          setStatus(
            err instanceof Error
              ? err.message
              : "Failed to load compendium overview",
          );
          setEntry(null);
        }
      });

    return () => {
      active = false;
    };
  }, [worldId]);

  const handleSave = async () => {
    if (!entry) {
      return;
    }
    setIsSaving(true);
    setStatus(null);
    try {
      const updated = await updateLoreEntry({
        loreEntryId: entry.id,
        content,
        expectedCurrentRevisionId: entry.currentRevisionId,
      });
      setEntry(updated);
      setContent(updated.content);
      setStatus("Saved.");
    } catch (err) {
      setStatus(
        err instanceof Error
          ? err.message
          : "Failed to save compendium overview",
      );
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <Card className="grid gap-3 p-6" data-testid="compendium-overview-card">
      <div>
        <h3 className="text-lg font-semibold">Compendium overview</h3>
        <p className="text-sm text-muted-foreground">
          Shown at the top of the Compendium (NPCs/Lore/Items/Abilities). Write
          whatever sets the scene — Markdown supported.
        </p>
      </div>

      {entry === undefined ? (
        <Loader label="Loading" />
      ) : entry === null && paused ? (
        <p
          className="text-sm text-muted-foreground"
          data-testid="compendium-overview-paused"
        >
          Play in this world is paused, so the overview cannot be written yet.
          It can be once play resumes.
        </p>
      ) : entry === null ? (
        <StatusBadge variant="danger">
          {status ?? "Failed to load compendium overview"}
        </StatusBadge>
      ) : (
        <>
          <LoreMarkdownEditor
            loreEntryId={entry.id}
            worldId={worldId}
            value={content}
            onChange={setContent}
            disabled={isSaving}
            label="Compendium overview"
          />
          <div className="flex items-center gap-3">
            <Button onClick={() => void handleSave()} disabled={isSaving}>
              {isSaving ? "Saving..." : "Save"}
            </Button>
            {status ? (
              <StatusBadge variant={status === "Saved." ? "success" : "danger"}>
                {status}
              </StatusBadge>
            ) : null}
          </div>
        </>
      )}
    </Card>
  );
}
