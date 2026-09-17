import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button/Button";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { listGameSystems, type GameSystemSummary } from "@/api/gameSystems";
import { createShelfCollection } from "@/api/shelfCollections";

/**
 * Start a collection on the shelf (spec 050 FR-007, FR-009).
 *
 * A collection is written here rather than read out of a document, so it has
 * a title and a system and nothing else to begin with. The system decides
 * which worlds it is offered to and which kinds of entry go in it, as a
 * book's does.
 */
export function NewCollection({ onCreated }: { onCreated: () => void }) {
  const [systems, setSystems] = useState<GameSystemSummary[]>([]);
  const [systemId, setSystemId] = useState("");
  const [title, setTitle] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    listGameSystems()
      .then((installed) => {
        if (!live) return;
        setSystems(installed.systems);
        setSystemId(
          (chosen) =>
            chosen || (installed.defaultId ?? installed.systems[0]?.id) || "",
        );
      })
      .catch(() => {
        if (live) setSystems([]);
      });
    return () => {
      live = false;
    };
  }, []);

  return (
    <form
      className="flex flex-wrap items-end gap-3"
      data-testid="new-collection"
      onSubmit={(event) => {
        event.preventDefault();
        setBusy(true);
        setError(null);
        createShelfCollection(title, systemId)
          .then(() => {
            setTitle("");
            onCreated();
          })
          .catch((cause: unknown) =>
            setError(
              cause instanceof Error
                ? cause.message
                : "The collection could not be started.",
            ),
          )
          .finally(() => setBusy(false));
      }}
    >
      <label className="grid gap-1 text-sm">
        <span className="font-medium">Collection title</span>
        <input
          className="h-9 rounded-md border border-input bg-background px-2 text-sm"
          data-testid="new-collection-title"
          value={title}
          onChange={(event) => setTitle(event.target.value)}
        />
      </label>
      <label className="grid gap-1 text-sm">
        <span className="font-medium">For</span>
        <select
          className="h-9 rounded-md border border-input bg-background px-2 text-sm"
          data-testid="new-collection-system"
          value={systemId}
          onChange={(event) => setSystemId(event.target.value)}
        >
          {systems.length === 0 && <option value="">Reading systems</option>}
          {systems.map((system) => (
            <option key={system.id} value={system.id}>
              {system.title}
            </option>
          ))}
        </select>
      </label>
      <Button
        type="submit"
        variant="secondary"
        disabled={busy || title.trim() === "" || systemId === ""}
        data-testid="new-collection-submit"
      >
        Start a collection
      </Button>
      {error && (
        <StatusBadge variant="danger" data-testid="new-collection-error">
          {error}
        </StatusBadge>
      )}
    </form>
  );
}
