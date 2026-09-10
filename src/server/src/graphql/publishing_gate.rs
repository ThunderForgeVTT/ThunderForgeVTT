//! The publishing gate: the guard that keeps it, and the fixtures the four
//! share paths' own tests use to declare an instance that may publish.
//!
//! Spec 040 FR-026 — "an instance with no notice contact MUST refuse
//! operations that publish content beyond a world, and MUST say why" — is
//! enforced by one predicate, `readiness::may_publish_beyond_world`, called by
//! the four `create_*_share_link_impl` functions. Spec 039's FR-011/FR-014 put
//! that call **at the mutation**, not in a page: a caller that never loads the
//! client is refused too.
//!
//! # Why this module is test-only, and why it lives here
//!
//! Nothing here ships. It exists because a requirement that lives only in a
//! spec is a requirement the next implementer does not read (spec 039's
//! `contracts/publishing-gate.md`), and because the four sibling modules need
//! one shared way to say "this instance is configured to publish" without
//! three copies of a mutex.
//!
//! It is declared from `mutations_collection_shares.rs` because `graphql.rs`
//! belongs to nobody in particular and a `#[path]` declaration next to the
//! canonical share path is cheaper to find than a new entry in the module
//! list. The other three files reach it through that path.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::MutexGuard;

use crate::settings::test_env::lock;

/// The whole of what `Capability::PublishBeyondWorld` requires, set to values
/// the declarations' own validators accept.
///
/// `realdomain.org` rather than anything `.local`, `.invalid` or `example.*`:
/// `settings::validate::placeholder_problem` treats an unreachable domain as
/// not-configured, so a plausible-looking address that fails validation would
/// leave the gate shut for a reason no test intended.
const NOTICE_CONTACT: [(&str, &str); 3] = [
    ("THUNDERFORGE_NOTICE_CONTACT_NAME", "The Steward"),
    (
        "THUNDERFORGE_NOTICE_CONTACT_EMAIL",
        "notices@realdomain.org",
    ),
    (
        "THUNDERFORGE_NOTICE_CONTACT_ADDRESS",
        "1 Anvil Road, Forgeton",
    ),
];

/// An instance configured (or deliberately not configured) to publish, for as
/// long as this value is alive.
///
/// It holds `settings::test_env`'s lock — the same one `readiness`'s own tests
/// take — for its whole lifetime, and restores what it found on drop. The
/// environment is process-global and `cargo test` is threaded: without the
/// shared lock, a share test that sets the notice contact and a readiness test
/// that asserts an unconfigured instance reports a gap are each other's
/// intermittent failure, in both directions.
///
/// Holding a `MutexGuard` across `.await` is fine here and only here: every
/// test in these modules is a default `#[tokio::test]`, which is
/// current-thread, so the future is never required to be `Send`, and nothing
/// inside a share test ever takes this lock again.
///
/// `clippy::await_holding_lock` is denied workspace-wide and does not see
/// through this wrapper, which is worth saying out loud rather than leaving as
/// a lucky escape: the lint exists to stop a guard blocking an executor thread
/// that other tasks need, and a single-threaded test runtime with one task is
/// the case it is not aimed at. An `async` mutex would be the alternative, and
/// it could not be shared with `settings::test_env`'s synchronous callers —
/// which is the entire reason this lock is the one being taken.
pub(crate) struct InstancePublishing {
    _lock: MutexGuard<'static, ()>,
    previous: Vec<(&'static str, Option<String>)>,
    /// The rows, taken away for as long as this guard lives.
    ///
    /// Clearing the environment is not enough: a declaration resolves from the
    /// environment **or** from a row, so an instance with a seeded
    /// `notice.contact_email` is configured no matter what the variables say.
    /// Spec 039's T005 seeded exactly that into the shared development
    /// database, and these tests stopped being able to create the condition
    /// they are about — the gate was wide open and the assertions failed with
    /// a share link in hand.
    ///
    /// `None` for the configured case, which needs nothing taken away.
    _rows: Option<crate::settings::test_env::AbsentRows>,
}

/// An instance that may publish beyond a world.
pub(crate) fn publishable_instance() -> InstancePublishing {
    with_notice_contact(true, None)
}

/// An instance with nowhere to serve a copyright notice.
///
/// Takes the `AppState` because it has to take the **rows** away as well as the
/// variables — see [`InstancePublishing`]'s `_rows`.
pub(crate) fn unpublishable_instance(state: &crate::state::AppState) -> InstancePublishing {
    with_notice_contact(false, Some(state))
}

/// The declared keys behind `Capability::PublishBeyondWorld`, as rows.
///
/// Named here rather than derived from `NOTICE_CONTACT`'s variable names,
/// because the row key and the variable name are two different vocabularies and
/// deriving one from the other is a rename away from being silently wrong.
const NOTICE_CONTACT_KEYS: [&str; 3] = [
    "notice.contact_name",
    "notice.contact_email",
    "notice.contact_postal_address",
];

fn with_notice_contact(
    configured: bool,
    state: Option<&crate::state::AppState>,
) -> InstancePublishing {
    let _lock = lock();
    let previous: Vec<(&'static str, Option<String>)> = NOTICE_CONTACT
        .iter()
        .map(|(key, _)| (*key, std::env::var(key).ok()))
        .collect();

    for (key, value) in NOTICE_CONTACT {
        // SAFETY: serialised by the lock above, which every other reader and
        // writer of these variables in this binary also takes.
        unsafe {
            if configured {
                std::env::set_var(key, value);
            } else {
                std::env::remove_var(key);
            }
        }
    }

    let _rows =
        state.map(|state| crate::settings::test_env::without_rows(state, &NOTICE_CONTACT_KEYS));

    InstancePublishing {
        _lock,
        previous,
        _rows,
    }
}

impl Drop for InstancePublishing {
    fn drop(&mut self) {
        for (key, value) in &self.previous {
            // SAFETY: the lock is still held — it is dropped after this.
            unsafe {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The guard
// ---------------------------------------------------------------------------

/// Every mutation the schema publishes whose name is `create…ShareLink`.
///
/// Read off the built SDL rather than a list kept here, because the point of
/// the guard is to notice the *fifth* one, and a hand-kept list only ever
/// contains the ones somebody remembered.
fn share_link_mutations(sdl: &str) -> Vec<String> {
    sdl.lines()
        .map(str::trim)
        .filter(|line| line.starts_with("create"))
        .filter_map(|line| {
            let name = line.split(['(', ':']).next()?.trim();
            name.ends_with("ShareLink").then(|| name.to_string())
        })
        .collect()
}

/// `createCollectionShareLink` → `create_collection_share_link`.
fn snake_case(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    for ch in name.chars() {
        if ch.is_ascii_uppercase() {
            out.push('_');
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// Every `.rs` file in the server crate, by path.
///
/// The whole crate, not just `graphql/`: an implementation moved or written
/// elsewhere must still be found, or the guard would report "no such function"
/// for a gate that is present and pass for one that is not.
fn crate_sources() -> Vec<(PathBuf, String)> {
    let mut found = Vec::new();
    collect_sources(
        Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src")),
        &mut found,
    );
    assert!(
        !found.is_empty(),
        "no source files found — the guard cannot see what it is guarding"
    );
    found
}

fn collect_sources(dir: &Path, found: &mut Vec<(PathBuf, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_sources(&path, found);
        } else if path.extension().is_some_and(|e| e == "rs")
            && let Ok(text) = std::fs::read_to_string(&path)
        {
            found.push((path, text));
        }
    }
}

/// The body of a top-level `fn` of this name, with comment lines removed.
///
/// Comments are stripped so that a doc comment *mentioning* the gate — this
/// file is full of sentences that would match — cannot stand in for a call to
/// it. A guard satisfied by prose is a decoration.
fn function_body(sources: &[(PathBuf, String)], name: &str) -> Option<String> {
    sources.iter().find_map(|(_, text)| body_of(text, name))
}

/// `function_body` for one file's text, so the stripping can be tested
/// against source the test wrote rather than against source it hopes exists.
fn body_of(text: &str, name: &str) -> Option<String> {
    let at = text.find(&format!("fn {name}("))?;
    Some(
        text[at..]
            .lines()
            .take_while(|line| *line != "}")
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// Every function in the crate whose name is `create_*_share_link_impl`.
fn share_link_impls(sources: &[(PathBuf, String)]) -> Vec<String> {
    let mut names: Vec<String> = sources
        .iter()
        .flat_map(|(_, text)| {
            text.match_indices("fn create_").filter_map(|(at, _)| {
                let rest = &text[at + 3..];
                let name = rest.split('(').next()?;
                name.ends_with("_share_link_impl")
                    .then(|| name.trim().to_string())
            })
        })
        .collect();
    names.sort();
    names.dedup();
    names
}

const GATE: &str = "may_publish_beyond_world";

#[cfg(test)]
mod tests {
    use super::*;

    use uuid::Uuid;

    use crate::graphql::mutations_ability_shares::{
        create_ability_share_link_impl, shared_ability_impl,
    };
    use crate::graphql::mutations_actor_shares::{create_actor_share_link_impl, shared_actor_impl};
    use crate::graphql::mutations_collection_shares::{
        create_collection_share_link_impl, shared_collection_impl,
    };
    use crate::graphql::mutations_collections::{
        AddCollectionMemberInput, CreateCollectionInput, add_collection_member_impl,
        create_collection_impl,
    };
    use crate::graphql::mutations_item_shares::{create_item_share_link_impl, shared_item_impl};
    use crate::state::AppState;
    use crate::test_support::{
        insert_test_ability, insert_test_actor, insert_test_item, insert_test_scene,
        insert_test_user, insert_test_world, test_app_state,
    };

    fn a_caller() -> String {
        format!("test-{}", Uuid::new_v4())
    }

    /// The guard spec 039's `contracts/publishing-gate.md` asks for, in the
    /// shape this codebase can actually hold.
    ///
    /// The contract proposes asserting that every `create*ShareLink` mutation
    /// declares a non-null `attestation` argument. That argument does not
    /// exist yet — the attestation record is spec 039's own work — so an SDL
    /// assertion about it would fail on the four mutations that are correct
    /// today, which is a guard nobody can keep. What *is* true today is that
    /// each of them must consult the gate before minting a code, and that is
    /// checked here: the schema supplies the list of mutations (so the fifth
    /// is discovered, not remembered) and the source supplies the proof.
    #[test]
    fn every_share_link_mutation_consults_the_publishing_gate() {
        let sdl = async_graphql::Schema::build(
            crate::graphql::QueryRoot::default(),
            crate::graphql::MutationRoot::default(),
            crate::graphql::SubscriptionRoot,
        )
        .finish()
        .sdl();

        let mutations = share_link_mutations(&sdl);
        for expected in [
            "createCollectionShareLink",
            "createActorShareLink",
            "createItemShareLink",
            "createAbilityShareLink",
        ] {
            assert!(
                mutations.iter().any(|m| m == expected),
                "`{expected}` was not found in the SDL — the guard is reading \
                 the schema wrongly and would pass for a missing gate"
            );
        }

        let sources = crate_sources();
        let mut checked = BTreeMap::new();
        for mutation in &mutations {
            let name = format!("{}_impl", snake_case(mutation));
            let body = function_body(&sources, &name).unwrap_or_else(|| {
                panic!(
                    "`{mutation}` publishes content beyond a world, but no \
                     `{name}` was found. Every share mutation's work belongs in \
                     a testable impl that calls `readiness::{GATE}` \
                     (spec 040 FR-026)."
                )
            });
            assert!(
                body.contains(GATE),
                "`{name}` mints a share link without calling \
                 `readiness::{GATE}`. An instance with no contact for \
                 copyright notices must refuse to publish (spec 040 FR-026), \
                 and the refusal belongs at the mutation rather than in the \
                 page (spec 039 FR-011)."
            );
            checked.insert(name, mutation.clone());
        }

        // The other direction: an impl written before its resolver is wired is
        // still a publishing path, and it is gated on the same day it is
        // written rather than on the day somebody exposes it.
        for name in share_link_impls(&sources) {
            let body = function_body(&sources, &name).expect("its own source");
            assert!(
                body.contains(GATE),
                "`{name}` mints a share link without calling `readiness::{GATE}`"
            );
        }
        assert!(
            checked.len() >= 4,
            "only {} share mutations were checked",
            checked.len()
        );
    }

    /// The guard, aimed at a fifth content type that does not exist yet.
    ///
    /// A guard that cannot detect the omission is worth less than none, so the
    /// two ways it must bite are exercised directly rather than assumed:
    /// a mutation the schema declares but no gated impl backs, and an impl
    /// whose only mention of the gate is prose about it.
    #[test]
    fn the_guard_would_notice_a_fifth_share_path() {
        let sdl = "type Mutation {\n\tcreateSceneShareLink(sceneId: UUID!): SceneShareLink!\n}";
        assert_eq!(share_link_mutations(sdl), vec!["createSceneShareLink"]);
        assert_eq!(
            snake_case("createSceneShareLink"),
            "create_scene_share_link"
        );

        // Assembled rather than written out, so that this test's own source
        // does not become the definition the guard then finds. It did, the
        // first time: the scan reads every `.rs` file in the crate, and this
        // one is a `.rs` file in the crate.
        let fifth = format!("{}_impl", snake_case("createSceneShareLink"));
        assert!(
            function_body(&crate_sources(), &fifth).is_none(),
            "the guard must fail on a mutation with no gated impl behind it"
        );

        let talks_about_it =
            format!("pub async fn {fifth}(\n    // {GATE} is consulted, honest\n    mint()\n}}\n");
        let body = body_of(&talks_about_it, &fifth).expect("found");
        assert!(
            !body.contains(GATE),
            "a comment mentioning the gate must not stand in for calling it"
        );
    }

    /// Taking the notice contact away is not a takedown.
    ///
    /// The gate is asked only when a link is *minted*. A reader holding a code
    /// that already worked is not punished for an operator's configuration
    /// changing afterwards — that would be a data-loss event triggered by a
    /// settings edit, and it would arrive with no notice to anybody. All four
    /// content types, because all four publish and three of them read without
    /// an account at all (ADR-071).
    #[tokio::test]
    async fn a_share_keeps_resolving_after_the_notice_contact_is_removed() {
        let state = test_app_state();
        let codes = share_one_of_everything(&state).await;

        let _unconfigured = unpublishable_instance(&state);
        crate::readiness::may_publish_beyond_world(&state)
            .await
            .expect_err("the instance must now refuse to publish, or this test proves nothing");

        shared_collection_impl(&state, &a_caller(), codes["collection"].clone())
            .await
            .expect("a collection shared while configured still reads");
        shared_actor_impl(&state, &a_caller(), codes["actor"].clone())
            .await
            .expect("an actor shared while configured still reads");
        shared_item_impl(&state, &a_caller(), codes["item"].clone())
            .await
            .expect("an item shared while configured still reads");
        shared_ability_impl(&state, &a_caller(), codes["ability"].clone())
            .await
            .expect("an ability shared while configured still reads");
    }

    /// One share of each kind, minted while the instance may publish. The
    /// guard is dropped before this returns, so the caller can take the
    /// opposite one.
    async fn share_one_of_everything(state: &AppState) -> BTreeMap<&'static str, String> {
        let _publishing = publishable_instance();

        let mut conn = state.db_pool.get().expect("connection");
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let actor_id = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
        let item_id = insert_test_item(&mut conn, world_id, owner_id);
        let ability_id = insert_test_ability(&mut conn, world_id, owner_id);
        drop(conn);

        let collection = create_collection_impl(
            state,
            owner_id,
            false,
            CreateCollectionInput {
                world_id,
                name: format!("Shared While Configured {}", Uuid::new_v4()),
                description: None,
            },
        )
        .await
        .expect("created");
        add_collection_member_impl(
            state,
            owner_id,
            false,
            AddCollectionMemberInput {
                collection_id: collection.id,
                member_type: "item".to_string(),
                member_id: item_id,
            },
        )
        .await
        .expect("added");

        BTreeMap::from([
            (
                "collection",
                create_collection_share_link_impl(state, owner_id, false, collection.id)
                    .await
                    .expect("collection shared")
                    .share_code,
            ),
            (
                "actor",
                create_actor_share_link_impl(state, owner_id, false, actor_id)
                    .await
                    .expect("actor shared")
                    .share_code,
            ),
            (
                "item",
                create_item_share_link_impl(state, owner_id, false, item_id)
                    .await
                    .expect("item shared")
                    .share_code,
            ),
            (
                "ability",
                create_ability_share_link_impl(state, owner_id, false, ability_id)
                    .await
                    .expect("ability shared")
                    .share_code,
            ),
        ])
    }

    /// The refusal itself, at the mutation, for the canonical path. It names
    /// what is missing and where to set it, and it does not name a value —
    /// there is not one to name.
    #[tokio::test]
    async fn an_instance_with_no_notice_contact_refuses_to_mint_a_link() {
        let state = test_app_state();
        let (owner_id, item_id) = {
            let mut conn = state.db_pool.get().expect("connection");
            let owner_id = insert_test_user(&mut conn);
            let world_id = insert_test_world(&mut conn, owner_id);
            (owner_id, insert_test_item(&mut conn, world_id, owner_id))
        };

        let _unconfigured = unpublishable_instance(&state);
        let refusal = create_item_share_link_impl(&state, owner_id, false, item_id)
            .await
            .expect_err("an instance that cannot be served a notice must not publish");
        let message = refusal.message;
        assert!(
            message.contains("copyright"),
            "the refusal must say why: {message}"
        );
        assert!(
            message.contains("Admin"),
            "the refusal must say where it is fixed: {message}"
        );
    }
}
