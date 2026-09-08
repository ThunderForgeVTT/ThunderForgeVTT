# Configuring an instance

Spec 040. This is the operator's and the contributor's side of the same thing:
how a value gets from somewhere into the running server, and how to add a new
one without re-deciding any of it.

## One precedence rule, for everything

```
environment variable  →  instance setting (a row)  →  declared default
```

The first one that has a value wins, and the resolution happens **per request**
rather than at startup. That is why changing a setting takes effect without a
restart, and it is also why an environment variable cannot be edited from the
administration screen: the row underneath it would still be there and the
variable would still win, so the edit would appear to work and change nothing.
ADR-041 records what that silent no-op cost the OAuth surface before this rule
existed; the settings surface refuses the write instead, and names the variable.

There is deliberately **no process-wide cache**. `settings/resolver.rs` says
why at length: a cache is a second answer to "what is this value now", and it
goes stale in the direction that hurts.

### Where a value physically lives

Three backings, and the precedence rule is the same for all of them:

| Backing | Storage | Notes |
|---|---|---|
| `Row` | `instance_settings` | Everything spec 040 introduced |
| `ManifestFile` | `manifest.json` | Six keys that predate it — realm name, support email, welcome message, the three pack ids |
| `AccessPolicy` | `instance_access_settings` | Spec 035's table, read here and written **there**, because that surface keeps its own audit trail |

A row whose key nothing declares is inert, not invalid: it does not resolve, it
is reported as unrecognised, and it is **never deleted**. An operator who
downgraded and upgraded again keeps their values.

## Adding a setting

One declaration in `src/server/src/settings/registry.rs`. That is the whole of
it — precedence, redaction, validation, readiness and the admin screen all
follow from the declaration, and none of them needs editing.

The struct has no `Default` implementation and every field is required, on
purpose: a new setting **cannot compile** without deciding its environment
variable name and whether it is a secret. Try it and the compiler says
`missing fields env_aliases, env_var and secret`.

What each field buys you:

| Field | What it decides |
|---|---|
| `kind` | How it is validated, and which control the editor draws — a port gets a number box, an enum a select, prose a textarea |
| `backing` | Which of the three stores above holds it |
| `env_var`, `env_aliases` | The variables that fix it, **in order**. Aliases exist for one shape: a PEM key that may arrive as a path, as base64 or inline |
| `requirement` | `Optional`, `RequiredAtSetup` (setup will not finish without it), or `RequiredFor(capability)` (a readiness gap and a refusal at the point of use — **never** a reason the server fails to start) |
| `secret` | Encryption at rest *and* every rendering. A secret has exactly two renderings anywhere in the product: `SET` and `NOT_SET` |
| `default` | What it resolves to when nothing else does |
| `validators` | A closed list. A cleverer heuristic rejects somebody's real name |
| `capability` | What it contributes to, so readiness can say what is limited without it |
| `what_to_set`, `what_is_limited` | Rendered **verbatim** to operators. Name a variable, never a value |
| `group` | Which heading an editor files it under, so somebody setting a copyright contact is not doing it in a list of thirty |
| `since` | The version that introduced it — FR-028's "an upgrade must ask, not break" needs to know when the asking started |

### The rule the declaration exists to enforce

**A new required setting must never stop an existing deployment from
starting.** `Requirement::RequiredFor` is a gap and a refusal at the point of
use, and `readiness::report` will say what is missing and what it costs. A
deployment that upgrades into a setting it has never heard of keeps running,
and the operator finds out on the Readiness screen rather than from a server
that will not come up.

## What an operator sees

- **Admin → Instance** — every setting, its group, where the value came from,
  and its change history. A value fixed by the environment shows the variable
  that fixed it instead of a disabled box with no explanation.
- **Admin → Readiness** — what this instance can and cannot do, derived on
  every read, with the registry's own sentence for each gap.
- **Admin → Mail** — whether mail works, a test message through the real
  outbox, and the outbox itself.

## What is never rendered

No credential, anywhere: not masked, not truncated, not length-hinted. The
search for one is recorded in
`specs/040-instance-setup/credential-search.md`, including the three values
that *are* disclosed once at creation and why.
