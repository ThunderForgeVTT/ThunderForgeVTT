import { useMemo, useState } from "react";
import { Button } from "@/components/ui/button/Button";
import { Field } from "@/components/ui/field/Field";
import { Input } from "@/components/ui/input";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import type { SystemManifest } from "@/types/admin";

/**
 * Spec 040 FR-009: a key the environment has fixed is shown with the variable
 * that fixed it, in place of the field.
 *
 * It was previously dropped into the "Manifest record" list at the bottom
 * alongside the keys that were never editable, which made two different facts
 * — "this key is not yours to change" and "this key is yours, but a variable
 * is currently winning" — look identical. An operator who has set
 * `THUNDERFORGE_REALM_NAME` and then cannot find realm name in the editor has
 * no way to tell which of those happened.
 */
interface ManifestEditorProps {
  manifest: SystemManifest;
  onSaveKey: (key: string, value: string) => Promise<SystemManifest>;
}

export function ManifestEditor({ manifest, onSaveKey }: ManifestEditorProps) {
  const editableEntries = useMemo(
    () => manifest.entries.filter((entry) => entry.editable),
    [manifest.entries],
  );
  const fixedEntries = useMemo(
    () => manifest.entries.filter((entry) => entry.fixedBy),
    [manifest.entries],
  );
  const readonlyEntries = useMemo(
    () => manifest.entries.filter((entry) => !entry.editable && !entry.fixedBy),
    [manifest.entries],
  );

  const [values, setValues] = useState<Record<string, string>>(
    Object.fromEntries(
      editableEntries.map((entry) => [entry.key, entry.value]),
    ),
  );
  const [status, setStatus] = useState<string | null>(null);
  const [savingKey, setSavingKey] = useState<string | null>(null);

  const handleSave = async (key: string) => {
    setSavingKey(key);
    setStatus(null);

    try {
      await onSaveKey(key, values[key] ?? "");
      setStatus(`Manifest key "${key}" updated.`);
    } catch (error) {
      setStatus(
        error instanceof Error ? error.message : "Failed to update manifest.",
      );
    } finally {
      setSavingKey(null);
    }
  };

  return (
    <div className="grid gap-4">
      <div className="grid gap-4">
        {editableEntries.map((entry) => (
          <div
            key={entry.key}
            className="grid gap-3 rounded-lg border border-border bg-secondary/40 p-4"
          >
            <Field
              label={entry.key.replaceAll("_", " ")}
              htmlFor={`manifest-${entry.key}`}
            >
              <Input
                id={`manifest-${entry.key}`}
                value={values[entry.key] ?? ""}
                onChange={(event) =>
                  setValues((current) => ({
                    ...current,
                    [entry.key]: event.target.value,
                  }))
                }
              />
            </Field>
            <Button
              type="button"
              variant="secondary"
              icon="quill"
              onClick={() => void handleSave(entry.key)}
              disabled={savingKey === entry.key}
            >
              {savingKey === entry.key ? "Saving..." : "Save"}
            </Button>
          </div>
        ))}
      </div>

      {fixedEntries.length ? (
        <div className="grid gap-3" data-testid="manifest-fixed-keys">
          {fixedEntries.map((entry) => (
            <div
              key={entry.key}
              className="grid gap-1 rounded-lg border border-border bg-secondary/40 p-4"
              data-testid={`manifest-fixed-${entry.key}`}
            >
              <p className="font-semibold">{entry.key.replaceAll("_", " ")}</p>
              <p className="text-muted-foreground">{entry.value}</p>
              <StatusBadge variant="info">
                Fixed by {entry.fixedBy}. Unset that variable to edit it here.
              </StatusBadge>
            </div>
          ))}
        </div>
      ) : null}

      <div className="grid gap-1.5 rounded-lg border border-primary/20 bg-primary/5 p-4">
        <h3 className="font-semibold">Manifest record</h3>
        <p className="text-muted-foreground">Path: {manifest.path}</p>
        <p className="text-muted-foreground">
          Schema version: {manifest.schemaVersion}
        </p>
        <p className="text-muted-foreground">
          Updated at: {new Date(manifest.updatedAt).toLocaleString()}
        </p>
        {readonlyEntries.map((entry) => (
          <p key={entry.key} className="text-muted-foreground">
            <strong className="text-foreground">{entry.key}:</strong>{" "}
            {entry.value}
          </p>
        ))}
      </div>

      {status ? <StatusBadge variant="info">{status}</StatusBadge> : null}
    </div>
  );
}
