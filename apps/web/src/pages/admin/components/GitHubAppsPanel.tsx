import { useCallback, useEffect, useState } from "react";
import { postGraphQL } from "@/api/graphqlClient";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";

/**
 * Spec 040 US5 / ADR-090: this instance's GitHub applications, at two scales.
 *
 * One application for everything, or one per subsystem, or a mixture — and the
 * operator understands what they have chosen **while they are choosing it**.
 * Three things are on the screen for that reason and not for completeness:
 *
 * 1. **Which subsystems this application will act for** (FR-020), rendered
 *    wherever global credentials are set or offered, computed by the server
 *    from the resolution delivery actually gets rather than from a rule
 *    restated here in TypeScript. The list grows as subsystems are added and
 *    the sentence says so.
 * 2. **Where each value came from** (FR-021) — the scope, the source, and the
 *    variable that fixed it. A field greyed out with no stated reason is the
 *    thing this feature exists to stop, so a field the environment fixes says
 *    which variable fixed it and is not offered for editing.
 * 3. **A subsystem that was stepped over**, when it is half-written. An
 *    application is never completed from another; the operator is told their
 *    subsystem application was skipped and what is missing from it, because
 *    otherwise the symptom is a 401 that reads like a bad key.
 *
 * The private key is never rendered. There is no masked form, no truncation
 * and no length — `hasPrivateKey` is a boolean and that is the entire
 * vocabulary this screen has for it (FR-023, SC-007). The client ID is shown
 * literally, because GitHub publishes it and hiding it would make the screen
 * useless for the one thing operators most often get wrong: pasting the slug
 * into the client ID.
 *
 * Saving parses the key on the server, by the parser resolution uses. The
 * **live check is a separate button** and never happens on save: a save that
 * required reachable GitHub could not be made from a firewalled deployment
 * (research.md § R10).
 */

type SettingSource = "ENVIRONMENT" | "INSTANCE" | "DEFAULT";

interface GithubApplicationField {
  field: string;
  key: string;
  set: boolean;
  source: SettingSource;
  fixedBy: string | null;
  editable: boolean;
}

interface GithubApplication {
  scope: string;
  serves: string;
  configured: boolean;
  complete: boolean;
  source: SettingSource | null;
  clientId: string | null;
  slug: string | null;
  hasPrivateKey: boolean;
  fields: GithubApplicationField[];
  missing: string[];
  guidance: string[];
  actsFor: string[];
  resolvesTo: string | null;
  steppedOver: string[];
  steppedOverGuidance: string | null;
  lastCheckedAt: string | null;
  lastCheckOutcome: string | null;
}

const APPLICATION_FIELDS = `
  scope
  serves
  configured
  complete
  source
  clientId
  slug
  hasPrivateKey
  fields { field key set source fixedBy editable }
  missing
  guidance
  actsFor
  resolvesTo
  steppedOver
  steppedOverGuidance
  lastCheckedAt
  lastCheckOutcome
`;

async function fetchApplications(): Promise<GithubApplication[]> {
  const data = await postGraphQL<{ githubApplications: GithubApplication[] }>(
    `query GithubApplications { githubApplications { ${APPLICATION_FIELDS} } }`,
  );
  return data.githubApplications;
}

async function setApplicationField(
  scope: string,
  field: string,
  value: string | null,
): Promise<GithubApplication> {
  const data = await postGraphQL<{
    setGithubApplication: GithubApplication;
  }>(
    `mutation SetGithubApplication($scope: String!, $field: String!, $value: String) {
      setGithubApplication(scope: $scope, field: $field, value: $value) { ${APPLICATION_FIELDS} }
    }`,
    { scope, field, value },
  );
  return data.setGithubApplication;
}

async function checkApplication(scope: string): Promise<GithubApplication> {
  const data = await postGraphQL<{ checkGithubApplication: GithubApplication }>(
    `mutation CheckGithubApplication($scope: String!) {
      checkGithubApplication(scope: $scope) { ${APPLICATION_FIELDS} }
    }`,
    { scope },
    // A live check crosses the public internet from the operator's own server.
    // The default 30s is a guard against a hung connection, not a budget, and
    // GitHub answering slowly is not a reason to tell an operator their
    // application is broken.
    { timeoutMs: 45_000 },
  );
  return data.checkGithubApplication;
}

const SCOPE_LABEL: Record<string, string> = {
  global: "Global application",
  sync: "Lore synchronisation",
  feedback: "Feedback",
};

const FIELD_LABEL: Record<string, string> = {
  client_id: "Client ID",
  slug: "URL slug",
  private_key: "Private key",
};

const FIELD_HELP: Record<string, string> = {
  client_id:
    "The value GitHub calls the client ID. Not the numeric app ID and not the slug — swapping them fails as an authentication error that reads like a bad key.",
  slug: "The last segment of the application's github.com/apps/… address. The install link is built from it.",
  private_key:
    "The PEM itself, a PEM with literal \\n escapes, or the PEM base64-encoded. A mounted secret file is set with the matching _PRIVATE_KEY_FILE environment variable instead, so the file stays the only place the key lives.",
};

/** The one sentence FR-020 is about, wherever global credentials are set. */
function actsForSentence(app: GithubApplication): string {
  if (app.scope !== "global") {
    return app.actsFor.length > 0
      ? `In use for ${app.serves}.`
      : `Not in use — ${app.serves} is currently using the global application.`;
  }
  if (!app.complete) {
    return "Once complete, this application will act for every subsystem that has no application of its own — and that list grows as subsystems are added.";
  }
  if (app.actsFor.length === 0) {
    return "Currently acting for no subsystem: each one has an application of its own. It will act for any subsystem added later that has none.";
  }
  return `Currently acting for: ${app.actsFor
    .map((scope) => SCOPE_LABEL[scope] ?? scope)
    .join(", ")}. That list grows as subsystems are added.`;
}

function sourceNote(field: GithubApplicationField): string | null {
  if (field.source === "ENVIRONMENT" && field.fixedBy) {
    return `Fixed by ${field.fixedBy}`;
  }
  if (field.set && field.source === "INSTANCE") {
    return "Set here";
  }
  return null;
}

function ApplicationCard({
  app,
  onSaved,
}: {
  app: GithubApplication;
  onSaved: (next: GithubApplication) => void;
}) {
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  const save = async (field: string, value: string | null) => {
    setBusy(field);
    setStatus(null);
    setFailed(false);
    try {
      onSaved(await setApplicationField(app.scope, field, value));
      setDrafts((current) => ({ ...current, [field]: "" }));
      setStatus(`${FIELD_LABEL[field] ?? field} saved.`);
    } catch (error) {
      setFailed(true);
      setStatus(
        error instanceof Error ? error.message : "The value was not stored.",
      );
    } finally {
      setBusy(null);
    }
  };

  const check = async () => {
    setBusy("check");
    setStatus(null);
    setFailed(false);
    try {
      onSaved(await checkApplication(app.scope));
    } catch (error) {
      setFailed(true);
      setStatus(
        error instanceof Error ? error.message : "The check could not run.",
      );
    } finally {
      setBusy(null);
    }
  };

  return (
    <Card
      surface="parchment"
      className="grid gap-4 p-6"
      data-testid={`github-app-${app.scope}`}
    >
      <div className="grid gap-1">
        <div className="flex flex-wrap items-center gap-2">
          <h3 className="text-lg font-semibold">
            {SCOPE_LABEL[app.scope] ?? app.scope}
          </h3>
          <span data-testid={`github-app-${app.scope}-state`}>
            <StatusBadge
              variant={
                app.complete ? "success" : app.configured ? "warning" : "info"
              }
            >
              {app.complete
                ? "Complete"
                : app.configured
                  ? "Incomplete"
                  : "Not configured"}
            </StatusBadge>
          </span>
        </div>
        <p className="text-sm text-muted-foreground">Used by {app.serves}.</p>
        <p
          className="text-sm text-muted-foreground"
          data-testid={`github-app-${app.scope}-acts-for`}
        >
          {actsForSentence(app)}
        </p>
      </div>

      {/* FR-021: a subsystem that was half-written and stepped over whole. */}
      {app.steppedOverGuidance ? (
        <span data-testid={`github-app-${app.scope}-stepped-over`}>
          <StatusBadge variant="warning">{app.steppedOverGuidance}</StatusBadge>
        </span>
      ) : null}

      {app.scope !== "global" && app.resolvesTo === "global" ? (
        <p
          className="text-sm text-muted-foreground"
          data-testid={`github-app-${app.scope}-resolves-to`}
        >
          This subsystem is using the global application.
        </p>
      ) : null}

      {app.guidance.length > 0 ? (
        <ul
          className="grid gap-1 text-sm text-muted-foreground"
          data-testid={`github-app-${app.scope}-guidance`}
        >
          {app.guidance.map((line) => (
            <li key={line}>{line}</li>
          ))}
        </ul>
      ) : null}

      <div className="grid gap-3">
        {app.fields.map((field) => {
          const note = sourceNote(field);
          const isKey = field.field === "private_key";
          const current = isKey
            ? // The one vocabulary this screen has for a key.
              app.hasPrivateKey
              ? "Set"
              : "Not set"
            : field.field === "client_id"
              ? (app.clientId ?? "")
              : (app.slug ?? "");

          return (
            <div key={field.key} className="grid gap-1">
              <label className="grid gap-1 text-sm">
                <span className="flex flex-wrap items-center gap-2">
                  {FIELD_LABEL[field.field] ?? field.field}
                  {note ? (
                    <span
                      className="text-xs text-muted-foreground"
                      data-testid={`github-app-${app.scope}-${field.field}-source`}
                    >
                      {note}
                    </span>
                  ) : null}
                </span>
                {field.editable ? (
                  <input
                    type={isKey ? "password" : "text"}
                    value={drafts[field.field] ?? (isKey ? "" : current)}
                    placeholder={
                      isKey
                        ? app.hasPrivateKey
                          ? "Set — paste a new key to replace it"
                          : "Paste the PEM, or its base64 form"
                        : ""
                    }
                    onChange={(event) =>
                      setDrafts((existing) => ({
                        ...existing,
                        [field.field]: event.target.value,
                      }))
                    }
                    className="h-9 rounded-lg border border-input bg-transparent px-2.5"
                    data-testid={`github-app-${app.scope}-${field.field}-input`}
                  />
                ) : (
                  <input
                    readOnly
                    disabled
                    value={current}
                    className="h-9 rounded-lg border border-input bg-transparent px-2.5 opacity-70"
                    data-testid={`github-app-${app.scope}-${field.field}-input`}
                  />
                )}
              </label>
              <p className="text-xs text-muted-foreground">
                {FIELD_HELP[field.field]}
              </p>
              {field.editable ? (
                <div className="flex items-center gap-2">
                  <Button
                    variant="ghost"
                    disabled={busy !== null}
                    onClick={() =>
                      void save(
                        field.field,
                        drafts[field.field] ??
                          (isKey ? "" : (current as string)),
                      )
                    }
                    data-testid={`github-app-${app.scope}-${field.field}-save`}
                  >
                    {busy === field.field ? "Saving…" : "Save"}
                  </Button>
                  {field.set ? (
                    <Button
                      variant="ghost"
                      disabled={busy !== null}
                      onClick={() => void save(field.field, null)}
                      data-testid={`github-app-${app.scope}-${field.field}-clear`}
                    >
                      Clear
                    </Button>
                  ) : null}
                </div>
              ) : null}
            </div>
          );
        })}
      </div>

      <div className="flex flex-wrap items-center gap-3">
        <Button
          variant="ghost"
          disabled={busy !== null || !app.complete}
          onClick={() => void check()}
          data-testid={`github-app-${app.scope}-check`}
        >
          {busy === "check" ? "Checking…" : "Check against GitHub"}
        </Button>
        {app.lastCheckOutcome ? (
          <span
            className="text-sm text-muted-foreground"
            data-testid={`github-app-${app.scope}-check-outcome`}
          >
            {app.lastCheckOutcome}
            {app.lastCheckedAt
              ? ` · ${new Date(app.lastCheckedAt).toLocaleString()}`
              : ""}
          </span>
        ) : (
          <span className="text-sm text-muted-foreground">
            Not checked. Saving parses the key; it does not prove the host
            accepts it.
          </span>
        )}
      </div>

      {status ? (
        <span data-testid={`github-app-${app.scope}-status`}>
          <StatusBadge variant={failed ? "warning" : "success"}>
            {status}
          </StatusBadge>
        </span>
      ) : null}
    </Card>
  );
}

export function GitHubAppsPanel() {
  const [applications, setApplications] = useState<GithubApplication[] | null>(
    null,
  );
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    fetchApplications()
      .then((next) => {
        setApplications(next);
        setError(null);
      })
      .catch((cause: unknown) => {
        setError(
          cause instanceof Error
            ? cause.message
            : "The GitHub applications could not be read.",
        );
      });
  }, []);

  useEffect(load, [load]);

  const replace = (next: GithubApplication) => {
    // Every scope's `actsFor` can change when any one of them changes, so a
    // save re-reads rather than patching the one card in place. Showing a
    // stale "acting for" list is the specific way this screen could lie.
    setApplications((current) =>
      current
        ? current.map((app) => (app.scope === next.scope ? next : app))
        : current,
    );
    load();
  };

  if (error) {
    return (
      <span data-testid="github-apps-error">
        <StatusBadge variant="warning">{error}</StatusBadge>
      </span>
    );
  }

  if (!applications) {
    return (
      <p className="text-sm text-muted-foreground">
        Reading this instance's GitHub applications…
      </p>
    );
  }

  return (
    <div className="grid gap-4" data-testid="github-apps-panel">
      <div className="grid gap-1">
        <p className="text-sm text-muted-foreground">
          Register one application and let every subsystem use it, or one per
          subsystem, or a mixture. A subsystem's own application wins for that
          subsystem; the global one serves the rest.
        </p>
        <p className="text-sm text-muted-foreground">
          An application is used whole. A subsystem application that is missing
          a value is skipped entirely rather than completed from the global one
          — a client ID from one registration with a key from another is an
          authentication failure that reads like a bad key.
        </p>
      </div>
      {applications.map((app) => (
        <ApplicationCard key={app.scope} app={app} onSaved={replace} />
      ))}
    </div>
  );
}
