import { markdown } from "@codemirror/lang-markdown";
import CodeMirror from "@uiw/react-codemirror";
import { useTheme } from "@/hooks/theme-context";

export interface SceneSummaryEditorProps {
  value: string;
  onChange: (value: string) => void;
  disabled?: boolean;
}

/**
 * Spec 022 (FR-005): the same CodeMirror Markdown editing experience
 * `LoreMarkdownEditor.tsx` uses (syntax highlighting, line numbers, a
 * fold gutter, `closeBrackets` off so fenced code blocks don't get
 * mangled) — but without that editor's lore-specific `[[link]]`
 * autocomplete and paste-image-upload extensions, which scene summaries
 * have no use for (a scene's map image is set via dd2vtt import, not
 * pasted into its summary).
 */
export function SceneSummaryEditor({
  value,
  onChange,
  disabled,
}: SceneSummaryEditorProps) {
  const { theme } = useTheme();

  return (
    <div className="grid gap-2" data-testid="scene-summary-editor">
      <CodeMirror
        value={value}
        onChange={onChange}
        extensions={[markdown()]}
        theme={theme}
        editable={!disabled}
        basicSetup={{
          lineNumbers: true,
          foldGutter: true,
          closeBrackets: false,
        }}
        placeholder="Write this scene's summary using Markdown — what it is, what to expect, anything the table should know."
      />
    </div>
  );
}
