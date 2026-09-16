import { useMemo, useState } from "react";
import { Button } from "@/components/ui/button/Button";
import { Input } from "@/components/ui/input";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { AdminTable } from "./AdminTable";
import type { SystemManifest } from "@/types/admin";

/**
 * The system manifest, as key / value / action.
 *
 * Spec 040 FR-009: a key the environment has fixed is shown with the variable
 * that fixed it, in place of the field.
 *
 * It was previously dropped into the "Manifest record" list at the bottom
 * alongside the keys that were never editable, which made two different facts
 * — "this key is not yours to change" and "this key is yours, but a variable
 * is currently winning" — look identical. An operator who has set
 * `THUNDERFORGE_REALM_NAME` and then cannot find realm name in the editor has
 * no way to tell which of those happened.
 *
 * # Why one table rather than three lists
 *
 * Editable, fixed and read-only keys were drawn as three differently-shaped
 * blocks, so a key's shape depended on a fact the operator had to infer from
 * the shape. They are one table now, in that order, and the difference lives
 * in the Action column where it is stated: a box and a Save, or the variable
 * that took the box away, or nothing at all. The rule FR-009 protects is
 * unchanged — a fixed key renders its value and its variable and no control —
 * it is simply now legible beside the keys it is being compared with.
 */
interface ManifestEditorProps {
  manifest: SystemManifest;
  onSaveKey: (key: string, value: string) => Promise<SystemManifest>;
}

const COLUMNS = ["Key", "Value", "Action"] as const;
const COLUMN_WIDTHS = ["26%", "44%", "30%"] as const;

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
      <AdminTable
        label="Manifest keys"
        columns={COLUMNS}
        columnWidths={COLUMN_WIDTHS}
        data-testid="manifest-table"
      >
        <tbody data-testid="manifest-editable-keys">
          {editableEntries.map((entry) => (
            <tr
              key={entry.key}
              className="border-b border-border"
              data-testid={`manifest-row-${entry.key}`}
              data-editable="true"
            >
              <th scope="row" className="px-3 py-3 text-left align-top">
                <label
                  htmlFor={`manifest-${entry.key}`}
                  className="font-semibold"
                >
                  {entry.key.replaceAll("_", " ")}
                </label>
                <span className="block text-xs font-normal text-muted-foreground">
                  {entry.key}
                </span>
              </th>
              <td className="px-3 py-3 align-top">
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
              </td>
              <td className="px-3 py-3 align-top">
                <Button
                  type="button"
                  variant="secondary"
                  size="sm"
                  icon="quill"
                  onClick={() => void handleSave(entry.key)}
                  disabled={savingKey === entry.key}
                  data-testid={`manifest-save-${entry.key}`}
                >
                  {savingKey === entry.key ? "Saving..." : "Save"}
                </Button>
              </td>
            </tr>
          ))}

          {/* FR-009: the value, the variable, and no control. */}
          {fixedEntries.map((entry) => (
            <tr
              key={entry.key}
              className="border-b border-border"
              data-testid={`manifest-fixed-${entry.key}`}
              data-editable="false"
            >
              <th scope="row" className="px-3 py-3 text-left align-top">
                <span className="font-semibold">
                  {entry.key.replaceAll("_", " ")}
                </span>
                <span className="block text-xs font-normal text-muted-foreground">
                  {entry.key}
                </span>
              </th>
              <td className="px-3 py-3 align-top break-words">{entry.value}</td>
              <td className="px-3 py-3 align-top">
                <StatusBadge variant="info">
                  Fixed by {entry.fixedBy}. Unset that variable to edit it here.
                </StatusBadge>
              </td>
            </tr>
          ))}

          {readonlyEntries.map((entry) => (
            <tr
              key={entry.key}
              className="border-b border-border last:border-b-0"
              data-testid={`manifest-readonly-${entry.key}`}
              data-editable="false"
            >
              <th scope="row" className="px-3 py-3 text-left align-top">
                <span className="font-semibold">
                  {entry.key.replaceAll("_", " ")}
                </span>
                <span className="block text-xs font-normal text-muted-foreground">
                  {entry.key}
                </span>
              </th>
              <td className="px-3 py-3 align-top break-words">{entry.value}</td>
              <td className="px-3 py-3 align-top text-muted-foreground">
                Not editable here
              </td>
            </tr>
          ))}
        </tbody>
      </AdminTable>

      <dl className="grid gap-x-6 gap-y-1 rounded-lg border border-border p-4 text-sm sm:grid-cols-[auto_minmax(0,1fr)]">
        <dt className="font-semibold">Manifest file</dt>
        <dd className="break-all text-muted-foreground">{manifest.path}</dd>
        <dt className="font-semibold">Schema version</dt>
        <dd className="text-muted-foreground">{manifest.schemaVersion}</dd>
        <dt className="font-semibold">Last written</dt>
        <dd className="text-muted-foreground">
          {new Date(manifest.updatedAt).toLocaleString()}
        </dd>
      </dl>

      {status ? <StatusBadge variant="info">{status}</StatusBadge> : null}
    </div>
  );
}
