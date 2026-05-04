import { useMemo, useState } from "react";
import { Button } from "@/components/ui/button/Button";
import { Field } from "@/components/ui/field/Field";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import type { SystemManifest } from "@/types/admin";
import styles from "./ManifestEditor.module.scss";

interface ManifestEditorProps {
  manifest: SystemManifest;
  onSaveKey: (key: string, value: string) => Promise<SystemManifest>;
}

export function ManifestEditor({ manifest, onSaveKey }: ManifestEditorProps) {
  const editableEntries = useMemo(
    () => manifest.entries.filter((entry) => entry.editable),
    [manifest.entries],
  );
  const readonlyEntries = useMemo(
    () => manifest.entries.filter((entry) => !entry.editable),
    [manifest.entries],
  );

  const [values, setValues] = useState<Record<string, string>>(
    Object.fromEntries(editableEntries.map((entry) => [entry.key, entry.value])),
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
      setStatus(error instanceof Error ? error.message : "Failed to update manifest.");
    } finally {
      setSavingKey(null);
    }
  };

  return (
    <div className={styles.editor}>
      <div className={styles.group}>
        {editableEntries.map((entry) => (
          <div key={entry.key} className={styles.row}>
            <Field
              label={entry.key.replaceAll("_", " ")}
              htmlFor={`manifest-${entry.key}`}
            >
              <input
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

      <div className={styles.readonly}>
        <h3>Manifest record</h3>
        <p>Path: {manifest.path}</p>
        <p>Schema version: {manifest.schemaVersion}</p>
        <p>Updated at: {new Date(manifest.updatedAt).toLocaleString()}</p>
        {readonlyEntries.map((entry) => (
          <p key={entry.key}>
            <strong>{entry.key}:</strong> {entry.value}
          </p>
        ))}
      </div>

      {status ? <StatusBadge variant="info">{status}</StatusBadge> : null}
    </div>
  );
}
