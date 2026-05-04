import { useState } from "react";
import { Button } from "@/components/ui/button/Button";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import type {
  AdminBootstrapSettings,
  AuthSecuritySettings,
} from "@/types/admin";
import styles from "./SecurityPanel.module.scss";

interface SecurityPanelProps {
  settings: AuthSecuritySettings;
  bootstrapSettings: AdminBootstrapSettings | null;
  onUpdate: (requiredForAllUsers: boolean) => Promise<AuthSecuritySettings>;
}

export function SecurityPanel({
  settings,
  bootstrapSettings,
  onUpdate,
}: SecurityPanelProps) {
  const [requiredForAllUsers, setRequiredForAllUsers] = useState(
    settings.twoFactorRequiredForAllUsers,
  );
  const [isSaving, setIsSaving] = useState(false);
  const [status, setStatus] = useState<string | null>(null);

  const handleSave = async () => {
    setIsSaving(true);
    setStatus(null);

    try {
      await onUpdate(requiredForAllUsers);
      setStatus("2FA enforcement policy updated.");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Failed to update security policy.");
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <div className={styles.panel}>
      <div className={styles.section}>
        <div className={styles.header}>
          <h3>Two-factor enforcement</h3>
          <p>Decide whether every steward must complete the second seal.</p>
        </div>
        <label className={styles.toggle}>
          <input
            type="checkbox"
            checked={requiredForAllUsers}
            onChange={(event) => setRequiredForAllUsers(event.target.checked)}
          />
          <span>
            {requiredForAllUsers
              ? "Require 2FA for every authenticated user"
              : "Only require 2FA where user-level or admin policy already applies"}
          </span>
        </label>
        <Button
          type="button"
          variant="secondary"
          icon="shield"
          onClick={() => void handleSave()}
          disabled={isSaving}
        >
          {isSaving ? "Applying..." : "Update security policy"}
        </Button>
      </div>

      <div className={styles.section}>
        <div className={styles.header}>
          <h3>Bootstrap record</h3>
          <p>Inspect the persisted first-run state that founded this realm.</p>
        </div>
        {bootstrapSettings ? (
          <div className={styles.bootstrapMeta}>
            <p>
              <strong>Setup completed:</strong>{" "}
              {bootstrapSettings.setupCompleted ? "Yes" : "No"}
            </p>
            <p>
              <strong>Admin code generated:</strong>{" "}
              {bootstrapSettings.adminCodeGeneratedAt
                ? new Date(bootstrapSettings.adminCodeGeneratedAt).toLocaleString()
                : "Not recorded"}
            </p>
            <p>
              <strong>Setup completed at:</strong>{" "}
              {bootstrapSettings.setupCompletedAt
                ? new Date(bootstrapSettings.setupCompletedAt).toLocaleString()
                : "Pending"}
            </p>
          </div>
        ) : (
          <StatusBadge variant="warning">Bootstrap settings are unavailable.</StatusBadge>
        )}
      </div>

      {status ? <StatusBadge variant="info">{status}</StatusBadge> : null}
    </div>
  );
}
