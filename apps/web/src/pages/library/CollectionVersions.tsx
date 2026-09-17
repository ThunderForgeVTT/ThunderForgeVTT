import { useCallback, useEffect, useState } from "react";
import { Button } from "@/components/ui/button/Button";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import {
  restoreShelfCollectionVersion,
  shelfCollectionVersions,
  type ShelfCollectionVersion,
} from "@/api/shelfCollections";

/**
 * A collection's earlier versions, and the way back to one (spec 050 FR-104,
 * US6 scenario 6).
 *
 * Every change to a collection keeps the version it replaced: an entry
 * written or taken out, a world's changes synced back, a version put back.
 * Going back is itself a change, so the version it replaces is kept too and
 * nothing here is final.
 *
 * Only a collection has these. A book read in is re-read, and this component
 * is never rendered for one.
 */
export function CollectionVersions({
  collectionId,
  baseVersion,
  onRestored,
}: {
  collectionId: string;
  baseVersion: number;
  onRestored: () => void;
}) {
  const [versions, setVersions] = useState<ShelfCollectionVersion[] | null>(
    null,
  );
  const [confirming, setConfirming] = useState<ShelfCollectionVersion | null>(
    null,
  );
  const [error, setError] = useState<string | null>(null);

  const read = useCallback(() => {
    shelfCollectionVersions(collectionId)
      .then((history) => {
        setVersions(history);
        setError(null);
      })
      .catch((cause: unknown) =>
        setError(
          cause instanceof Error
            ? cause.message
            : "This collection's versions could not be read.",
        ),
      );
  }, [collectionId]);

  useEffect(read, [read, baseVersion]);

  const goBack = useCallback(() => {
    if (!confirming) return;
    restoreShelfCollectionVersion(collectionId, confirming.version)
      .then(() => {
        setConfirming(null);
        onRestored();
      })
      .catch((cause: unknown) =>
        setError(
          cause instanceof Error
            ? cause.message
            : "That version could not be put back.",
        ),
      );
  }, [collectionId, confirming, onRestored]);

  return (
    <section className="grid gap-2" data-testid="collection-versions">
      <h3 className="text-sm font-semibold">
        Earlier versions — this is version {baseVersion}
      </h3>
      {error && <StatusBadge variant="danger">{error}</StatusBadge>}
      {versions !== null && versions.length === 0 && (
        <p className="text-sm text-muted-foreground">
          Nothing has changed since it was started.
        </p>
      )}
      {versions !== null && versions.length > 0 && (
        <ul className="grid gap-1">
          {versions.map((past) => (
            <li
              key={past.version}
              className="flex flex-wrap items-center justify-between gap-2 text-sm"
              data-testid={`collection-version-${past.version}`}
            >
              <span>
                Version {past.version} — {past.entryTotal}{" "}
                {past.entryTotal === 1 ? "entry" : "entries"}.{" "}
                <span className="text-muted-foreground">
                  Replaced by: {past.replacedBy},{" "}
                  {new Date(past.replacedAt).toLocaleString()}
                </span>
              </span>
              <Button
                type="button"
                size="sm"
                variant="secondary"
                data-testid="restore-version"
                onClick={() => setConfirming(past)}
              >
                Go back to this version
              </Button>
            </li>
          ))}
        </ul>
      )}
      {confirming && (
        <div
          className="grid gap-2 rounded-md border border-amber-500 p-3"
          data-testid="restore-version-report"
        >
          <p className="text-sm">
            The collection will hold what version {confirming.version} held —{" "}
            {confirming.entryTotal}{" "}
            {confirming.entryTotal === 1 ? "entry" : "entries"} — as version{" "}
            {baseVersion + 1}. What it holds now is kept as version{" "}
            {baseVersion}, so this can be undone the same way. Every world
            running it reads the change; a world&apos;s own changes that no
            longer fit are kept and named there.
          </p>
          <div className="flex gap-2">
            <Button
              type="button"
              size="sm"
              data-testid="restore-version-confirm"
              onClick={goBack}
            >
              Go back to version {confirming.version}
            </Button>
            <Button
              type="button"
              size="sm"
              variant="secondary"
              onClick={() => setConfirming(null)}
            >
              Keep version {baseVersion}
            </Button>
          </div>
        </div>
      )}
    </section>
  );
}
