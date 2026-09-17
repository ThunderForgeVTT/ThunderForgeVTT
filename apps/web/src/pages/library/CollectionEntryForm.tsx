import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button/Button";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { getGameSystemManifest } from "@/api/gameSystems";
import { writeShelfCollectionEntry } from "@/api/shelfCollections";

/**
 * Write an entry into a collection (spec 050 FR-007).
 *
 * The kinds offered are the ones the collection's system declares, which are
 * the only ones the server takes. What an entry says is written as prose, as
 * a world's own additions are.
 */
export function CollectionEntryForm({
  collectionId,
  systemId,
  onWritten,
}: {
  collectionId: string;
  systemId: string;
  onWritten: () => void;
}) {
  const [kinds, setKinds] = useState<string[]>([]);
  const [kind, setKind] = useState("");
  const [name, setName] = useState("");
  const [text, setText] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    getGameSystemManifest(systemId)
      .then((manifest) => {
        if (!live) return;
        const declared = Array.isArray(manifest.contentPatterns)
          ? (manifest.contentPatterns as { kind: string }[]).map(
              (pattern) => pattern.kind,
            )
          : [];
        setKinds(declared);
        setKind((chosen) => chosen || declared[0] || "");
      })
      .catch(() => {
        if (live) setError("This collection's system could not be read.");
      });
    return () => {
      live = false;
    };
  }, [systemId]);

  return (
    <form
      className="grid gap-2 rounded-md border p-3"
      data-testid="collection-entry-form"
      onSubmit={(event) => {
        event.preventDefault();
        setBusy(true);
        setError(null);
        writeShelfCollectionEntry({
          collectionId,
          kind,
          name,
          proseText: text,
        })
          .then(() => {
            setName("");
            setText("");
            onWritten();
          })
          .catch((cause: unknown) =>
            setError(
              cause instanceof Error
                ? cause.message
                : "That entry could not be written.",
            ),
          )
          .finally(() => setBusy(false));
      }}
    >
      <p className="text-sm font-medium">Write an entry</p>
      <div className="flex flex-wrap gap-2">
        <label className="grid gap-1 text-sm">
          <span>Kind</span>
          <select
            className="h-9 rounded-md border border-input bg-background px-2 text-sm"
            data-testid="collection-entry-kind"
            value={kind}
            onChange={(event) => setKind(event.target.value)}
          >
            {kinds.map((declared) => (
              <option key={declared} value={declared}>
                {declared}
              </option>
            ))}
          </select>
        </label>
        <label className="grid flex-1 gap-1 text-sm">
          <span>Name</span>
          <input
            className="h-9 rounded-md border border-input bg-background px-2 text-sm"
            data-testid="collection-entry-name"
            value={name}
            onChange={(event) => setName(event.target.value)}
          />
        </label>
      </div>
      <label className="grid gap-1 text-sm">
        <span>What it says</span>
        <textarea
          className="min-h-20 rounded-md border border-input bg-background p-2 text-sm"
          data-testid="collection-entry-text"
          value={text}
          onChange={(event) => setText(event.target.value)}
        />
      </label>
      <div className="flex items-center gap-2">
        <Button
          type="submit"
          size="sm"
          disabled={
            busy || kind === "" || name.trim() === "" || text.trim() === ""
          }
          data-testid="collection-entry-submit"
        >
          Write it
        </Button>
        {error && (
          <StatusBadge variant="danger" data-testid="collection-entry-error">
            {error}
          </StatusBadge>
        )}
      </div>
    </form>
  );
}
