import { markdown } from "@codemirror/lang-markdown";
import CodeMirror from "@uiw/react-codemirror";
import { useTheme } from "@/hooks/useTheme";

export interface MarkdownCodeEditorProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
}

/** Isolated into its own module (and lazy-loaded by SessionNotesPanel) so
 * CodeMirror's bundle only downloads for a GM who actually opens the
 * editor — not for every visitor of a page as widely hit as Session
 * Setup, most of whom (players) never see this component at all.
 *
 * Spec 021: line numbers + a fold gutter are enabled (basicSetup already
 * adds `foldGutter()` itself when not explicitly disabled — no separate
 * import needed) so this actually reads as a code editor rather than a
 * plain styled text box. `@codemirror/lang-markdown` ships real
 * heading-based folding out of the box (research.md R1), so the gutter
 * is meaningful here, not decorative. `theme` follows the app's own
 * light/dark mode (useTheme) instead of `@uiw/react-codemirror`'s
 * hardcoded `"light"` default — otherwise the editor stays a bright
 * white box inside an otherwise-dark page. */
export default function MarkdownCodeEditor({
  value,
  onChange,
  placeholder,
}: MarkdownCodeEditorProps) {
  const { theme } = useTheme();
  return (
    <CodeMirror
      value={value}
      onChange={onChange}
      extensions={[markdown()]}
      height="200px"
      placeholder={placeholder}
      theme={theme}
      // closeBrackets is disabled: basicSetup enables it by default and
      // it treats backtick as a pairable character, corrupting a fenced
      // code block the instant you type its opening ``` (confirmed live
      // in LoreMarkdownEditor.tsx, spec 021) — markdown doesn't benefit
      // from bracket-pairing the way code languages do.
      basicSetup={{ lineNumbers: true, foldGutter: true, closeBrackets: false }}
    />
  );
}
