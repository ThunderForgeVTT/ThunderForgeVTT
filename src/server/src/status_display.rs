//! Resolving what each viewer is told about each token's resources.
//!
//! Spec 029. This is the boundary where entitlement is applied, so it is the
//! boundary that has to be right: **the server resolves, the client renders.**
//! A client is never sent a figure it may not display — not sent and hidden,
//! not sent. A UI that conceals a field the API still returns is a UI, not a
//! permission, and this project has already shipped one bug of that shape (a
//! hidden scene's art was reachable by asking for its id directly, because two
//! call sites answered the same question differently).
//!
//! # What decides the answer
//!
//! Three things, in order:
//!
//! 1. **The game system** declares which resources exist and where their
//!    numbers live. Nothing here knows what "health" means; it reads named
//!    fields out of a JSONB slot.
//! 2. **The actor** behind the token decides the default disclosure — see
//!    `thunderforge_canvas_core::resource_display::default_disclosure`. Your
//!    own character reads exactly, another player's does too, an NPC is
//!    chunked.
//! 3. **An explicit per-token row**, if the Game Master set one, overrides
//!    that default.
//!
//! A Game Master sees the true value throughout, whatever is stored.

use diesel::prelude::*;
use uuid::Uuid;

use thunderforge_canvas_core::resource_display::{
    Disclosed, DisclosureState, ResourceDefinition, ResourceKind, ResourceSource, TokenSubject,
    disclose, entries_from,
};

/// One token's resources, already reduced to what one viewer may see.
#[derive(Debug, Clone)]
pub struct TokenStatus {
    pub token_id: Uuid,
    pub resources: Vec<ResolvedResource>,
}

#[derive(Debug, Clone)]
pub struct ResolvedResource {
    pub definition: ResourceDefinition,
    pub disclosed: Disclosed,
}

/// A resource declaration plus where to read it, as a system's manifest gives
/// it.
#[derive(Debug, Clone)]
pub struct DeclaredResource {
    pub definition: ResourceDefinition,
    pub source: ResourceSource,
}

/// Read a system's resource declarations out of its manifest.
///
/// A system that declares none yields an empty list, and its tokens then carry
/// no bars at all — which is correct for a ruleset that tracks no pools, not a
/// gap to fill with a default.
pub fn declarations_for_system(systems_dir: &str, system_id: &str) -> Vec<DeclaredResource> {
    let path = std::path::Path::new(systems_dir)
        .join(system_id)
        .join("system.json");

    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    let Some(entries) = manifest.get("resources").and_then(|r| r.as_array()) else {
        return Vec::new();
    };

    entries
        .iter()
        .filter_map(|raw| {
            let source: ResourceSource = serde_json::from_value(raw.get("source")?.clone()).ok()?;
            let kind = match raw.get("kind")?.as_str()? {
                "bar" => ResourceKind::Bar,
                "counter" => ResourceKind::Counter,
                // A kind nothing can draw is dropped rather than guessed at.
                _ => return None,
            };
            Some(DeclaredResource {
                definition: ResourceDefinition {
                    id: raw.get("id")?.as_str()?.to_string(),
                    label: raw.get("label")?.as_str()?.to_string(),
                    kind,
                    order: raw.get("order").and_then(|o| o.as_i64()).unwrap_or(0) as i32,
                    allow_stacking: raw
                        .get("allowStacking")
                        .and_then(|a| a.as_bool())
                        .unwrap_or(false),
                },
                source,
            })
        })
        .collect()
}

/// What a token is to this viewer.
///
/// Derived rather than configured: the actor already records whether it is a
/// player character, and ownership says whose. See the note on
/// `default_disclosure`.
pub fn subject_for(viewer_id: Uuid, actor_is_npc: bool, token_owner: Option<Uuid>) -> TokenSubject {
    if actor_is_npc {
        return TokenSubject::NonPlayerCharacter;
    }
    if token_owner == Some(viewer_id) {
        TokenSubject::OwnCharacter
    } else {
        TokenSubject::PartyCharacter
    }
}

/// Everything one viewer may see about one token.
///
/// `stored` is the actor's decoded system data for the slot each resource
/// names. `overrides` are the explicit per-token rows, keyed by resource id.
pub fn resolve_token(
    token_id: Uuid,
    viewer_runs_the_world: bool,
    subject: TokenSubject,
    declarations: &[DeclaredResource],
    stored: &serde_json::Value,
    overrides: &std::collections::HashMap<String, DisclosureState>,
) -> TokenStatus {
    let mut resources = Vec::new();

    for declared in declarations {
        let entries = entries_from(stored, &declared.source);
        // A resource the actor stores nothing for is not displayed at all.
        // Showing an empty bar would claim the creature is at zero.
        if entries.is_empty() {
            continue;
        }

        // The Game Master sees the truth regardless of what is stored for
        // everybody else. This is the only branch that ignores the override,
        // and it must stay that way: a GM who has hidden a boss from the table
        // still needs to run the fight.
        let state = if viewer_runs_the_world {
            DisclosureState::Visible
        } else {
            overrides
                .get(&declared.definition.id)
                .copied()
                .unwrap_or_else(|| {
                    thunderforge_canvas_core::resource_display::default_disclosure(subject)
                })
        };

        resources.push(ResolvedResource {
            definition: declared.definition.clone(),
            disclosed: disclose(&entries, state),
        });
    }

    resources.sort_by_key(|r| r.definition.order);

    TokenStatus {
        token_id,
        resources,
    }
}

/// The explicit disclosure rows for a set of tokens.
pub fn overrides_for_tokens(
    conn: &mut PgConnection,
    token_ids: &[Uuid],
) -> QueryResult<std::collections::HashMap<Uuid, std::collections::HashMap<String, DisclosureState>>>
{
    use crate::schema::token_resource_disclosure as trd;

    let rows: Vec<(Uuid, String, String)> = trd::table
        .filter(trd::token_id.eq_any(token_ids))
        .select((trd::token_id, trd::resource_id, trd::state))
        .load(conn)?;

    let mut out: std::collections::HashMap<
        Uuid,
        std::collections::HashMap<String, DisclosureState>,
    > = std::collections::HashMap::new();

    for (token_id, resource_id, state) in rows {
        // A stored state this build does not recognise is skipped, so the
        // derived default applies. Failing closed toward *less* disclosure
        // than the row asked for is the safe direction to be wrong in.
        if let Some(parsed) = parse_state(&state) {
            out.entry(token_id).or_default().insert(resource_id, parsed);
        }
    }

    Ok(out)
}

/// Parse a stored disclosure state. Unknown values answer `None`.
pub fn parse_state(stored: &str) -> Option<DisclosureState> {
    match stored {
        "visible" => Some(DisclosureState::Visible),
        "greyed" => Some(DisclosureState::Greyed),
        "percentage" => Some(DisclosureState::Percentage),
        "chunked" => Some(DisclosureState::Chunked),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use thunderforge_canvas_core::resource_display::EntrySource;

    fn viewer() -> Uuid {
        Uuid::from_u128(1)
    }

    fn health() -> DeclaredResource {
        DeclaredResource {
            definition: ResourceDefinition {
                id: "health".into(),
                label: "Health".into(),
                kind: ResourceKind::Bar,
                order: 0,
                allow_stacking: false,
            },
            source: ResourceSource {
                slot: "resourceData".into(),
                entries: vec![EntrySource {
                    current: "current_health".into(),
                    max: Some("max_health".into()),
                    label: None,
                    max_value: None,
                    optional: false,
                }],
            },
        }
    }

    fn stored(current: i64, max: i64) -> serde_json::Value {
        serde_json::json!({ "current_health": current, "max_health": max })
    }

    /// The exact strings the column holds.
    ///
    /// Asserted against literals rather than round-tripped through a
    /// serialiser, because these values are written by a migration and by the
    /// disclosure mutation, and a round-trip test would agree with itself
    /// while both drifted away from the database.
    #[test]
    fn the_stored_spellings_are_the_ones_the_column_contains() {
        assert_eq!(parse_state("visible"), Some(DisclosureState::Visible));
        assert_eq!(parse_state("greyed"), Some(DisclosureState::Greyed));
        assert_eq!(parse_state("percentage"), Some(DisclosureState::Percentage));
        assert_eq!(parse_state("chunked"), Some(DisclosureState::Chunked));
    }

    #[test]
    fn an_unrecognised_stored_state_falls_back_rather_than_guessing() {
        assert_eq!(parse_state("Visible"), None);
        assert_eq!(parse_state("exact"), None);
    }

    /// The default in action: a player reads their own character exactly.
    #[test]
    fn a_player_sees_their_own_token_exactly_with_nothing_configured() {
        let status = resolve_token(
            Uuid::from_u128(9),
            false,
            subject_for(viewer(), false, Some(viewer())),
            &[health()],
            &stored(7, 12),
            &HashMap::new(),
        );

        match &status.resources[0].disclosed {
            Disclosed::Visible { entries } => assert_eq!(entries[0].current, 7),
            other => panic!("expected exact, got {other:?}"),
        }
    }

    /// And an NPC is coarsened without anybody configuring anything.
    #[test]
    fn a_player_sees_an_npc_chunked_with_nothing_configured() {
        let status = resolve_token(
            Uuid::from_u128(9),
            false,
            subject_for(viewer(), true, None),
            &[health()],
            &stored(7, 12),
            &HashMap::new(),
        );

        match &status.resources[0].disclosed {
            // 7/12 is in the second quarter.
            Disclosed::Chunked { quarter } => assert_eq!(*quarter, 2),
            other => panic!("expected chunked, got {other:?}"),
        }
    }

    /// The Game Master's view ignores the override, and must.
    #[test]
    fn the_game_master_sees_the_truth_even_where_the_table_sees_a_band() {
        let mut overrides = HashMap::new();
        overrides.insert("health".to_string(), DisclosureState::Greyed);

        let gm = resolve_token(
            Uuid::from_u128(9),
            true,
            TokenSubject::NonPlayerCharacter,
            &[health()],
            &stored(7, 12),
            &overrides,
        );

        match &gm.resources[0].disclosed {
            Disclosed::Visible { entries } => assert_eq!(entries[0].current, 7),
            other => panic!("a GM must see the value they hid: {other:?}"),
        }
    }

    #[test]
    fn an_explicit_override_beats_the_derived_default() {
        let mut overrides = HashMap::new();
        overrides.insert("health".to_string(), DisclosureState::Visible);

        let status = resolve_token(
            Uuid::from_u128(9),
            false,
            TokenSubject::NonPlayerCharacter,
            &[health()],
            &stored(7, 12),
            &overrides,
        );

        assert!(
            matches!(status.resources[0].disclosed, Disclosed::Visible { .. }),
            "a GM who reveals a boss must be obeyed"
        );
    }

    /// A resource with nothing stored is absent, not empty.
    #[test]
    fn a_resource_the_actor_stores_nothing_for_is_not_displayed() {
        let status = resolve_token(
            Uuid::from_u128(9),
            false,
            TokenSubject::OwnCharacter,
            &[health()],
            &serde_json::json!({}),
            &HashMap::new(),
        );

        assert!(
            status.resources.is_empty(),
            "an empty bar would claim the creature is at zero"
        );
    }

    /// The assertion SC-004 is really about, at the layer that decides it.
    #[test]
    fn no_coarse_resolution_carries_the_exact_figure() {
        for state in [
            DisclosureState::Greyed,
            DisclosureState::Percentage,
            DisclosureState::Chunked,
        ] {
            let mut overrides = HashMap::new();
            overrides.insert("health".to_string(), state);

            let status = resolve_token(
                Uuid::from_u128(9),
                false,
                TokenSubject::NonPlayerCharacter,
                &[health()],
                &stored(37, 250),
                &overrides,
            );

            let json = serde_json::to_string(&status.resources[0].disclosed).unwrap();
            assert!(!json.contains("37"), "{state:?} leaked the current: {json}");
            assert!(
                !json.contains("250"),
                "{state:?} leaked the maximum: {json}"
            );
        }
    }
}
