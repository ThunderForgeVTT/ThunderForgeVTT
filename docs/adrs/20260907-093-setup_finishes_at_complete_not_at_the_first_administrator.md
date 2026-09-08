# Setup Finishes At `/complete`, Not At The First Administrator

- **Date**: 2026-09-07
- **Status**: Accepted
- **Spec**: `specs/040-instance-setup/` (FR-002, FR-002a, FR-003, FR-006),
  `contracts/setup.md` rules 4, 6, 7 and 8
- **Follows**: ADR-081, which made a confirmed second factor a thing an account
  either has or does not, and ADR-088's rule that a setting resolves from the
  registry rather than from a list somebody typed
- **Governs**: what an uninitialised instance is, and when it stops being one

## The decision

**An instance is set up when `admin_bootstrap_setup.setup_completed_at` is
written, and that happens in exactly one place:
`POST /authentication/setup/complete`.** Creating the first administrator no
longer finishes setup.

`/complete` succeeds only when:

1. every `settings::registry` declaration marked `RequiredAtSetup` resolves
   (FR-002, FR-003), and
2. the **first** administrator's `users.two_factor_confirmed_at` is non-null
   (FR-002a).

## What was there

`admin_setup_basic` created the first administrator and called
`mark_admin_setup_complete_sync` in the same closure. `setup_status` read
completion as `admin_exists || setup_completed_at IS NOT NULL`, and
`ensure_admin_bootstrap_code` wrote `setup_completed_at` at every start where
an administrator existed. Three places, one implicit rule: *an administrator
existing is the same fact as the instance being configured.*

FR-002a makes that rule false. An instance whose only administrator holds a
password and nothing else is not configured; it is halfway through being
configured. And FR-002 asks setup for four more things after the account —
operator identity, notice contact, support address, mail — none of which can be
collected after a step that has already declared the pass finished and consumed
the credential the remaining steps authenticate with.

So the account step had to move into the middle of the wizard, and something
else had to be the end of it.

## Why this shape

**Completion is a written fact, not a derived one.** `setup_completed_at` is
the single source, and `setup_is_completed` is the single reader. The one
derivation kept is a back-compatibility arm: an administrator existing *with no
bootstrap code in flight* still reads as set up, so a deployment upgraded from
before this feature, or one whose administrator was made outside setup, does
not wake up offering its own wizard to the internet. That arm is the whole of
the upgrade path and it is why `admin_code_hash` is part of the predicate
rather than just `setup_completed_at`.

**Completion is one transaction over the singleton row.** `complete_setup_
exclusively` takes `admin_bootstrap_setup` `FOR UPDATE`, re-reads, and writes.
Two people finishing at the same instant get one success and one `409
setup_complete`; before this the loser fell through a check-then-insert window
and got a generic `500` out of a unique-constraint violation. The reasoning is
`instance_identity::instance_id`'s, in this codebase, for this exact case: first
launch is when several requests arrive at once. The row-absent branch uses `ON
CONFLICT DO NOTHING`, because there is nothing to lock when the row does not
exist and the unique index is what serialises the callers instead.

**An unconsumed bootstrap code survives a restart.** It used to be reissued on
every start while setup was open, which meant closing a browser and restarting
a container silently invalidated the link the operator was given — FR-006's
resumability was a claim rather than a behaviour. The decision now lives in a
pure `bootstrap_action(admin_exists, setup_completed, has_unconsumed_code,
regeneration_requested)`, and an unconsumed code is kept.

The cost is honest: only the Argon2 hash is stored, so the code **cannot be
printed again**. The log says the earlier link is still valid rather than
pretending to reissue it, and `THUNDERFORGE_REGENERATE_SETUP_CODE=1` is the
deliberate way to replace it — refused on an instance that is already set up,
so a variable left behind in a container's environment cannot reopen setup
months later. Keeping the plaintext of a credential that creates an
administrator, purely so a log line could repeat itself, was the alternative and
it is a worse one.

## The public URL, and the setting that does not exist

The logged setup link hard-coded `http://127.0.0.1:5173/setup/{code}` — the
Vite dev server on the machine running `make dev`. It is the first thing an
operator sees and it is wrong for every containerised deployment.

**There is no public-URL setting anywhere in this product.** Checked:
`settings::registry`'s 33 declarations have none, `config::Config` holds only
`secret`, `data_path` and `secure_cookies`, and `TUNNEL_HOSTNAME` is
cloudflared's own dev-loop variable and not the instance's identity.

So `admin_bootstrap.rs` reads `THUNDERFORGE_PUBLIC_URL` directly, and when it is
unset logs the **path** plus how to get a full link, rather than a host it would
be guessing. Reading an environment variable outside the registry is a
deliberate exception to ADR-088 and it is recorded here rather than left to be
discovered: this one message has to be right *before* anything is configured
and before the database is necessarily readable, which is precisely the
condition a stored setting cannot help with. When a second caller needs the
instance's public URL — mail's message footer is the obvious candidate — it
should become a declaration, and this variable should become its `env_var`.

## What this costs

`admin_setup_basic` now leaves setup open, so any caller that treated "the
first administrator exists" as "the wizard is over" is wrong until it reads
`setup_required` from `/authentication/setup/status` instead. That is the
intended reading — the wizard has five more steps — but it is a real behaviour
change to a route that has existed since the beginning, and it is the reason
this is an ADR.

The bootstrap code also stays live for the second half of the pass. It cannot
create a second administrator (that check is unchanged), but it does authorise
`/authentication/setup/settings`. That is the design `contracts/setup.md`
specifies — every step carries the `admin_code` — and it is the same credential
guarding the same window, held open longer because the window is genuinely
longer now.
