import { useCallback, useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { Button } from "@/components/ui/button/Button";
import { Field } from "@/components/ui/field/Field";
import { Input } from "@/components/ui/input";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { AdminTable } from "./AdminTable";
import { SettingRow } from "./InstanceSettingsPanel";
import {
  applyResults,
  baselineOf,
  canSave,
  clear,
  dirtyKeys,
  discard,
  draftOf,
  edit,
  emptyForm,
  isDirty,
  rebase,
  savedNotice,
  sendValue,
  type FormState,
  type SaveResult,
} from "@/pages/admin/settingsForm";
import { useUnsavedChanges } from "@/hooks/useUnsavedChanges";
import {
  fetchMailAvailability,
  fetchMailOutbox,
  retryOutboxMessage,
  sendTestMail,
  type MailAvailability,
  type OutboxEntry,
} from "@/api/mail";
import {
  fetchInstanceSettings,
  updateInstanceSetting,
  type ResolvedSetting,
} from "@/api/instanceSettings";

/**
 * Spec 040 US4: setting mail up, proving it works, and what this instance has
 * failed to send — in that order, because that is the order it is done in.
 *
 * # There is no message body on this screen, and there cannot be
 *
 * Not for an administrator, not in a diagnostic, not for the test message
 * (FR-016). The server's outbox type has no body field and asserts its own
 * absence against the generated SDL; this panel has nothing to render even if
 * one appeared. `subject` is shown where the server sends one, which is the
 * instance's own test message alone — a subject about a person is content.
 *
 * # The test message goes through the outbox
 *
 * It is not a shortcut past the queue. It is enqueued, attempted and recorded
 * exactly like every other message, so the path this button proves is the path
 * production uses — a test that exercises a shortcut proves the shortcut. That
 * is also why a failed test leaves a row behind: the failure is findable later
 * by whoever is actually debugging it.
 *
 * # One form, one Save (spec 088 US7)
 *
 * The rows used to save one at a time, so an admin who filled in five boxes
 * and pressed one Save lost four. Now the page is one form over
 * `settingsForm.ts`: Save sends one `updateInstanceSetting` per dirty key and
 * nothing for the rest, in form order with `mail.enabled` last, says how many
 * landed, and keeps a refused key dirty with its reason. While anything is
 * unsaved, leaving the page asks first.
 *
 * # Why the settings ARE edited here now, and why that is not a second writer
 *
 * This file used to say the opposite, and the reasoning was sound but the
 * conclusion was wrong: it argued that editing `mail.*` here would be "a
 * second write path to the same keys". It would only be a second write path
 * if it were a second *implementation*. It is not. The rows below are
 * `SettingRow` (in its form mode) — the same component the instance settings
 * panel renders, over
 * the same `updateInstanceSetting` mutation, with the same `editable`
 * refusals, the same secret handling and the same change record. There is one
 * writer; this page is a second place it is mounted, filtered to the eight
 * keys the tester beside it is about.
 *
 * The owner's words: "this email tester is a really great place to actually
 * add email setup concepts." An operator who cannot send mail was being shown
 * a list of key names to go and find somewhere else, on a screen that already
 * knew exactly which ones were missing.
 *
 * # Operator identity is linked, not copied
 *
 * `mail.from_address` is meaningless without a name to put beside it, and
 * `operator.name` is that name — but it is also the name on the legal pages,
 * and a person editing it should be doing that where its consequences are
 * visible. So it is a link.
 */

const STATE_VARIANT: Record<
  OutboxEntry["state"],
  "success" | "warning" | "danger" | "info"
> = {
  SENT: "success",
  QUEUED: "info",
  SENDING: "info",
  BLOCKED: "warning",
  FAILED: "danger",
};

/**
 * The order the settings are asked for, which is the order they are found in
 * an SMTP provider's own instructions. Alphabetical — what the settings panel
 * shows — puts "from address" second and "host" fourth, which is nobody's
 * mental model of setting up a mail server.
 */
const SERVER_KEYS = [
  "mail.host",
  "mail.port",
  "mail.security",
  "mail.username",
  "mail.password",
] as const;

const IDENTITY_KEYS = ["mail.from_address", "mail.from_name"] as const;

/** Form order: `dirtyKeys` puts `mail.enabled` last when it sends. */
const FORM_ORDER = ["mail.enabled", ...SERVER_KEYS, ...IDENTITY_KEYS];

/**
 * The client's own check. Only a port is checked here; an address is left to
 * the server, which already says what is wrong with one.
 */
function validatorFor(setting: ResolvedSetting) {
  if (setting.kind !== "PORT") {
    return undefined;
  }
  return (value: string) => {
    if (value === "") {
      return null;
    }
    const port = Number(value);
    return /^\d+$/.test(value) && port >= 1 && port <= 65535
      ? null
      : "A port is a whole number from 1 to 65535.";
  };
}

const OUTBOX_COLUMNS = [
  "To",
  "State",
  "Purpose",
  "Attempted",
  "Retry",
] as const;

const OUTBOX_WIDTHS = ["26%", "16%", "26%", "20%", "12%"] as const;

export function MailPanel() {
  const [availability, setAvailability] = useState<MailAvailability | null>(
    null,
  );
  const [outbox, setOutbox] = useState<OutboxEntry[]>([]);
  const [settings, setSettings] = useState<ResolvedSetting[]>([]);
  const [to, setTo] = useState("");
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<string | null>(null);
  const [resultOk, setResultOk] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [form, setForm] = useState<FormState | null>(null);
  const [saving, setSaving] = useState(false);
  const [notices, setNotices] = useState<Record<string, string>>({});
  const [saveResult, setSaveResult] = useState<string | null>(null);

  const load = useCallback(
    () =>
      Promise.all([
        fetchMailAvailability(),
        fetchMailOutbox(),
        fetchInstanceSettings(),
      ])
        .then(([nextAvailability, nextOutbox, nextSettings]) => {
          setAvailability(nextAvailability);
          setOutbox(nextOutbox);
          setSettings(nextSettings);
          // A reload keeps what the admin has not saved yet, and only that:
          // a change the fresh settings already hold is no longer a change.
          const fresh = baselineOf(nextSettings);
          setForm((current) =>
            current ? rebase(current, fresh) : emptyForm(fresh),
          );
          setError(null);
        })
        .catch((cause: unknown) => {
          setError(
            cause instanceof Error
              ? cause.message
              : "The mail configuration could not be read.",
          );
        }),
    [],
  );

  useEffect(() => {
    void load();
    // Availability and the outbox are read together because a test message
    // changes both: the row it leaves behind, and — for the first message a
    // freshly configured instance sends — whether mail is reported ready.
  }, [load]);

  const byKey = useMemo(
    () => new Map(settings.map((setting) => [setting.key, setting])),
    [settings],
  );

  const dirty = form ? dirtyKeys(form, FORM_ORDER) : [];
  useUnsavedChanges(dirty.length > 0);

  const touched = (key: string) => {
    setSaveResult(null);
    setNotices((current) => {
      if (!(key in current)) {
        return current;
      }
      const next = { ...current };
      delete next[key];
      return next;
    });
  };

  /**
   * A save changes what this instance can do, so the whole page is re-read
   * afterwards rather than the rows patched. Leaving "cannot send mail yet"
   * on screen above a host that was just filled in is the specific way this
   * page could lie.
   */
  const handleSave = async () => {
    if (!form) {
      return;
    }
    const keys = dirtyKeys(form, FORM_ORDER);
    setSaving(true);
    setSaveResult(null);
    const results: SaveResult[] = [];
    for (const key of keys) {
      try {
        await updateInstanceSetting(key, sendValue(form, key));
        results.push({ key, saved: true });
      } catch (caught) {
        results.push({
          key,
          saved: false,
          error:
            caught instanceof Error
              ? caught.message
              : "The change was refused.",
        });
      }
    }
    const saved: Record<string, string> = {};
    for (const result of results) {
      if (result.saved) {
        saved[result.key] =
          form.changes[result.key]?.kind === "clear" ? "Cleared." : "Saved.";
      }
    }
    setNotices(saved);
    setSaveResult(savedNotice(results));
    setForm((current) => (current ? applyResults(current, results) : current));
    await load();
    setSaving(false);
  };

  const handleDiscard = () => {
    setForm((current) => (current ? discard(current) : current));
    setNotices({});
    setSaveResult(null);
  };

  const renderSettings = (keys: readonly string[]) =>
    keys
      .map((key) => byKey.get(key))
      .filter((setting): setting is ResolvedSetting => Boolean(setting))
      .map((setting) => (
        <SettingRow
          key={setting.key}
          setting={setting}
          form={
            form
              ? {
                  draft: draftOf(form, setting.key),
                  dirty: isDirty(form, setting.key),
                  error:
                    form.errors[setting.key] ??
                    form.refused[setting.key] ??
                    null,
                  notice: notices[setting.key] ?? null,
                  busy: saving,
                  onChange: (value) => {
                    touched(setting.key);
                    setForm((current) =>
                      current
                        ? edit(
                            current,
                            setting.key,
                            value,
                            validatorFor(setting),
                          )
                        : current,
                    );
                  },
                  onClear: () => {
                    touched(setting.key);
                    setForm((current) =>
                      current ? clear(current, setting.key) : current,
                    );
                  },
                }
              : undefined
          }
        />
      ));

  const handleTest = async () => {
    setBusy(true);
    setResult(null);
    try {
      const outcome = await sendTestMail(to);
      setResultOk(outcome.delivered);
      setResult(
        outcome.delivered
          ? `Delivered to ${to}. If it does not arrive, the fault is past this instance.`
          : (outcome.reason ??
              "It was not delivered, and the server gave no reason."),
      );
      await load();
    } catch (caught) {
      setResultOk(false);
      setResult(
        caught instanceof Error
          ? caught.message
          : "The test message could not be sent.",
      );
    } finally {
      setBusy(false);
    }
  };

  const handleRetry = async (id: string) => {
    setBusy(true);
    try {
      const updated = await retryOutboxMessage(id);
      setOutbox((current) =>
        current.map((entry) => (entry.id === id ? updated : entry)),
      );
    } catch (caught) {
      setError(
        caught instanceof Error ? caught.message : "The retry was refused.",
      );
    } finally {
      setBusy(false);
    }
  };

  if (error) {
    return <StatusBadge variant="danger">{error}</StatusBadge>;
  }

  if (!availability) {
    return <p className="text-muted-foreground">Reading mail settings...</p>;
  }

  const enabled = byKey.get("mail.enabled");
  const mailIsOff = enabled ? enabled.value !== "true" : false;

  return (
    <div className="grid gap-6" data-testid="mail-panel">
      <div className="grid gap-2">
        <div
          data-testid="mail-availability"
          data-ready={availability.ready ? "true" : "false"}
        >
          <StatusBadge variant={availability.ready ? "success" : "warning"}>
            {availability.ready
              ? "This instance can send mail."
              : "This instance cannot send mail yet."}
          </StatusBadge>
        </div>
        <p className="max-w-[70ch] text-sm text-muted-foreground">
          Set the server details, save them, then send yourself a test message.
          Nothing is lost while this is unconfigured: messages wait in the
          outbox and are sent once mail works.
        </p>
      </div>

      {availability.missing.length ? (
        <div className="grid gap-1.5 rounded-lg border border-amber-500/40 bg-amber-500/5 p-4">
          <p className="font-semibold">Still to set</p>
          <ul className="flex flex-wrap gap-x-4 gap-y-1 text-sm text-muted-foreground">
            {availability.missing.map((key) => (
              <li key={key} data-testid={`mail-missing-${key}`}>
                <a
                  href={`#setting-${key}`}
                  className="underline-offset-4 hover:text-foreground hover:underline"
                >
                  {key}
                </a>
              </li>
            ))}
          </ul>
          {availability.limitedFeatures.length ? (
            <ul className="mt-1 grid gap-1 text-sm text-muted-foreground">
              {availability.limitedFeatures.map((limit) => (
                <li key={limit}>{limit}</li>
              ))}
            </ul>
          ) : null}
        </div>
      ) : null}

      <section className="grid gap-3" data-testid="mail-settings-switch">
        <div className="grid gap-1">
          <h4 className="text-lg font-semibold">Is mail switched on</h4>
          <p className="text-sm text-muted-foreground">
            {mailIsOff
              ? "Mail is off. Nothing is sent and nothing is discarded — messages queue until it is on and the server details are right."
              : "Mail is on. Messages are attempted as soon as they are queued."}
          </p>
        </div>
        {renderSettings(["mail.enabled"])}
      </section>

      <section className="grid gap-3" data-testid="mail-settings-server">
        <div className="grid gap-1">
          <h4 className="text-lg font-semibold">The server it sends through</h4>
          <p className="text-sm text-muted-foreground">
            Your SMTP provider's details. Each row says where its current value
            came from.
          </p>
        </div>
        {renderSettings(SERVER_KEYS)}
      </section>

      <section className="grid gap-3" data-testid="mail-settings-identity">
        <div className="grid gap-1">
          <h4 className="text-lg font-semibold">Who it comes from</h4>
          <p className="text-sm text-muted-foreground">
            The address and name on every message this instance sends. Who runs
            the instance is set in{" "}
            <Link
              className="underline underline-offset-4 hover:text-foreground"
              to="/admin/instance?group=operator"
            >
              Operator
            </Link>
            , where it also reaches the legal pages.
          </p>
        </div>
        {renderSettings(IDENTITY_KEYS)}
      </section>

      <section
        className="sticky bottom-0 z-10 flex flex-wrap items-center gap-3 rounded-lg border border-border bg-card p-4 shadow-sm"
        data-testid="mail-form-bar"
        data-dirty={dirty.length > 0 ? "true" : "false"}
      >
        <p
          className="text-sm font-medium"
          data-testid="mail-form-unsaved"
          aria-live="polite"
        >
          {dirty.length === 0
            ? "No unsaved changes"
            : `${dirty.length} unsaved ${dirty.length === 1 ? "change" : "changes"}`}
        </p>
        <div className="ml-auto flex flex-wrap gap-2">
          <Button
            type="button"
            variant="ghost"
            disabled={saving || dirty.length === 0}
            onClick={handleDiscard}
            data-testid="mail-form-discard"
          >
            Discard
          </Button>
          <Button
            type="button"
            icon="quill"
            disabled={saving || !form || !canSave(form)}
            onClick={() => void handleSave()}
            data-testid="mail-form-save"
          >
            {saving ? "Saving..." : "Save"}
          </Button>
        </div>
        {saveResult ? (
          <p
            className="basis-full text-sm text-muted-foreground"
            data-testid="mail-form-result"
          >
            {saveResult}
          </p>
        ) : null}
      </section>

      <section className="grid gap-2 rounded-lg border border-border bg-secondary/40 p-4">
        <h4 className="text-lg font-semibold">Prove it works</h4>
        <Field
          label="Send a test message to"
          htmlFor="mail-test-to"
          hint="It goes through the outbox like everything else, so whatever happens is recorded below."
        >
          <Input
            id="mail-test-to"
            data-testid="mail-test-to"
            type="email"
            autoComplete="off"
            value={to}
            onChange={(event) => setTo(event.target.value)}
          />
        </Field>
        <div>
          <Button
            type="button"
            variant="secondary"
            icon="quill"
            disabled={busy || !to.trim()}
            onClick={() => void handleTest()}
            data-testid="mail-test-send"
          >
            {busy ? "Sending..." : "Send test message"}
          </Button>
        </div>
        {result ? (
          <div
            data-testid="mail-test-result"
            data-delivered={resultOk ? "true" : "false"}
          >
            <StatusBadge variant={resultOk ? "success" : "danger"}>
              {result}
            </StatusBadge>
          </div>
        ) : null}
      </section>

      <section className="grid gap-2">
        <h4 className="text-lg font-semibold">Outbox</h4>
        {outbox.length === 0 ? (
          <p className="text-muted-foreground">
            This instance has not tried to send anything.
          </p>
        ) : (
          <AdminTable
            label="Outbox"
            columns={OUTBOX_COLUMNS}
            columnWidths={OUTBOX_WIDTHS}
            data-testid="mail-outbox-table"
          >
            <tbody>
              {outbox.map((entry) => (
                <tr
                  key={entry.id}
                  className="border-b border-border align-top last:border-b-0"
                  data-testid={`outbox-entry-${entry.id}`}
                  data-state={entry.state}
                >
                  <th scope="row" className="px-3 py-3 text-left break-all">
                    {entry.toAddress}
                  </th>
                  <td className="px-3 py-3">
                    <StatusBadge variant={STATE_VARIANT[entry.state]}>
                      {entry.state}
                    </StatusBadge>
                    {entry.lastFailureReason ? (
                      <span className="mt-1 block text-xs text-destructive">
                        {entry.lastFailureReason}
                      </span>
                    ) : null}
                  </td>
                  <td className="px-3 py-3 text-muted-foreground">
                    {entry.purpose}
                    {entry.subject ? ` · ${entry.subject}` : ""}
                  </td>
                  <td className="px-3 py-3 text-muted-foreground tabular-nums">
                    {new Date(entry.createdAt).toLocaleString()}
                    <span className="block text-xs">
                      {entry.attempts}{" "}
                      {entry.attempts === 1 ? "attempt" : "attempts"}
                    </span>
                  </td>
                  <td className="px-3 py-3">
                    {entry.state === "FAILED" || entry.state === "BLOCKED" ? (
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        disabled={busy}
                        onClick={() => void handleRetry(entry.id)}
                        data-testid={`outbox-retry-${entry.id}`}
                      >
                        Try again
                      </Button>
                    ) : null}
                  </td>
                </tr>
              ))}
            </tbody>
          </AdminTable>
        )}
      </section>
    </div>
  );
}
