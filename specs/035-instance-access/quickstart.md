# Quickstart: Proving Instance Access Works

**Plan**: [plan.md](./plan.md) | **Contracts**: [contracts/](./contracts/)

How to demonstrate US1 and US2 end to end, and which automated check stands in
for each success criterion. This is a validation guide — implementation belongs
in `tasks.md`.

---

## Prerequisites

```bash
docker compose up -d          # postgres + rustfs
pnpm install
pnpm run dev                  # backend :30000, web :5173
```

At least one OAuth provider must be **configured and enabled** for the US1
scenarios that matter. A run with no provider configured cannot prove the
feature's central claim, because the leak this feature closes is the OAuth
path — see research §1. Configure one in `/admin` → Configuration first, or
export its env vars and restart.

---

## Scenario A — a closed instance is actually closed (US1)

1. Sign in as an administrator. Go to **/admin → Access**. Set the policy to
   **Closed**. No restart (FR-002).
2. Sign out. Load the app.
   - **Expect**: no sign-up affordance; sign-in still offered (US1-1).
3. Attempt local registration directly, bypassing the hidden UI:
   ```bash
   curl -sS -X POST localhost:30000/api/authentication/register \
     -H 'content-type: application/json' \
     -d '{"username":"walkin","email":"walkin@example.test","password":"Sup3r-Secret-Passphrase!"}'
   ```
   - **Expect**: `409` with `registration_blocked` (US1-2).
4. Repeat step 3 with an email that **already belongs to a user**.
   - **Expect**: a byte-identical response. A closed instance must not become
     an account-existence oracle (FR-008).
5. **The one that matters.** Click "Sign in with <provider>" and authenticate
   with a provider account whose verified email matches no user.
   - **Expect**: redirected to `/login?error=instance_closed`, no session
     cookie, and the user count unchanged (US1-3, SC-001).
   - *If this admits you, the feature is not done — this is the exact failure
     the spec exists to prevent.*
6. Sign in as a pre-existing user and open a world.
   - **Expect**: entirely unaffected (US1-5, SC-002).
7. As the administrator, open **/admin → Access → Recent activity**.
   - **Expect**: each refusal from steps 3 and 5 listed with its time, route
     and the policy at the time — and **no email address** (FR-007).

## Scenario B — invite one named person in (US2)

1. Set the policy to **Invite only**.
2. **/admin → Access → Invitations** → issue one, 1 use, 24h, note "Priya".
   Copy the link.
3. In a clean browser profile, open the link and register locally.
   - **Expect**: account created despite the instance not being open, signed
     in, remaining uses now 0 (US2-2).
4. Issue a second invitation. In another clean profile, open it and sign in
   with the OAuth provider for the first time.
   - **Expect**: account created and linked under ADR-042's unchanged rules
     (US2-3).
5. Re-open the first link.
   - **Expect**: refused — it is exhausted (US2-5).
6. Issue a third, **revoke** it, then open it.
   - **Expect**: the same refusal wording as step 5. Revoked and exhausted must
     be indistinguishable (US2-4).
7. As the administrator, view the redeemed invitation.
   - **Expect**: the redeeming account and time (US2-8).
8. Confirm the account from step 3 is ordinary: it can create a world, be
   granted a role, and be deleted like any other (US2-7).

**SC-004 is a stopwatch check**: from an open instance, steps 1–2 and handing
over the link should take under 3 minutes with no config file edited and no
restart.

---

## Automated checks

| Criterion | Check |
|---|---|
| SC-001 zero accounts when closed | `instance-access-gate.spec.ts` — both routes, asserts the user count |
| SC-002 existing users unaffected | same spec — sign-in and world load throughout |
| SC-003 fresh instance admits its first admin | server test: no admin + closed → bootstrap succeeds |
| SC-005 revoked admits nobody | server test + `instance-invitations.spec.ts` |
| SC-006 at most N under concurrency | server test: N+2 concurrent redemptions of an N-use invitation |
| FR-004 / FR-007 audit | server tests asserting one event per act, with no email stored |
| FR-008 no existence oracle | server test comparing the two refusal bodies |

```bash
cargo test --workspace -- --test-threads=1     # see the authoring_mode flake note
cd apps/web && pnpm exec vitest run
node scripts/e2e-parallel.mjs --shards=2 --only=instance-access-gate,instance-invitations
pnpm verify
```

> Run `cargo llvm-cov` **alone** if measuring coverage — running it beside the
> e2e shards exhausted memory on a 31GB machine and killed both.

---

## Definition of done

- [ ] Scenario A step 5 refuses, with a provider genuinely configured
- [ ] Scenario B steps 3 and 4 both admit, on an instance that is not open
- [ ] Steps 5 and 6 produce identical refusals
- [ ] `pnpm verify` 11/11; `cargo test --workspace` green; `vitest` green
- [ ] Both new e2e specs pass against a real stack
- [ ] **ADR-072 lands in the same change set** — Principle IV requires it, and
      this moves the boundary ADR-042 established
