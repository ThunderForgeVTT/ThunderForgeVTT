# Test fixtures

`throwaway-test-app-key.pem` and its `.pub.pem` half are a **2048-bit RSA
keypair generated for this test suite and registered with nothing.** It is not
a credential. It authenticates nothing, was never installed on any repository
host, and grants access to no account.

It exists so that `app_assertion.rs` can sign a JWT and verify the result
against the matching public key — proving the signing path actually produces a
verifiable RS256 signature rather than merely returning a string. That check is
the whole reason `thunderforge-repo-host` carries no `reqwest`: the crate's
rules must be exercisable **with no network and no application configured**,
and a signature you cannot verify is a rule you are hoping for.

## If a secret scanner flags this

It is a false positive, and an understandable one — a `BEGIN PRIVATE KEY` block
looks the same whoever made it. Rotating it means regenerating both halves:

```sh
openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 \
  -out throwaway-test-app-key.pem
openssl rsa -in throwaway-test-app-key.pem -pubout \
  -out throwaway-test-app-key.pub.pem
```

Nothing else needs changing; the tests read whatever is here.

## What must never live here

A real application's private key. That is instance configuration, supplied by
the operator to a running server, and it must never enter this repository — see
`specs/034-lore-git-sync/research.md` R5.
