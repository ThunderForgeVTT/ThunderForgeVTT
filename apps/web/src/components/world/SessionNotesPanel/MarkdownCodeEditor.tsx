import { markdown } from "@codemirror/lang-markdown";
import CodeMirror from "@uiw/react-codemirror";

export interface MarkdownCodeEditorProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
}

/** Isolated into its own module (and lazy-loaded by SessionNotesPanel) so
 * CodeMirror's bundle only downloads for a GM who actually opens the
 * editor — not for every visitor of a page as widely hit as Session
 * Setup, most of whom (players) never see this component at all. */
export default function MarkdownCodeEditor({ value, onChange, placeholder }: MarkdownCodeEditorProps) {
  return (
    <CodeMirror
      value={value}
      onChange={onChange}
      extensions={[markdown()]}
      height="200px"
      placeholder={placeholder}
      basicSetup={{ lineNumbers: false, foldGutter: false }}
    />
  );
}
