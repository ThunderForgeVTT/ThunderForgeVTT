/**
 * Spec 048: an actor's import history, which the world store does not hold,
 * with `refetch()` (AGENTS.md: reads the store does not hold are plain
 * fetches in a hook). The preview and the apply are calls the import page
 * makes when the player asks; the actor itself comes back through event 40.
 */
import { useCallback, useEffect, useState } from "react";
import { useResetOnChange } from "@/hooks/useResetOnChange";
import {
  applySheetImport,
  getActorImports,
  getSheetImportPreview,
  type ActorImportRecord,
  type ApplySheetImportInput,
  type SheetCorrections,
  type SheetImportPlan,
} from "@/api/sheetImport";

export interface UseSheetImportResult {
  imports: ActorImportRecord[];
  loading: boolean;
  error: Error | null;
  refetch: () => Promise<void>;
  preview: (
    reading: unknown,
    corrections?: SheetCorrections,
  ) => Promise<SheetImportPlan>;
  apply: (
    input: Omit<ApplySheetImportInput, "actorId">,
  ) => Promise<ActorImportRecord>;
}

function asError(err: unknown): Error {
  return err instanceof Error ? err : new Error(String(err));
}

export function useSheetImport(actorId: string): UseSheetImportResult {
  const [imports, setImports] = useState<ActorImportRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  useResetOnChange(actorId, () => {
    setLoading(true);
    setError(null);
  });

  useEffect(() => {
    let active = true;
    getActorImports(actorId)
      .then((records) => {
        if (active) {
          setImports(records);
          setError(null);
        }
      })
      .catch((err) => {
        if (active) {
          setError(asError(err));
          setImports([]);
        }
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [actorId]);

  const refetch = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setImports(await getActorImports(actorId));
    } catch (err) {
      setError(asError(err));
      setImports([]);
    } finally {
      setLoading(false);
    }
  }, [actorId]);

  const preview = useCallback(
    (reading: unknown, corrections?: SheetCorrections) =>
      getSheetImportPreview(actorId, reading, corrections),
    [actorId],
  );

  const apply = useCallback(
    async (input: Omit<ApplySheetImportInput, "actorId">) => {
      const record = await applySheetImport({ ...input, actorId });
      await refetch();
      return record;
    },
    [actorId, refetch],
  );

  return { imports, loading, error, refetch, preview, apply };
}
