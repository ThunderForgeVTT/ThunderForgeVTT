use async_graphql::{Error, Json, MergedObject, Result as GraphQLResult, Schema};
use base64::Engine;
use chrono::Utc;
use diesel::prelude::*;
use diesel::result::Error as DieselError;

use crate::admin::{
    load_admin_stats, recalculate_disk_usage as calculate_disk_usage,
    update_manifest_key as persist_manifest_key, update_oauth_provider as persist_oauth_provider,
    update_two_factor_policy as persist_two_factor_policy,
};
use crate::auth::world_membership::require_world_member;
use crate::models::{
    World,
    WorldActor,
    // Policy - disabled pending schema
};
use crate::schema::{world_actors, worlds}; // policies disabled
use crate::state::AppState;
// Phase 4.8.1: dnd5e_server will be loaded at runtime via game system registry

// Phase 4.9.Z Step 1: Core entity types extracted to separate module
pub mod types;
pub use types::{
    GraphQLMyWorldEntry, GraphQLUser, GraphQLWorld, GraphQLWorldEvent, GraphQLWorldToken,
};

// Phase 4.9.Z Step 2: Admin types extracted to separate module
pub mod admin_types;
pub use admin_types::{
    GraphQLAdminBootstrapSettings, GraphQLAdminStats, GraphQLAdminWelcomeSummary,
    GraphQLAuthSecuritySettings, GraphQLOAuthProvider, GraphQLOAuthProviderConfigInput,
    GraphQLSystemManifest,
};

// Phase 4.9.Z Step 3: Input & utility types extracted to separate module
pub mod input_types;
pub use input_types::{
    GraphQLCreateLightSourceInput, GraphQLCreateSceneInput, GraphQLCreateShapeInput,
    GraphQLCreateTokenInput, GraphQLCreateWallInput, GraphQLCreateWorldInput,
    GraphQLCreateWorldTokenInput, GraphQLDeleteMyDataPayload, GraphQLDeleteWorldPayload,
    GraphQLDoorState, GraphQLExportManifest, GraphQLExportMyDataPayload, GraphQLMoveTokenInput,
    GraphQLPlaceholderDomainObject, GraphQLPlayersOnlineList, GraphQLShapeKind,
    GraphQLUpdateFogMaskInput, GraphQLUpdateLightSourceInput, GraphQLUpdateSceneInput,
    GraphQLUpdateShapeInput, GraphQLUpdateTokenInput, GraphQLUpdateWallInput,
    GraphQLUpsertWorldTokenInput,
};

// Phase 4.9.Z Step 4a: Helper functions extracted to separate module
pub mod helpers;
pub use helpers::{
    admin_user, app_state, authenticated_user, get_world_id_from_scene, load_all_worlds,
    load_owned_world_event_by_id, load_owned_world_events, load_owned_world_token_by_id,
    load_owned_world_tokens, load_owned_worlds, load_visible_world_by_id, normalize_world_name,
    prepare_world_input, require_visible_world, validate_world_name, world_write_error,
};

// Phase 4.9.Z Step 5: Query extraction into separate modules
pub mod queries;
pub use queries::{
    AbilityQuery, AbilityVocabularyQuery, ActorQuery, AdminQuery, HealthcheckQuery, InventoryQuery,
    InviteQuery, ItemQuery, LoreQuery, LoreSyncQuery, ModerationQuery, RollQuery, SceneQuery,
    UserQuery, WorldContentQuery, WorldEventsSinceQuery, WorldSyncPlanQuery,
};

// Phase 4.10.B: Invite & Membership mutations for multiplayer campaigns
// Spec 026: content collections — authoring, and (separately) sharing.
pub mod mutations_collection_shares;
pub mod mutations_collections;
pub mod mutations_invites;
pub mod share_codes;
pub use mutations_invites::InviteMutation;

// Phase 6: Wall mutations (vision-blocking scene geometry)
pub mod mutations_interactives; // Spec 030: interactive elements
pub mod mutations_walls;
pub use mutations_walls::WallMutation;

// Native canvas authoring: light source mutations
pub mod mutations_lighting;
pub use mutations_lighting::LightSourceMutation;

// Native canvas authoring: shape (stroke/rect/ellipse/line/text) mutations
pub mod mutations_shapes;
pub use mutations_shapes::ShapeMutation;

// Native canvas authoring: scene-scoped token mutations
pub mod mutations_heartbeat;
pub mod mutations_reconcile;
pub mod mutations_tokens;
pub use mutations_heartbeat::{HeartbeatMutation, PresenceQuery};
pub use mutations_reconcile::ReconcileMutation;
pub use mutations_tokens::TokenMutation;

// Spec 002: canvas image asset storage (RustFS)
pub mod mutations_assets;
pub use mutations_assets::{AssetMutation, AssetQuery};

// Spec 010: actor creation/field-editing mutations
pub mod mutations_actors;
pub use mutations_actors::ActorMutation;

// Spec 010: the actor "ownership block" (Viewer/Editor/Owner grants)
pub mod mutations_actor_permissions;
pub use mutations_actor_permissions::{ActorPermissionMutation, ActorPermissionQuery};

// Spec 010: actor sharing and cross-world deep copy
pub mod mutations_actor_images; // Spec 031: portrait/token imagery, rows keyed by role
pub mod mutations_actor_shares;
pub use mutations_actor_shares::{ActorShareMutation, ActorShareQuery};

// Spec 012: lore entry creation/editing/deletion/restore mutations
pub mod mutations_lore;
pub use mutations_lore::LoreMutation;

// Spec 012: the lore entry "ownership block" (Viewer/Editor/Owner grants)
pub mod mutations_lore_permissions;
pub use mutations_lore_permissions::{LorePermissionMutation, LorePermissionQuery};

// Spec 031 (FR-038): the lore tree and its tags — move, tag, untag
pub mod mutations_lore_tree;

// Spec 012: paste/drop image upload for lore entries
pub mod mutations_lore_images;
pub use mutations_lore_images::LoreImageMutation;

// Spec 034: establishing, acknowledging and removing a world's repository
// connection. Nothing here writes to a world's lore.
pub mod mutations_lore_sync;
pub use mutations_lore_sync::LoreSyncMutation;

// Spec 013: item creation/field-editing/deletion and effect CRUD
pub mod mutations_abilities;
pub mod mutations_ability_permissions;
pub mod mutations_ability_shares;
pub mod mutations_actor_abilities;
pub mod mutations_items;
pub use mutations_abilities::AbilityMutation;
pub use mutations_ability_permissions::{AbilityPermissionMutation, AbilityPermissionQuery};
pub use mutations_ability_shares::{AbilityShareMutation, AbilityShareQuery};
pub use mutations_actor_abilities::{ActorAbilityMutation, ActorAbilityQuery};
pub use mutations_items::ItemMutation;

// Spec 013: the item "ownership block" (Viewer/Editor/Owner grants)
pub mod mutations_item_abilities;
pub mod mutations_item_permissions;
pub mod mutations_item_prices; // Spec 031: the GM's presentational price note
pub use mutations_item_permissions::{ItemPermissionMutation, ItemPermissionQuery};

// Spec 013: item sharing and cross-world deep copy
pub mod mutations_item_shares;
pub use mutations_item_shares::{ItemShareMutation, ItemShareQuery};

// Spec 013: actor inventory (Item + quantity, permissioned via the actor)
pub mod mutations_inventory;
pub use mutations_inventory::InventoryMutation;

// Spec 031: taking a placed item off the map into an inventory — one
// transaction, exactly one winner.
pub mod mutations_pickup;
pub use mutations_pickup::PickupMutation;

// Spec 031 (T032b, FR-046): `setAuthoringToolGrant` — a Game Master handing
// one player one authoring tool.
pub mod mutations_authoring_tools;
pub use mutations_authoring_tools::AuthoringToolMutation;

// Spec 031 (T055, FR-019): `bringPartyToScene` — the party's characters get a
// token in the destination, and no character gets a second one.
pub mod mutations_party;
pub use mutations_party::PartyMutation;

// Spec 015: DMCA notice-and-takedown moderation mutations
pub mod mutations_moderation;
pub use mutations_moderation::ModerationMutation;

pub mod mutations_roll;
pub use mutations_roll::RollMutation;

// Spec 018's Genie session loop used to be declared here — thirteen
// mutations and the queries beside them, 2,763 lines of one ruleset's rules
// in shared server code. It lives in `packs/systems/genie/server` now, which
// is where a pack's behaviour belongs (spec 032 FR-004, ADR-063). The
// binary merges what packs contribute into the schema roots; this file does
// not know they exist.

// Play-view Chat + Combat. Both are built on the existing `world_events`
// bus rather than a separate transport — see each module's doc comment.
pub mod mutations_chat;
pub use mutations_chat::{ChatMutation, ChatQuery};
pub mod mutations_combat;
pub use mutations_combat::{CombatMutation, CombatQuery};

// Spec 017: actor "available for claiming" flag, atomic claiming,
// player-created characters, and GM un-claim.
pub mod mutations_actor_claims;
pub use mutations_actor_claims::{ActorClaimMutation, ActorClaimQuery};

// Admin types are now in admin_types.rs module (Phase 4.9.Z Step 2)

#[path = "graphql/types_scene.rs"]
pub mod types_scene;
pub use types_scene::*;

#[path = "graphql/mutations_world_tokens.rs"]
pub mod mutations_world_tokens;
pub use mutations_world_tokens::*;

#[path = "graphql/mutations_actor_system_data.rs"]
pub mod mutations_actor_system_data;
pub use mutations_actor_system_data::*;

#[path = "graphql/mutations_scenes.rs"]
pub mod mutations_scenes;
pub use mutations_scenes::*;

#[path = "graphql/mutations_worlds.rs"]
pub mod mutations_worlds;
pub use mutations_worlds::*;

#[path = "graphql/mutations_user_data.rs"]
pub mod mutations_user_data;
pub use mutations_user_data::*;

#[path = "graphql/mutations_admin.rs"]
pub mod mutations_admin;
pub use mutations_admin::*;

#[path = "graphql/subscriptions.rs"]
pub mod subscriptions;
pub use subscriptions::*;

// Empty placeholder in the mutation root — the world_collaborators-based
// RBAC mutations this was meant to hold were never built; world/scene
// authorization instead runs through world_members (see
// src/server/src/auth/world_membership.rs).
#[derive(async_graphql::MergedObject, Default)]
pub struct CollaboratorMutation;

#[derive(MergedObject, Default)]
pub struct QueryRoot(
    PresenceQuery,
    HealthcheckQuery,
    UserQuery,
    AdminQuery,
    SceneQuery,
    queries::token_status::TokenStatusQuery,
    queries::token_attributes::TokenAttributesQuery,
    // Spec 030: `effectRegistry` and `interactives(sceneId)`.
    queries::interactives::InteractiveQuery,
    // Spec 031: `authoringTools(worldId)` — which tools the caller may use.
    queries::AuthoringToolsQuery,
    InviteQuery,
    AssetQuery,
    ActorQuery,
    ActorPermissionQuery,
    ActorShareQuery,
    LoreQuery,
    LorePermissionQuery,
    // Spec 034: the world's repository connection, its runs, and whether this
    // instance can offer the feature at all.
    LoreSyncQuery,
    AbilityQuery,
    AbilityVocabularyQuery,
    WorldContentQuery,
    mutations_item_abilities::ItemAbilityQuery,
    AbilityPermissionQuery,
    AbilityShareQuery,
    // Spec 026: a world's own collections. The ONLY listing surface here.
    mutations_collections::CollectionQuery,
    // Spec 026: `sharedCollection` — the anonymous read (ADR-070).
    mutations_collection_shares::CollectionShareQuery,
    ActorAbilityQuery,
    ItemQuery,
    ItemPermissionQuery,
    ItemShareQuery,
    InventoryQuery,
    ModerationQuery,
    RollQuery,
    ActorClaimQuery,
    ChatQuery,
    CombatQuery,
    // Spec 028: `worldSyncPlan` — what a returning client must fetch and
    // discard for one world.
    WorldSyncPlanQuery,
    // `worldEventsSince` — what a client missed while its socket was down.
    // Live delivery is at-most-once by construction, so the durable record is
    // what a reconnecting client asks, not the wire it just lost.
    WorldEventsSinceQuery,
    // Spec 028 (T086): `peerSessions` — who else is reachable right now.
    crate::peer_signaling::PeerSignalingQuery,
);

#[derive(MergedObject, Default)]
pub struct MutationRoot(
    queries::token_status::TokenDisclosureMutation,
    WorldMutation,
    UserDataMutation,
    AdminMutation,
    SceneMutation,
    WorldTokenMutation,
    ActorSystemDataMutation,
    mutations_item_abilities::ItemAbilityMutation,
    CollaboratorMutation,
    InviteMutation,
    WallMutation,
    LightSourceMutation,
    ShapeMutation,
    // Spec 030: authoring, activation and approval for interactive elements.
    mutations_interactives::InteractiveMutation,
    TokenMutation,
    AssetMutation,
    ActorMutation,
    ActorPermissionMutation,
    ActorShareMutation,
    mutations_actor_images::ActorImageMutation,
    LoreMutation,
    LorePermissionMutation,
    LoreImageMutation,
    mutations_lore_tree::LoreTreeMutation,
    LoreSyncMutation,
    AbilityMutation,
    AbilityPermissionMutation,
    AbilityShareMutation,
    // Spec 026: gather artifacts into a named collection.
    mutations_collections::CollectionMutation,
    mutations_collection_shares::CollectionShareMutation,
    ActorAbilityMutation,
    ItemMutation,
    ItemPermissionMutation,
    ItemShareMutation,
    mutations_item_prices::ItemPriceMutation,
    InventoryMutation,
    PickupMutation,
    PartyMutation,
    ModerationMutation,
    RollMutation,
    ActorClaimMutation,
    // Spec 031 (FR-046): per-player authoring tool grants.
    AuthoringToolMutation,
    ChatMutation,
    CombatMutation,
    ReconcileMutation,
    HeartbeatMutation,
    // Spec 028 (T086): `sendPeerSignal` — the post box.
    crate::peer_signaling::PeerSignalingMutation,
);

pub type AppSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

#[cfg(test)]
#[path = "graphql_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "graphql_world_default_system_tests.rs"]
mod world_default_system_tests;

#[cfg(test)]
#[path = "graphql_world_interface_pack_tests.rs"]
mod world_interface_pack_tests;
