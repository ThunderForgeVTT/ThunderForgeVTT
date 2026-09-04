# The Signing Backend for Repository Application Tokens, and the Three Things It Traded

- **Date**: 2026-09-04
- **Status**: Accepted
- **Spec**: `specs/034-lore-git-sync/` (FR-035, FR-036a), research R1 and R5
- **Follows**: ADR-067 (the mirror determination), ADR-029 (outside code is not run)

## Context

Spec 034 authenticates to a repository host by signing a short-lived RS256 JWT
with the instance's application key and exchanging it for an installation
token. **Nothing in this workspace could sign asymmetrically** — the crypto
surface was `aes-gcm`, `sha2`, `rand` and `totp-rs`, none of which does — so
this was a new capability rather than a reuse, and worth deciding on the record
rather than by whichever crate came first to mind.

Three candidates were evaluated. Each was better than the others at something,
which is why this is an ADR and not a line in a manifest.

## Decision

**`jsonwebtoken` with the `aws_lc_rs` backend.**

## What each option was better at

### `jsonwebtoken` + `aws_lc_rs` — chosen

105 crates in the resulting tree. `aws-lc-rs` was **already in the workspace**,
pulled in by rustls, so the signing capability arrived without a single new
transitive dependency of consequence.

**The trap it nearly set.** `jsonwebtoken` v10 made its crypto backend
pluggable, and **its default feature set enables none of them**. The crate
compiles cleanly, every type resolves, and the first attempt to sign panics at
runtime with "no default CryptoProvider". A dependency added as `jsonwebtoken =
"10"` — which is exactly how it was first scaffolded here — would have passed
`cargo check`, passed code review, and failed the first time a Game Master
connected a repository. It was caught only because the crate's tests sign a JWT
and verify it against its own public key, rather than asserting that a
non-empty string came back.

That is the strongest argument for the crate's no-`reqwest` design, and it
arrived unprompted: a test that could only run against a live application would
not have caught this before shipping.

### `jsonwebtoken` + `rust_crypto` — rejected

Pure Rust, so it would compile anywhere, and it needs no C toolchain. It routes
RSA through the `rsa` crate, whose own README states plainly that it is
vulnerable to the Marvin attack and that private key recovery by a network
attacker is possible (RUSTSEC-2023-0071), with no fixed version available.

Adding it would have introduced that advisory to a workspace that does not
currently contain the crate at all.

### `jwt-simple` — rejected, and the interesting one

Proposed for a real advantage the other two lack: **built-in WebAssembly
support**, which matters for a crate intended to be reused in more than one
place.

Measured rather than argued:

| | `jsonwebtoken` + `aws_lc_rs` | `jwt-simple` + `pure-rust` |
|---|---|---|
| Crates in tree | 105 | 263 |
| `aws-lc-rs` | already present via rustls | n/a |
| `rsa` crate | absent from the workspace | pulled via `superboring` → `rsa 0.9.10` |

So it is 2.5× the dependency footprint and reaches the same advisory by a
longer road. But the dependency count is not what decided it.

**The decisive argument is what WebAssembly support would mean for this
particular crate.** It signs with the *application's private key* — instance
configuration, held by the operator's server. Any WebAssembly consumer of it is
a browser. A browser that can sign the application JWT is a browser that holds
the application's private key, which means the key ships to every user who
loads the page.

WASM compatibility here is therefore not a capability the chosen option lacks.
It is a door that should stay shut, and a crate that offers it invites someone
to open it later without noticing what they are doing.

The instinct behind the proposal is still right, and is recorded so it is not
lost: a future need to *verify* a token client-side requires only a public key,
is a genuinely different job, and `jwt-simple` would be a reasonable answer to
it. What must not happen is merging that job with this one.

## The related decision this sits beside

The same feature drives the `git` binary as a subprocess rather than using
`git2` or `gix` (research R1), and for a structurally identical reason: the
requirement decided it, not the ergonomics. FR-004c forbids anything past the
credential grant from knowing which host arranged it, and git-over-HTTPS *is*
the host-neutral protocol where a host's REST API would embed one vendor in the
synchronisation engine.

It carries its own trap, handled the same way. Authenticating a push by
embedding the token in the remote URL is the obvious approach and publishes the
credential to the process table, where `ps` makes it readable by any local user
with no privilege and nothing ever rotates it — worse than the log FR-035
forbids. The token goes through the child process environment instead, with
`argv` carrying a credential helper that names the variable rather than its
value. `the_credential_never_appears_in_arguments` asserts it against the
constructed invocation, and was mutation-tested by putting the token back into
the URL: it fails, naming the leak.

## Consequences

- `aws-lc-rs` becomes load-bearing for something beyond TLS. If rustls ever
  stops pulling it, this crate's cost changes from nothing to a real addition,
  and the comparison above should be re-run rather than assumed.
- **`thunderforge-repo-host` cannot target WebAssembly, deliberately.** That
  should be stated to anyone proposing to reuse it, because "reusable" and
  "runnable in a browser" are easy to conflate and only the first is true here.
- `git` is a runtime dependency of the server for the first time, checked at
  startup and reported the way an incomplete configuration is (FR-036c).
- The general rule worth keeping from all three findings: **a dependency that
  compiles is not a dependency that works.** Each of these was caught by a test
  that exercised the real operation — signing and verifying, inspecting a
  constructed command — rather than by one asserting a call returned something.
