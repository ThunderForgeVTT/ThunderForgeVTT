//! The caller identity behind every share read that does not authenticate.
//!
//! Four resolvers now resolve without a session — `sharedCollection`
//! (ADR-070) and `sharedAbility`, `sharedItem`, `sharedActor` (ADR-071) — and
//! each must rate-limit before its lookup, because an unguessable code is
//! unguessable only while the number of guesses is bounded.
//!
//! This lived in `mutations_collection_shares` while collections were the only
//! anonymous path. It moved here when the other three joined them: a newtype
//! four resolvers depend on is not owned by one of them, and leaving it there
//! would have made three modules import from a fourth for a reason unrelated to
//! collections.

use async_graphql::Context;

/// The caller's identity for rate-limiting purposes, put into the GraphQL
/// context by the public transport handler.
///
/// A newtype rather than a bare `String` so nothing else in the context can be
/// mistaken for it.
#[derive(Clone, Debug)]
pub struct AnonymousCaller(pub String);

/// The caller identity to rate-limit against, for a resolver that does not
/// authenticate.
///
/// An absent identity means the transport did not supply one. Falling back to a
/// shared bucket is the safe way to be wrong: it rate-limits such callers
/// together rather than exempting them.
///
/// Written once rather than four times so the four anonymous reads cannot come
/// to disagree about what an unidentified caller is — the same reasoning that
/// makes each module's refusal a constant instead of a repeated literal.
pub fn caller_id(ctx: &Context<'_>) -> String {
    ctx.data_opt::<AnonymousCaller>()
        .map(|c| c.0.clone())
        .unwrap_or_else(|| "unknown".to_string())
}

/// The six operator values the published legal pages render.
///
/// **Deliberately unauthenticated**, per spec 039's FR-056 and
/// `contracts/legal-rendering.md` rule 4: somebody who needs to serve a
/// copyright notice on this instance has no account here, and a notice contact
/// that can only be read from inside is not a notice contact.
///
/// Six values and nothing else. The type is a closed struct rather than a
/// lookup by key, so "expose one more setting" is a code change with a
/// reviewer, not a string a caller supplies. The test at the bottom of this
/// module asserts the field list against `contracts/legal-rendering.md`'s
/// token set rather than against the ones somebody remembered.
///
/// Null for an unset value: the page then renders its visible
/// `[OPERATOR — …]` marker, which `legalDocuments.test.ts` has always
/// required. A page with a blank is better than one that reads as complete and
/// names nobody.
#[derive(async_graphql::SimpleObject, Debug, Default, Clone, PartialEq, Eq)]
pub struct PublishedOperatorValues {
    pub operator_name: Option<String>,
    pub operator_contact_email: Option<String>,
    pub operator_jurisdiction: Option<String>,
    pub notice_contact_name: Option<String>,
    pub notice_contact_email: Option<String>,
    pub notice_contact_postal_address: Option<String>,
}

/// The settings keys, in the order the fields above declare them.
///
/// One array read by both the resolver and its test, so the query cannot come
/// to expose a key the contract does not list without the test noticing.
const PUBLISHED_KEYS: [&str; 6] = [
    "operator.name",
    "operator.contact_email",
    "operator.jurisdiction",
    "notice.contact_name",
    "notice.contact_email",
    "notice.contact_postal_address",
];

#[derive(Default)]
pub struct PublishedOperatorValuesQuery;

#[async_graphql::Object]
impl PublishedOperatorValuesQuery {
    /// **Deliberately unauthenticated** — spec 039 FR-056. Do not add
    /// `authenticated_user(ctx)?` here.
    ///
    /// Unlike the four anonymous share reads above, this one is **not** rate
    /// limited, and the difference is the reason those are: a share code is
    /// unguessable only while the number of guesses is bounded, and this query
    /// takes no argument to guess. It returns the same six values to everybody,
    /// which is exactly what the published legal pages already show.
    async fn published_operator_values(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<PublishedOperatorValues> {
        let state = crate::graphql::app_state(ctx)?;
        let settings = crate::settings::resolve_all(state)
            .await
            .map_err(async_graphql::Error::new)?;

        let value = |key: &str| settings.value(key).map(str::to_string);

        Ok(PublishedOperatorValues {
            operator_name: value(PUBLISHED_KEYS[0]),
            operator_contact_email: value(PUBLISHED_KEYS[1]),
            operator_jurisdiction: value(PUBLISHED_KEYS[2]),
            notice_contact_name: value(PUBLISHED_KEYS[3]),
            notice_contact_email: value(PUBLISHED_KEYS[4]),
            notice_contact_postal_address: value(PUBLISHED_KEYS[5]),
        })
    }
}

#[cfg(test)]
mod published_operator_values_tests {
    use super::*;

    /// Every key this query reads is declared, is not secret, and is one of the
    /// six `contracts/legal-rendering.md` names.
    ///
    /// The secrecy assertion is the one that matters: this resolver answers
    /// anybody at all, and a declaration marked secret reaching it would put an
    /// SMTP password on a page a browser can open with no account. Asserted
    /// against the registry rather than against the six names, so it stays true
    /// if a declaration's `secret` flag is ever changed.
    #[test]
    fn only_the_six_declared_operator_values_are_published() {
        for key in PUBLISHED_KEYS {
            let d = crate::settings::registry::declaration(key)
                .unwrap_or_else(|| panic!("`{key}` is not a declared setting"));
            assert!(
                !d.secret,
                "`{key}` is a secret and this query is unauthenticated"
            );
        }
    }

    /// The query is registered under the name the client uses, and the type it
    /// answers with carries **exactly** the six fields — no more.
    ///
    /// Read out of the real SDL rather than counted off the struct: a test that
    /// counted a literal I typed here would agree with itself forever. A
    /// seventh field would be a seventh value published to anybody with the
    /// URL, which is the whole risk of an unauthenticated query.
    #[test]
    fn the_published_type_carries_the_six_values_and_nothing_else() {
        let schema = async_graphql::Schema::build(
            crate::graphql::QueryRoot::default(),
            crate::graphql::MutationRoot::default(),
            crate::graphql::SubscriptionRoot,
        )
        .finish();
        let sdl = schema.sdl();

        assert!(
            sdl.contains("publishedOperatorValues: PublishedOperatorValues!"),
            "`publishedOperatorValues` must be reachable from the root"
        );

        let start = sdl
            .find("type PublishedOperatorValues {")
            .expect("the published type is in the schema");
        let body = &sdl[start..];
        let body = &body[..body.find("\n}").expect("the type closes")];

        let fields: Vec<&str> = body
            .lines()
            .skip(1)
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('"') && !line.starts_with('#'))
            .collect();

        assert_eq!(
            fields,
            vec![
                "operatorName: String",
                "operatorContactEmail: String",
                "operatorJurisdiction: String",
                "noticeContactName: String",
                "noticeContactEmail: String",
                "noticeContactPostalAddress: String",
            ],
            "the unauthenticated operator query publishes something other than \
             the six values `contracts/legal-rendering.md` declares"
        );
    }
}
