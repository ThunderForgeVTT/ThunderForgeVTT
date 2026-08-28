import { useState } from "react";
import { Button } from "@/components/ui/button/Button";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { Switch } from "@/components/ui/switch";
import type {
  AdminBootstrapSettings,
  AuthSecuritySettings,
} from "@/types/admin";

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
      setStatus(
        error instanceof Error
          ? error.message
          : "Failed to update security policy.",
      );
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <div className="grid gap-6">
      <div className="grid gap-3">
        <div>
          <h3 className="font-semibold">Two-factor enforcement</h3>
          <p className="text-muted-foreground">
            Decide whether every user must complete two-factor authentication.
          </p>
        </div>
        <label className="flex items-center gap-2 text-sm">
          <Switch
            checked={requiredForAllUsers}
            onCheckedChange={(checked) => setRequiredForAllUsers(checked)}
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

      <div className="grid gap-3">
        <div>
          <h3 className="font-semibold">Bootstrap record</h3>
          <p className="text-muted-foreground">
            Inspect the persisted first-run state that initialized this
            instance.
          </p>
        </div>
        {bootstrapSettings ? (
          <div className="grid gap-1">
            <p className="text-muted-foreground">
              <strong className="text-foreground">Setup completed:</strong>{" "}
              {bootstrapSettings.setupCompleted ? "Yes" : "No"}
            </p>
            <p className="text-muted-foreground">
              <strong className="text-foreground">Admin code generated:</strong>{" "}
              {bootstrapSettings.adminCodeGeneratedAt
                ? new Date(
                    bootstrapSettings.adminCodeGeneratedAt,
                  ).toLocaleString()
                : "Not recorded"}
            </p>
            <p className="text-muted-foreground">
              <strong className="text-foreground">Setup completed at:</strong>{" "}
              {bootstrapSettings.setupCompletedAt
                ? new Date(bootstrapSettings.setupCompletedAt).toLocaleString()
                : "Pending"}
            </p>
          </div>
        ) : (
          <StatusBadge variant="warning">
            Bootstrap settings are unavailable.
          </StatusBadge>
        )}
      </div>

      {status ? <StatusBadge variant="info">{status}</StatusBadge> : null}
    </div>
  );
}
