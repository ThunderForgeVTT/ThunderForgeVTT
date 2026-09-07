# Contract: The publishing gate

What every operation that publishes content beyond its world must now do. There
are four such operations today. The contract is written so that the fifth
inherits it without anybody deciding to give it one.

## The four, as they change

```graphql
extend type Mutation {
  createCollectionShareLink(collectionId: UUID!, attestation: AttestationInput!): CollectionShareLink!
  createActorShareLink(actorId: UUID!,   attestation: AttestationInput!): ActorShareLink!
  createItemShareLink(itemId: UUID!,     attestation: AttestationInput!): ItemShareLink!
  createAbilityShareLink(abilityId: UUID!, attestation: AttestationInput!): AbilityShareLink!
}
```

`attestation` is **non-null**. A nullable argument is a requirement that is
satisfied by omission, which is the shape this feature exists to remove.

## The gate

One function, in `src/server/src/publishing.rs`, called by all four
`create_*_share_link_impl` functions before a share code is minted:

```rust
pub enum PublishableKind { Collection, Actor, Item, Ability }

pub async fn require_attestation(
    state: &AppState,
    subject: Uuid,
    kind: PublishableKind,
    target: Uuid,
    offered: &AttestationInput,
) -> GraphQLResult<PendingAttestation>;
```

It performs four checks, in this order, and the order matters because the
earlier ones must not leak the later ones:

1. **The instance can be notified.** `require_notice_contact(state)?` — an
   instance with no contact for copyright notices publishes nothing (FR-053).
   Play is untouched. Until spec 040 owns this value it is read from
   `THUNDERFORGE_NOTICE_CONTACT`.
2. **The account may publish.** `standing_of(state, subject)?.may_publish` —
   refused at the suspension rung and at disablement (FR-018).
3. **The version is one this instance knows.** `offered.terms_version_id` must
   be the current `sharing-terms` version *or* resolve in `terms_versions`
   (FR-012). Accepting a recently-superseded archived version is deliberate:
   see Rules § 3.
4. **Ownership**, which each impl already checks its own way and keeps.

On success it returns a `PendingAttestation` the impl writes **inside the
transaction that mints the share row**. There is no ordering in which one exists
without the other.

## How the fifth content type inherits this

A guard test, in the style of the existing
`the_access_surface_is_registered_under_the_names_the_client_uses`, walks the
built SDL and asserts:

> every mutation whose name matches `create*ShareLink` declares a non-null
> argument named `attestation` of type `AttestationInput`.

A share path added later fails that test the day it is written. FR-002 says the
requirement extends to new types "without a separate decision"; this is what
that sentence has to mean in code, because a requirement that lives only in a
spec is a requirement the next implementer does not read.

## Failure shapes

| Situation | Result |
|---|---|
| `attestation` absent | Rejected by the schema; the argument is non-null |
| `termsVersionId` empty or unknown | `"This share needs the current sharing agreement. Reload the page and try again."` |
| Account at the suspension rung | The standing message, naming the process and linking to the person's own standing page |
| Account disabled | The mutation is unreachable — a disabled account is refused at `authenticated_user` before the resolver runs |
| Instance has no notice contact | `"This instance cannot publish yet: no contact for copyright notices has been configured."` — and the administrator is told what is missing |
| Ownership fails | Unchanged from today, per path |
| Everything passes, the transaction rolls back | No share, no attestation. Both or neither |

## Rules

1. **The gate is in the impl, not the resolver.** The impls are the surface the
   existing unit tests exercise, and a caller that bypasses GraphQL is refused
   too. Principle III.
2. **Per publish, not per person, per world or per session** (FR-003). Nothing
   caches an attestation and nothing consults a previous one. Sharing the same
   collection twice writes two rows.
3. **A recently-archived version is accepted; an unknown one is not.** A person
   who opened the dialog thirty seconds before the operator revised the terms
   should not lose their work to a race (spec.md's edge case, and FR-005). What
   they agreed to is recorded exactly, which is the property that matters. An
   instance wanting strict currency has the version comparison one line away,
   and this contract does not offer it as a setting because two behaviours here
   is one more than anybody can reason about.
4. **Declining publishes nothing and discards nothing** (FR-005). The dialog is
   a client-side gate before a mutation is sent; a collection under assembly is
   untouched because no mutation ran.
5. **A refusal never names a valid version identity** (FR-013), including the
   expected one. The identity is obtainable a legitimate request away through
   `sharingTerms`; that is not the secret. The record is.
6. **Revoking and re-issuing a link is two publishes.** Each is a separate act
   about the same content and each attests (spec.md's edge case). The revoked
   link's attestation stays.

## What is deliberately absent

- **No bulk attestation.** SC-007 asks that sharing five things in a row stay a
  task somebody will finish, and the answer is a dialog that is short and
  remembers the words are the same, not a "attest to my next ten shares"
  affordance. That affordance is the per-publish requirement with a hole in it.
- **No exemption for a private instance, a single-user instance, or an
  administrator.** The product does not know which instance is which
  (spec.md's Assumptions), and an exemption is a code path nobody tests.
- **No attestation on adopting.** Adopting is not publishing; an adopter attests
  when they publish onward, in their own right (spec.md's Assumptions).
- **No attestation on world invites, private uploads, or spec 038's shared tab
  audio.** None of them publishes a copy beyond its world.
