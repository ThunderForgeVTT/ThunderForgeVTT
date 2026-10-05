import { lazy } from "react";
import { EditorLoading } from "@/components/ui/lazy-boundary/EditorLoading";
import { LazyBoundary } from "@/components/ui/lazy-boundary/LazyBoundary";
import type { LoreMarkdownEditorProps } from "./LoreMarkdownEditor";

// Spec 068 FR-002: CodeMirror is the largest thing this app ships after the
// engine, and a lore entry is read far more often than it is edited.
const Editor = lazy(() =>
  import("./LoreMarkdownEditor").then((module) => ({
    default: module.LoreMarkdownEditor,
  })),
);

/** `LoreMarkdownEditor`, loaded when it is on screen. */
export function LazyLoreMarkdownEditor(props: LoreMarkdownEditorProps) {
  return (
    <LazyBoundary
      what="The editor"
      fallback={<EditorLoading value={props.value} />}
    >
      <Editor {...props} />
    </LazyBoundary>
  );
}
