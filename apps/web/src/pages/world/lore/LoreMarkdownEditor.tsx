import { useMemo, useState } from "react";
import { type Completion, type CompletionContext, type CompletionResult, autocompletion } from "@codemirror/autocomplete";
import { markdown } from "@codemirror/lang-markdown";
import { EditorView } from "@codemirror/view";
import CodeMirror from "@uiw/react-codemirror";
import { getLoreLinkTargets, uploadLoreImage } from "@/api/lore";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { useTheme } from "@/hooks/useTheme";

export interface LoreMarkdownEditorProps {
  loreEntryId: string;
  worldId: string;
  value: string;
  onChange: (value: string) => void;
  disabled?: boolean;
}

/**
 * Spec 021: migrated off a plain `<textarea>` onto the same CodeMirror
 * editor Session Notes uses (MarkdownCodeEditor.tsx) — syntax
 * highlighting, line numbers, a real (heading-based) fold gutter. Two
 * features from the original spec 012 `<textarea>` implementation are
 * rebuilt against CodeMirror's own extension system instead of raw DOM
 * events, with identical user-facing behavior (research.md R2/R3):
 *  - `[[`-trigger autocomplete (lore entries + actors) via
 *    `@codemirror/autocomplete`'s `autocompletion()` — its own popover,
 *    not the old Radix `Popover`.
 *  - paste/drop image upload via `EditorView.domEventHandlers`, calling
 *    the same `uploadLoreImage` mutation and inserting the same
 *    `![name](url)` markdown at the cursor.
 */
export function LoreMarkdownEditor({
  loreEntryId,
  worldId,
  value,
  onChange,
  disabled,
}: LoreMarkdownEditorProps) {
  const { theme } = useTheme();
  const [isUploading, setIsUploading] = useState(false);
  const [uploadError, setUploadError] = useState<string | null>(null);

  const extensions = useMemo(() => {
    const uploadFile = (file: File, view: EditorView) => {
      setIsUploading(true);
      setUploadError(null);
      uploadLoreImage(loreEntryId, file)
        .then((asset) => {
          const text = `![${file.name}](${asset.url})`;
          const pos = view.state.selection.main.head;
          view.dispatch({
            changes: { from: pos, insert: text },
            selection: { anchor: pos + text.length },
          });
        })
        .catch((err) => {
          setUploadError(err instanceof Error ? err.message : "Failed to upload image");
        })
        .finally(() => {
          setIsUploading(false);
        });
    };

    const loreLinkCompletionSource = async (context: CompletionContext): Promise<CompletionResult | null> => {
      const match = context.matchBefore(/\[\[[^[\]]*/);
      if (!match) {
        return null;
      }
      const prefix = match.text.slice(2);
      let targets;
      try {
        targets = await getLoreLinkTargets(worldId, prefix);
      } catch {
        return null;
      }
      if (targets.length === 0) {
        return null;
      }
      // CodeMirror's own completion popup filters options by fuzzy-
      // matching each `label` against the document text in [from, pos) —
      // that range must NOT include the literal "[[" (no label contains
      // it), or every option silently fails the filter and the popup
      // never renders even though this source found real matches
      // (confirmed live: the network call succeeded and options were
      // built, but nothing showed until this was fixed). `from` here is
      // therefore the position right after "[[", not "[[" itself — the
      // wider replacement (removing "[[" too) happens via a function
      // `apply` below instead of the default string-apply, which only
      // replaces [from, to).
      const options: Completion[] = targets.map((target) => ({
        label: target.title,
        detail: target.kind === "LORE_ENTRY" ? "Lore" : "Actor",
        apply: (view: EditorView, _completion: Completion, _from: number, to: number) => {
          const text = `[[${target.title}]]`;
          view.dispatch({
            changes: { from: match.from, to, insert: text },
            selection: { anchor: match.from + text.length },
          });
        },
      }));
      return { from: match.from + 2, options };
    };

    return [
      markdown(),
      autocompletion({ override: [loreLinkCompletionSource] }),
      EditorView.domEventHandlers({
        paste(event, view) {
          const items = Array.from(event.clipboardData?.items ?? []);
          const imageItem = items.find((item) => item.type.startsWith("image/"));
          const file = imageItem?.getAsFile();
          if (!file) {
            return false;
          }
          event.preventDefault();
          uploadFile(file, view);
          return true;
        },
        drop(event, view) {
          const files = Array.from(event.dataTransfer?.files ?? []);
          const imageFile = files.find((file) => file.type.startsWith("image/"));
          if (!imageFile) {
            return false;
          }
          event.preventDefault();
          uploadFile(imageFile, view);
          return true;
        },
      }),
    ];
    // eslint-disable-next-line react-hooks/exhaustive-deps -- setIsUploading/setUploadError are stable useState setters
  }, [loreEntryId, worldId]);

  return (
    <div className="grid gap-2" data-testid="lore-markdown-editor-textarea">
      <CodeMirror
        value={value}
        onChange={onChange}
        extensions={extensions}
        theme={theme}
        editable={!disabled && !isUploading}
        // closeBrackets: `basicSetup` enables it by default, and it
        // treats backtick as a pairable character — auto-inserting a
        // matching closing backtick corrupts a fenced code block the
        // moment you type its opening ``` (confirmed live: it silently
        // mangled a ```js fence into an empty code block). Markdown
        // doesn't benefit from bracket-pairing the way code languages
        // do, so it's off entirely rather than only for backticks.
        // autocompletion: this editor supplies its own via `extensions`
        // above (the `[[Title]]` source) — basicSetup's default ambient
        // one would otherwise run alongside it for no reason.
        basicSetup={{ lineNumbers: true, foldGutter: true, closeBrackets: false, autocompletion: false }}
        placeholder="Write this entry's lore using Markdown — tables, task lists, code blocks, `[[Other Entry]]` links, and pasted images are all supported."
      />

      {isUploading ? (
        <StatusBadge variant="info">Uploading image…</StatusBadge>
      ) : uploadError ? (
        <StatusBadge variant="danger">{uploadError}</StatusBadge>
      ) : null}
    </div>
  );
}
