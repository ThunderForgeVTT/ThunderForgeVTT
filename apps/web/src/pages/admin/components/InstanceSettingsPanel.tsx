import { useCallback, useEffect, useMemo, useState } from "react";
import { Button } from "@/components/ui/button/Button";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import {
  fetchInstanceSettings,
  fetchSettingChanges,
  updateInstanceSetting,
  type ResolvedSetting,
  type SettingChange,
} from "@/api/instanceSettings";
import {
  groupOf,
  groupTitle,
  settingLabel,
  type RequiredSetting,
} from "@/services/instanceSetup";
import { SettingField } from "@/pages/setup/steps/SettingField";

/**
 * Spec 040 US2/US3: every declared setting, where its value came from, and who
 * changed it.
 *
 * # It renders through the wizard's field, on purpose
 *
 * `SettingField` already draws a declaration from its `kind`, and the setup
 * wizard already decides what a group is called. Drawing a second set of
 * fields here would mean a new declaration renders in one surface and not the
 * other — which is the shape of every defect this spec's session found: two
 * halves, each correct, disagreeing at the join. The adapter below is the
 * whole of the difference between the two surfaces' types.
 *
 * # What it refuses to restate
 *
 * Whether a field may be edited is `editable`, from the server, and nothing
 * here recomputes it from `source`. FR-009's rule is that a fixed field is not
 * offered *and says which variable fixed it* — a greyed-out box with no reason
 * is the thing the requirement exists to stop, so a fixed setting renders its
 * value and its variable and no control at all.
 *
 * # Secrets
 *
 * A secret's value never arrives. It is "Set" or "Not set", an empty box, and
 * a hint saying blank keeps what is stored — there is no masked form and no
 * length anywhere on this screen (FR-023). Clearing one is a separate,
 * deliberate button, because an accidental empty save must not erase a
 * credential.
 *
 * # GitHub applications are not here
 *
 * Those nine keys have their own panel, which resolves an application *whole*
 * — a client ID from one registration with a key from another authenticates
 * as a bad key (FR-021). Offering the same keys here one at a time would put
 * back exactly the field-at-a-time write that rule forbids, so this panel
 * points at that one instead of duplicating it.
 */

const GITHUB_GROUP = "GitHub applications";

/**
 * Groups, in the order an operator has reason to read them: who runs the
 * instance and who a copyright notice is served on first (T033 — a person
 * editing "who to serve a notice on" should not be doing it in a list of
 * thirty keys), then what the instance is, then the machinery.
 */
const GROUP_ORDER = [
  "Operator",
  "Copyright notices",
  "Legal prose",
  "Realm",
  "Support",
  "Access",
  "Mail",
];

function groupRank(group: string): number {
  const index = GROUP_ORDER.indexOf(group);
  return index === -1 ? GROUP_ORDER.length : index;
}

/** The one place the two surfaces' types are reconciled. */
export function asRequiredSetting(setting: ResolvedSetting): RequiredSetting {
  return {
    key: setting.key,
    kind: setting.kind,
    satisfied: setting.secretState
      ? setting.secretState === "SET"
      : Boolean(setting.value),
    source: setting.source,
    fixed_by: setting.fixedBy,
    requirement: setting.requirement,
    value: setting.value,
    group: setting.group,
    what_to_set: setting.whatToSet,
    capability: setting.capability,
    options: setting.enumOptions,
    secret: setting.secretState !== null,
  };
}

function sourceLabel(setting: ResolvedSetting): string {
  switch (setting.source) {
    case "ENVIRONMENT":
      return `Fixed by ${setting.fixedBy ?? "the environment"}`;
    case "INSTANCE":
      return "Set on this instance";
    default:
      return "Default";
  }
}

/**
 * One row of a setting's history (T032).
 *
 * Split out so it can be rendered on its own in a test: this surface has no
 * jsdom, so anything only reachable behind a click is only reachable behind a
 * mount. The redacted rendering is the one that matters — a secret's history
 * says a change happened and refuses to say what to, and it must not read as
 * "it was set to nothing".
 */
export function SettingChangeRow({
  settingKey,
  change,
}: {
  settingKey: string;
  change: SettingChange;
}) {
  return (
    <div
      className="grid gap-0.5 text-sm"
      data-testid={`instance-setting-change-${settingKey}`}
      data-redacted={change.redacted ? "true" : "false"}
    >
      <p className="text-muted-foreground">
        {new Date(change.changedAt).toLocaleString()} ·{" "}
        {change.changedBy ?? "the instance itself"} · {change.source}
      </p>
      {change.redacted ? (
        <p data-testid="redacted-change">
          A secret was changed. Its values are not recorded.
        </p>
      ) : (
        <p>
          <span className="text-muted-foreground">
            {change.previousValue ?? "unset"}
          </span>{" "}
          → {change.newValue ?? "unset"}
        </p>
      )}
    </div>
  );
}

interface SettingRowProps {
  setting: ResolvedSetting;
  onSaved: (updated: ResolvedSetting) => void;
}

export function SettingRow({ setting, onSaved }: SettingRowProps) {
  const isSecret = setting.secretState !== null;
  const [draft, setDraft] = useState(isSecret ? "" : (setting.value ?? ""));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [history, setHistory] = useState<SettingChange[] | null>(null);
  const [historyOpen, setHistoryOpen] = useState(false);

  const write = async (value: string | null) => {
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      const updated = await updateInstanceSetting(setting.key, value);
      onSaved(updated);
      setDraft(updated.secretState !== null ? "" : (updated.value ?? ""));
      setNotice(value === null ? "Cleared." : "Saved.");
      // The history on screen is now a version behind the value above it.
      if (historyOpen) {
        setHistory(await fetchSettingChanges(setting.key));
      }
    } catch (caught) {
      setError(
        caught instanceof Error ? caught.message : "The change was refused.",
      );
    } finally {
      setBusy(false);
    }
  };

  const toggleHistory = async () => {
    const next = !historyOpen;
    setHistoryOpen(next);
    if (next && history === null) {
      try {
        setHistory(await fetchSettingChanges(setting.key));
      } catch (caught) {
        setError(
          caught instanceof Error
            ? caught.message
            : "The change history could not be read.",
        );
      }
    }
  };

  return (
    <div
      className="grid gap-2 rounded-lg border border-border bg-secondary/40 p-4"
      data-testid={`instance-setting-${setting.key}`}
      data-source={setting.source}
      data-editable={setting.editable ? "true" : "false"}
    >
      {setting.editable ? (
        <SettingField
          setting={asRequiredSetting(setting)}
          value={draft}
          onChange={(_key, value) => setDraft(value)}
        />
      ) : (
        <div className="grid gap-1">
          <p className="text-xs font-semibold tracking-wider text-muted-foreground uppercase">
            {settingLabel(setting.key)}
          </p>
          <p data-testid={`instance-setting-value-${setting.key}`}>
            {isSecret
              ? setting.secretState === "SET"
                ? "Set"
                : "Not set"
              : (setting.value ?? "Not set")}
          </p>
          {/* FR-009: never a disabled box with no explanation. */}
          <StatusBadge variant="info">
            {setting.fixedBy
              ? `Fixed by ${setting.fixedBy}. Unset that variable to edit it here.`
              : "This value is written through another surface, which keeps its own record."}
          </StatusBadge>
        </div>
      )}

      <div className="flex flex-wrap items-center gap-2">
        <span
          className="text-sm text-muted-foreground"
          data-testid={`instance-setting-source-${setting.key}`}
        >
          {sourceLabel(setting)}
        </span>
        {isSecret ? (
          <StatusBadge
            variant={setting.secretState === "SET" ? "success" : "warning"}
          >
            {setting.secretState === "SET" ? "Set" : "Not set"}
          </StatusBadge>
        ) : null}
        {setting.undecryptable ? (
          <StatusBadge variant="danger">
            Stored, but unreadable — the instance secret changed since it was
            written. Set it again.
          </StatusBadge>
        ) : null}
      </div>

      {setting.editable ? (
        <div className="flex flex-wrap gap-2">
          <Button
            type="button"
            variant="secondary"
            icon="quill"
            disabled={busy}
            onClick={() => void write(draft)}
            data-testid={`instance-setting-save-${setting.key}`}
          >
            {busy ? "Saving..." : "Save"}
          </Button>
          <Button
            type="button"
            variant="ghost"
            disabled={busy}
            onClick={() => void write(null)}
            data-testid={`instance-setting-clear-${setting.key}`}
          >
            Clear
          </Button>
          <Button
            type="button"
            variant="ghost"
            onClick={() => void toggleHistory()}
            data-testid={`instance-setting-history-toggle-${setting.key}`}
          >
            {historyOpen ? "Hide history" : "History"}
          </Button>
        </div>
      ) : (
        <div>
          <Button
            type="button"
            variant="ghost"
            onClick={() => void toggleHistory()}
            data-testid={`instance-setting-history-toggle-${setting.key}`}
          >
            {historyOpen ? "Hide history" : "History"}
          </Button>
        </div>
      )}

      {error ? <StatusBadge variant="danger">{error}</StatusBadge> : null}
      {notice ? <StatusBadge variant="success">{notice}</StatusBadge> : null}

      {historyOpen ? (
        <div
          className="grid gap-2 rounded-lg border border-border/60 p-3"
          data-testid={`instance-setting-history-${setting.key}`}
        >
          {history === null ? (
            <p className="text-muted-foreground">Reading the history...</p>
          ) : history.length === 0 ? (
            <p className="text-muted-foreground">
              This setting has not been changed through the settings surface.
            </p>
          ) : (
            history.map((change, index) => (
              <SettingChangeRow
                key={`${change.changedAt}-${index}`}
                settingKey={setting.key}
                change={change}
              />
            ))
          )}
        </div>
      ) : null}
    </div>
  );
}

export function InstanceSettingsPanel() {
  const [settings, setSettings] = useState<ResolvedSetting[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    fetchInstanceSettings()
      .then((next) => {
        setSettings(next);
        setError(null);
      })
      .catch((cause: unknown) => {
        setError(
          cause instanceof Error
            ? cause.message
            : "The instance settings could not be read.",
        );
      });
  }, []);

  useEffect(load, [load]);

  const groups = useMemo(() => {
    if (!settings) return [];
    const byGroup = new Map<string, ResolvedSetting[]>();
    for (const setting of settings) {
      const group = groupOf(asRequiredSetting(setting));
      if (group === GITHUB_GROUP) continue;
      const bucket = byGroup.get(group);
      if (bucket) bucket.push(setting);
      else byGroup.set(group, [setting]);
    }
    return [...byGroup.entries()]
      .map(([group, items]) => ({ group, items }))
      .sort(
        (left, right) =>
          groupRank(left.group) - groupRank(right.group) ||
          left.group.localeCompare(right.group),
      );
  }, [settings]);

  const onSaved = useCallback((updated: ResolvedSetting) => {
    setSettings((current) =>
      current
        ? current.map((setting) =>
            setting.key === updated.key ? updated : setting,
          )
        : current,
    );
  }, []);

  if (error) {
    return <StatusBadge variant="danger">{error}</StatusBadge>;
  }

  if (!settings) {
    return <p className="text-muted-foreground">Reading this instance...</p>;
  }

  return (
    <div className="grid gap-6" data-testid="instance-settings-panel">
      {groups.map(({ group, items }) => (
        <section
          key={group}
          className="grid gap-3"
          data-testid={`settings-group-${group}`}
        >
          <h4 className="text-sm font-semibold tracking-wider text-muted-foreground uppercase">
            {groupTitle(group)}
          </h4>
          {items.map((setting) => (
            <SettingRow key={setting.key} setting={setting} onSaved={onSaved} />
          ))}
        </section>
      ))}
      <p className="text-muted-foreground">
        GitHub applications are configured above, as whole applications rather
        than as individual keys.
      </p>
    </div>
  );
}
