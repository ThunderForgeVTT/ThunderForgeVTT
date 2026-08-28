import { useEffect, useState } from "react";
import { createLoreEntry, getLoreEntry, updateLoreEntry } from "@/api/lore";
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

  useEffect(() => {
    let active = true;
    setEntry(undefined);

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
        <h2 className="text-lg font-semibold">Compendium overview</h2>
        <p className="text-sm text-muted-foreground">
          Shown at the top of the Compendium (NPCs/Lore/Items/Abilities). Write
          whatever sets the scene — Markdown supported.
        </p>
      </div>

      {entry === undefined ? (
        <Loader label="Loading" />
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
