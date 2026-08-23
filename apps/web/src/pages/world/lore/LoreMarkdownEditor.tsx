import { useCallback, useRef, useState } from "react";
import { getLoreLinkTargets, uploadLoreImage } from "@/api/lore";
import {
  Popover,
  PopoverAnchor,
  PopoverContent,
} from "@/components/ui/popover";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { Textarea } from "@/components/ui/textarea";
import type { LoreLinkTargetRecord } from "@/types/lore";

export interface LoreMarkdownEditorProps {
  loreEntryId: string;
  worldId: string;
  value: string;
  onChange: (value: string) => void;
  disabled?: boolean;
}

/**
 * Spec 012 (T028, T034, T041): a plain `<textarea>`-based Markdown editor —
 * deliberately not a rich-text/WYSIWYG framework (research.md §6). Adds:
 *  - T034: a `[[`-trigger autocomplete popover (lore entries + actors,
 *    FR-007a) that inserts a resolved `[[Title]]` reference at the cursor.
 *  - T041: paste/drop image upload — intercepts image `DataTransfer`
 *    items, uploads via `uploadLoreImage`, inserts `![](url)` at the
 *    cursor on success.
 */
export function LoreMarkdownEditor({
  loreEntryId,
  worldId,
  value,
  onChange,
  disabled,
}: LoreMarkdownEditorProps) {
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);
  const [isUploading, setIsUploading] = useState(false);
  const [uploadError, setUploadError] = useState<string | null>(null);

  const [linkQuery, setLinkQuery] = useState<{ start: number; prefix: string } | null>(null);
  const [linkTargets, setLinkTargets] = useState<LoreLinkTargetRecord[]>([]);

  const insertAtCursor = useCallback(
    (text: string) => {
      const textarea = textareaRef.current;
      if (!textarea) {
        onChange(value + text);
        return;
      }
      const start = textarea.selectionStart ?? value.length;
      const end = textarea.selectionEnd ?? value.length;
      const next = value.slice(0, start) + text + value.slice(end);
      onChange(next);
      requestAnimationFrame(() => {
        textarea.focus();
        const caret = start + text.length;
        textarea.setSelectionRange(caret, caret);
      });
    },
    [onChange, value],
  );

  const replaceRange = useCallback(
    (start: number, end: number, text: string) => {
      const next = value.slice(0, start) + text + value.slice(end);
      onChange(next);
      requestAnimationFrame(() => {
        const textarea = textareaRef.current;
        if (!textarea) {
          return;
        }
        textarea.focus();
        const caret = start + text.length;
        textarea.setSelectionRange(caret, caret);
      });
    },
    [onChange, value],
  );

  const uploadFile = useCallback(
    async (file: File) => {
      setIsUploading(true);
      setUploadError(null);
      try {
        const asset = await uploadLoreImage(loreEntryId, file);
        insertAtCursor(`![${file.name}](${asset.url})`);
      } catch (err) {
        setUploadError(err instanceof Error ? err.message : "Failed to upload image");
      } finally {
        setIsUploading(false);
      }
    },
    [insertAtCursor, loreEntryId],
  );

  const handlePaste = (event: React.ClipboardEvent<HTMLTextAreaElement>) => {
    const items = Array.from(event.clipboardData?.items ?? []);
    const imageItem = items.find((item) => item.type.startsWith("image/"));
    if (!imageItem) {
      return;
    }
    const file = imageItem.getAsFile();
    if (!file) {
      return;
    }
    event.preventDefault();
    void uploadFile(file);
  };

  const handleDrop = (event: React.DragEvent<HTMLTextAreaElement>) => {
    const files = Array.from(event.dataTransfer?.files ?? []);
    const imageFile = files.find((file) => file.type.startsWith("image/"));
    if (!imageFile) {
      return;
    }
    event.preventDefault();
    void uploadFile(imageFile);
  };

  const handleChange = (event: React.ChangeEvent<HTMLTextAreaElement>) => {
    const nextValue = event.target.value;
    onChange(nextValue);

    const caret = event.target.selectionStart ?? nextValue.length;
    const uptoCaret = nextValue.slice(0, caret);
    const match = /\[\[([^[\]]*)$/.exec(uptoCaret);
    if (match) {
      const prefix = match[1];
      const start = caret - match[0].length;
      setLinkQuery({ start, prefix });
      void getLoreLinkTargets(worldId, prefix)
        .then(setLinkTargets)
        .catch(() => setLinkTargets([]));
    } else {
      setLinkQuery(null);
      setLinkTargets([]);
    }
  };

  const handleSelectTarget = (target: LoreLinkTargetRecord) => {
    if (!linkQuery) {
      return;
    }
    const caret = textareaRef.current?.selectionStart ?? linkQuery.start + linkQuery.prefix.length + 2;
    replaceRange(linkQuery.start, caret, `[[${target.title}]]`);
    setLinkQuery(null);
    setLinkTargets([]);
  };

  return (
    <div className="grid gap-2">
      <Popover
        open={Boolean(linkQuery && linkTargets.length > 0)}
        onOpenChange={(open) => {
          if (!open) {
            setLinkQuery(null);
            setLinkTargets([]);
          }
        }}
      >
        <PopoverAnchor asChild>
          <Textarea
            ref={textareaRef}
            value={value}
            onChange={handleChange}
            onPaste={handlePaste}
            onDrop={handleDrop}
            onDragOver={(event) => event.preventDefault()}
            disabled={disabled || isUploading}
            rows={16}
            className="font-mono text-sm"
            placeholder="Write this entry's lore using Markdown — tables, task lists, code blocks, `[[Other Entry]]` links, and pasted images are all supported."
            data-testid="lore-markdown-editor-textarea"
          />
        </PopoverAnchor>
        <PopoverContent
          align="start"
          className="w-64 p-1"
          onOpenAutoFocus={(event) => event.preventDefault()}
          data-testid="lore-link-autocomplete"
        >
          <div className="grid gap-0.5">
            {linkTargets.map((target) => (
              <button
                key={`${target.kind}-${target.id}`}
                type="button"
                className="flex items-center justify-between gap-2 rounded px-2 py-1.5 text-left text-sm hover:bg-muted"
                onClick={() => handleSelectTarget(target)}
                data-testid={`lore-link-target-${target.id}`}
              >
                <span>{target.title}</span>
                <span className="text-xs text-muted-foreground uppercase">
                  {target.kind === "LORE_ENTRY" ? "Lore" : "Actor"}
                </span>
              </button>
            ))}
          </div>
        </PopoverContent>
      </Popover>

      {isUploading ? (
        <StatusBadge variant="info">Uploading image…</StatusBadge>
      ) : uploadError ? (
        <StatusBadge variant="danger">{uploadError}</StatusBadge>
      ) : null}
    </div>
  );
}
