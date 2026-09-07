# Contract: First run

Setup already exists and works. This contract extends it; it does not replace
it, and it deliberately keeps the REST shape rather than moving to GraphQL.

## What is there today

Registered in `src/server/src/auth/mod.rs::router()`:

| Route | Handler | Takes |
|---|---|---|
| `GET /authentication/setup/status` | `setup_status` | — |
| `POST /authentication/setup/basic` | `admin_setup_basic` | `{ admin_code, username, email, password }` |
| `POST /authentication/setup/oauth/{provider_key}/start` | `admin_setup_oauth_start` | `{ admin_code, redirect_uri, username, return_to }` |
| `GET /authentication/setup/oauth/{provider_key}/callback` | `admin_setup_oauth_callback` | provider redirect |

State lives in `admin_bootstrap_setup` (singleton, `id = 1`):
`setup_completed_at`, `admin_code_hash`, `admin_code_generated_at`.
"Uninitialised" is `!(any user with is_admin = true) && setup_completed_at IS NULL`.

**Why REST stays REST**: these endpoints answer before anybody is signed in
and before an administrator exists, which is the one part of the product where
`admin_user(ctx)?` cannot be the gate. The bootstrap code is the gate instead.
Moving the flow to GraphQL would mean an unauthenticated mutation root, which
is a larger change than this feature needs and a worse boundary than the one
that is there.

## What this feature adds

```jsonc
// GET /authentication/setup/status  — response gains:
{
  "setup_required": true,
  "setup_completed": false,
  "configured_oauth_providers": [ … ],   // unchanged
  "access_policy": "invite_only",        // unchanged
  "accepting_access_requests": false,    // unchanged

  // NEW — what setup still needs, so the wizard is driven by the registry
  // rather than by a hard-coded list of steps.
  "required_settings": [
    { "key": "operator.name", "kind": "TEXT", "satisfied": false,
      "source": null, "fixed_by": null },
    { "key": "support_email", "kind": "EMAIL", "satisfied": true,
      "source": "ENVIRONMENT", "fixed_by": "THUNDERFORGE_SUPPORT_EMAIL" }
  ],
  // NEW — FR-002a. Owned by spec 041; reported here so setup knows it is not done.
  "second_factor_confirmed": false
}
```

```jsonc
// POST /authentication/setup/settings   — NEW
// One step of the pass. Writes immediately, so FR-006's resumability is a
// property of the storage rather than of a session kept alive.
{ "admin_code": "…", "values": { "operator.name": "…", "notice.contact_email": "…" } }

// 200 → { "status": "success", "remaining": [ … ] }
// 400 → { "status": "error", "code": "invalid_value",
//         "field": "notice.contact_email",
//         "message": "A contact address at a reserved domain (.example) cannot
//                     receive a copyright notice. Use an address you monitor." }
// 409 → { "status": "error", "code": "fixed_by_environment",
//         "field": "support_email", "fixed_by": "THUNDERFORGE_SUPPORT_EMAIL" }

// POST /authentication/setup/complete   — NEW
{ "admin_code": "…" }
// 200 → setup_completed_at written; the bootstrap code is consumed
// 409 → { "code": "incomplete", "missing": ["operator.name"] }
// 409 → { "code": "second_factor_required" }
```

## Rules

1. **The wizard is driven by the registry, not by a list of steps.** Adding a
   `RequiredAtSetup` declaration adds a field to setup with no change to
   `SetupPage.tsx`'s structure. This is FR-012 reaching the interface.
2. **Every step writes as it is completed** (FR-006). Abandon the browser and
   come back: the answered questions are answered. Nothing is held in a
   session, a cookie or component state across a page load.
3. **A setting the environment has fixed is shown as fixed and is not asked
   for** (FR-009). It counts as satisfied. An instance configured entirely by
   environment can therefore reach `/complete` having been asked only for the
   administrator account and the second factor.
4. **`/complete` is refused unless every `RequiredAtSetup` declaration
   resolves and the first administrator's `two_factor_confirmed_at` is
   non-null** (FR-002a). The predicate lives here; **the enrolment flow is
   spec 041's and is not designed in this feature.** Until 041 lands, the gate
   is satisfiable only through the existing `two_factor_setup_start`/confirm
   endpoints — 040 is finishable but not pleasant, and 041 makes it pleasant.
   See research.md § D1 before scheduling.
5. **Optional things may be skipped, and setup then says what is unset**
   (FR-003, FR-005 scenario 5). The completion response carries the readiness
   report, so the operator's last screen is the honest one.
6. **Setup is completable exactly once** (FR-006). Completion is one
   `conn.transaction`: re-check "does an admin exist", write the user, write
   `setup_completed_at`, consume the code. Today those are three statements
   with no transaction and the loser of a race gets a generic `500`; after
   this, the loser gets `409 setup_complete`. The pattern is
   `instance_identity::instance_id`'s, in this codebase, for this reason.
7. **An unconsumed bootstrap code survives a restart.**
   `ensure_admin_bootstrap_code` currently generates a fresh code on every
   start while setup is incomplete, so closing the browser and restarting the
   container kills the link with no explanation. It reuses an existing
   unconsumed code instead, and offers a deliberate regeneration. Without this
   FR-006 is a claim rather than a behaviour.
8. **The logged setup URL stops hard-coding `http://127.0.0.1:5173`.** It is
   the first thing an operator running a container sees and it is wrong for
   every one of them.

## Failure shapes

| Situation | Result |
|---|---|
| Two people POST `/complete` at the same instant | One succeeds; the other gets `409 setup_complete`. Never a `500` |
| Required value blank or an obvious placeholder | `400 invalid_value`, naming the field and what would be acceptable |
| A required value the environment already fixed | Not asked for; reported as satisfied with `fixed_by` |
| Second factor not confirmed | `409 second_factor_required` — setup stays resumable, nothing is rolled back |
| Browser closed mid-pass, container restarted | The same code still works; answered steps are still answered |
| An instance that already has an administrator | `409 setup_complete` on every setup route, as today |

## What is deliberately absent

- **No "skip setup entirely" flag.** An instance configured wholly by
  environment reaches the end of setup in two screens; a flag to bypass the
  administrator account would be a way to have an instance with no
  administrator, which is a different and worse feature.
- **No email verification of the administrator's address.** There is no
  email-verification flow in the product (ADR-042 deferred it for the same
  reason), and FR-001b of spec 041 is explicit that a fresh instance may have
  no working mail server at the moment its first administrator enrols. Setup
  must not depend on the thing setup is configuring.
