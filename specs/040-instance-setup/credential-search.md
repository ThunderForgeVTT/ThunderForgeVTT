# T113 — Searching the product for a rendered credential

SC-007 says the claim is "demonstrated by **attempting** to find one", so the
attempt is the deliverable and this is the record of it. Searched
2026-09-08 against `749c5d7`.

**Result: no credential is rendered anywhere a person can read it back.** Three
values are disclosed exactly once, at the moment they are created, and none of
them is readable afterwards. Those three are listed below with the reason,
because a search that reports "nothing found" while three secrets are handed
out on purpose is a search that was not looking.

---

## What counts as a credential here

Not just the four declarations marked `secret: true`. Anything that admits
somebody, or acts as this instance:

| Family | Where it lives |
|---|---|
| `mail.password` | `instance_settings`, encrypted |
| `github_app.{global,sync,feedback}.private_key` | `instance_settings`, encrypted |
| OAuth client secrets | `oauth_providers.oauth_client_secret` |
| Recovery codes | `user_recovery_codes`, Argon2 hashes |
| TOTP secrets | `users.two_factor_secret_encrypted` |
| The bootstrap admin code | Argon2 hash in the database; plaintext only in the log line |
| Session tokens | cookie, hashed at rest |

## Surfaces searched, and how

### 1. The GraphQL read surface

`settings/graphql.rs` renders a secret as `SET` / `NOT_SET` and nothing else,
and asserts it: `no_secret_reaches_the_read_surface_as_anything_but_set_or_not_set`
iterates **every declaration** rather than the ones somebody remembered. The
`GraphQLResolvedSetting.value` field is `None` for any declaration marked
secret, decided by a `match` on the declaration and not on the value, so a
secret with a value and a secret without one take the same branch.

GitHub applications expose `hasPrivateKey: Boolean` — a boolean is the entire
vocabulary that surface has for a key. The OAuth read type exposes
`has_client_secret: bool`; the only type carrying `oauth_client_secret` is
`GraphQLOAuthProviderConfigInput`, which is an `InputObject` — a write path,
not a read.

Mail exposes no body field at all, and asserts the absence against its own
generated SDL, so adding one is a failing test rather than a review someone
has to catch.

### 2. Screens

Searched `apps/web/src` for any component rendering a secret's value, and for
masked previews — `sk-…3f9`, truncation, character counts — since a masked
preview is the tempting version of this mistake and still leaks length.

Two `•` sequences exist and neither is a credential: `OAuthProviderForm`'s
placeholder on a disabled, environment-sourced field (a placeholder, drawn
when the field is empty), and a token id prefix in `TokenPanel`, which is a
game token, not an auth token.

Every secret input is write-only: `oauthClientSecret` initialises to `""` and
is never populated from the server, and the instance settings panel renders a
secret as an empty box whose hint says blank keeps what is stored.

`InstanceSettingsPanel.test.tsx` plants a value in a setting the server would
never send one for, and requires it to be absent from the rendered markup — so
the component would fail even if the API started leaking.

### 3. Logs

Searched every `println!`, `eprintln!`, `tracing::*` and `log::*` in
`src/server/src` and `src/app/src` for a credential family. The only matches
are `storage/transcode.rs`'s benchmark table and `mutations_tokens_tests.rs`,
both about game tokens and image sizes.

`mail-delivery.spec.ts` additionally asserts, against a real instance with a
real SMTP password set, that the password appears nowhere in the rendered page
— including in input values, which `innerText` does not cover and which are
therefore read separately.

### 4. The audit trail

`instance_setting_changes` stores the **transition** for a secret, not the
value: `set` → `set`, never either side. `redacted` is stored rather than
recomputed at read time, so a row stays truthful if a declaration's `secret`
flag is ever changed. Asserted by
`a_secret_records_the_transition_and_neither_value`.

There is deliberately no column for the submitted value of a **refused** write.
A rejected password recorded anywhere is a rejected password that has leaked.

---

## The three deliberate disclosures

Each is a one-time hand-off at creation. None has a read path.

1. **Recovery codes**, returned by the body that creates them — enrolment
   confirmation and regeneration. Stored as Argon2 hashes; `two_factor_status`
   returns a remaining count and a "running low" flag and never a code.
2. **The TOTP secret**, returned by `setup/start`, because enrolling requires
   transcribing it into an authenticator.
3. **The bootstrap admin code**, printed to the server log once. The database
   holds an Argon2 hash. This is the operator's own position: they read it out
   of the log too.

## What this search did not cover

- **Backups and database dumps.** A dump contains the ciphertext and the
  instance secret is not in it, but nothing here verified that.
- **Browser storage.** `feedbackDraft` uses `sessionStorage` deliberately and
  carries no credential, but no exhaustive sweep of `localStorage` was done.
- **The engine's wasm bundle.** Out of scope: it holds no instance
  configuration.
