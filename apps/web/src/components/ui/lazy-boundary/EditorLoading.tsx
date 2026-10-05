/**
 * What stands where a Markdown editor will be while its code arrives (spec
 * 068 FR-003): the text it will edit, read-only, at the editor's size.
 *
 * Read-only on purpose. Something typed here would be lost when the editor
 * took over, so there is nothing to type into.
 */
export function EditorLoading({ value }: { value: string }) {
  return (
    <div
      aria-busy="true"
      aria-label="Loading the editor"
      className="min-h-[200px] rounded-md border border-border bg-muted/40 p-3 font-mono text-sm whitespace-pre-wrap text-muted-foreground"
      data-testid="editor-loading"
    >
      {value}
    </div>
  );
}
