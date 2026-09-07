# Contract: GitHub applications at two scales

One application for everything, or one per subsystem, or a mixture — and the
operator understands what they have chosen while they are choosing it.

## What is there today, and it is most of the diagnostic contract already

`src/server/src/repo_host.rs` reads `SYNC_GITHUB_APP_*` from the environment
only:

| Variable | Meaning |
|---|---|
| `SYNC_GITHUB_APP_CLIENT_ID` | the application's client ID — **not** its numeric id, and not its slug |
| `SYNC_GITHUB_APP_SLUG` | the URL slug the install link is built from |
| `SYNC_GITHUB_APP_PRIVATE_KEY_FILE` | a path — **highest precedence**, because Docker and systemd deliver secrets as files |
| `SYNC_GITHUB_APP_PRIVATE_KEY_BASE64` | the PEM, base64 — the `.env`-safe form |
| `SYNC_GITHUB_APP_PRIVATE_KEY` | the PEM itself, real newlines or literal `\n` |

`enum RegistrationProblem` + `guidance()` already **is** FR-023 for one
subsystem: every variant names a variable, the prose names variables and never
values, and `registration_from_env` returns *every* problem rather than the
first "so an operator fixes their configuration in one pass instead of
discovering the next missing variable after each restart". The key is parsed
at configuration time, not first use, because "a key that is present but not a
key is the failure a presence check calls configured."

**None of that is rewritten.** It is generalised.

## Scopes and resolution

Scopes: `global`, and one per subsystem (`sync`, `feedback`, and whatever
comes next).

```text
For subsystem S, resolve in this order and take the first COMPLETE application:

  1. S,      environment    <S>_GITHUB_APP_*            e.g. SYNC_GITHUB_APP_*
  2. S,      instance       github_app.<s>.*            (instance_settings)
  3. global, environment    GLOBAL_GITHUB_APP_*
  4. global, instance       github_app.global.*
```

**Scope is the outer axis; source is the inner one.** The spec does not decide
this and it must be decided: FR-010 says the environment wins over the
instance, FR-019 says the subsystem wins over the global, and when an operator
sets a global app in the environment *and* a subsystem app in the admin
screens those rules point opposite ways. Specific intent wins, and the source
rule applies within it. The alternative would let a broad
`GLOBAL_GITHUB_APP_*` silently override the subsystem application an operator
had just deliberately configured — which is the surprise this spec exists to
remove. Recorded in research.md § R10 and in the ADR.

**An application resolves whole, never field by field.** A subsystem
application with a client id and no private key does **not** borrow the global
one's key. It is *incomplete*, is reported as incomplete naming the missing
variables, and resolution falls through to the global application entire,
saying so. US5's own scenario 4 asks for exactly this — "rather than a
half-configured application being silently completed" — and a client id from
one registration with a key from another is not an application, it is an
authentication failure that reads like a bad key.

## The GraphQL surface

```graphql
type GithubApplication {
  scope: String!            # "global" | "sync" | "feedback"
  configured: Boolean!
  complete: Boolean!
  source: SettingSource     # ENVIRONMENT | INSTANCE, null when absent
  clientId: String          # not a secret; GitHub publishes it
  slug: String
  hasPrivateKey: Boolean!   # never the key, never a fragment, never a length
  "Variables or fields this application is missing, by name."
  missing: [String!]!
  "Every problem, not the first — the shape registration_from_env already returns."
  guidance: [String!]!
  "Which subsystems this application will act for, as it currently resolves."
  actsFor: [String!]!
  lastCheckedAt: String
  lastCheckOutcome: String
}

extend type Query {
  "Every scope, plus how each subsystem currently resolves. Administrators only."
  githubApplications: [GithubApplication!]!
}

extend type Mutation {
  """
  Set or clear one field of one scope's application. The private key is
  accepted in the same three forms the environment accepts and is parsed on
  save (FR-022); a value that is not a key is refused now, not at first use.
  """
  setGithubApplication(scope: String!, field: String!, value: String): GithubApplication!

  """
  Ask the host whether this application actually works, and record the answer.
  Deliberately operator-initiated: a save that requires a reachable network
  cannot be made from a firewalled deployment.
  """
  checkGithubApplication(scope: String!): GithubApplication!
}
```

## Rules

1. **`SYNC_GITHUB_APP_*` keeps working, unchanged, and keeps winning for lore
   sync** (FR-024). `registration_from_env()` keeps its signature and becomes
   step 1 of `github_apps::registration_for(subsystem)`. A deployment that
   sets only those five variables sees no behaviour change at all — and that
   is the acceptance test for FR-024, written as a test.
2. **`actsFor` is shown wherever global credentials are set or offered**
   (FR-020). An operator setting a global application is told, on the same
   screen, which subsystems it will act for — including that the list grows as
   subsystems are added.
3. **Validation on save is a parse, not a network call** (FR-022, read
   deliberately). The private key is parsed exactly as `registration_from_env`
   parses it at startup. A live check is a separate, explicit mutation whose
   result is recorded. A configuration screen that refuses to store a correct
   value because a network was down is worse than one that stores it and says
   it has not been proven yet.
4. **No diagnostic prints a credential, a fragment or a length** (FR-023,
   SC-007). `hasPrivateKey` is a boolean. `guidance` names variables. The
   existing `RegistrationProblem::guidance()` strings already obey this and
   are reused verbatim where they fit.
5. **A revoked credential is the subsystem's failure and this feature's
   diagnostic** (spec Edge Case). The subsystem fails as it does today;
   `lastCheckOutcome` and readiness are where an operator finds out why.
6. **Private keys are stored encrypted** through `crypto.rs`, like every other
   secret this feature stores. The `_FILE` form is not stored at all — a path
   is stored and read on use, so a Docker secret stays a Docker secret.

## Failure shapes

| Situation | Result |
|---|---|
| Only global configured | Every subsystem uses it; `actsFor` lists them all |
| Global + subsystem configured | The subsystem's wins for it; the global still serves the rest |
| Subsystem half-configured | Reported incomplete with the missing names; falls through to global, and says so |
| Client id and slug swapped | The existing `MissingAppId`/`MissingAppSlug` guidance already warns these are different; a live check reports an authentication failure |
| A key that is not a key | Refused on save, naming the field. Never stored, never "configured" |
| `_BASE64` set to something that is not base64 | Refused against **that** field, not as a generic unreadable-key error — the existing `UndecodableBase64Key` distinction, preserved |
