//! Exporting and deleting everything a user owns.

use async_graphql::{Context, Result as GraphQLResult};

use super::*;
use crate::users::{UserDataDeleteSummary, UserDataExport, delete_user_data_owned};

impl From<UserDataDeleteSummary> for GraphQLDeleteMyDataPayload {
    fn from(summary: UserDataDeleteSummary) -> Self {
        Self {
            status: "deleted".to_string(),
            message: "User profile and owned data were permanently deleted".to_string(),
            worlds_deleted: summary.worlds_deleted,
            world_tokens_deleted: summary.world_tokens_deleted,
            world_events_deleted: summary.world_events_deleted,
            policies_deleted: summary.policies_deleted,
            oauth_links_deleted: summary.oauth_links_deleted,
            sessions_deleted: summary.sessions_deleted,
            login_challenges_deleted: summary.login_challenges_deleted,
            oauth_link_challenges_deleted: summary.oauth_link_challenges_deleted,
            users_deleted: summary.users_deleted,
        }
    }
}

impl From<UserDataExport> for GraphQLExportMyDataPayload {
    fn from(export: UserDataExport) -> Self {
        Self {
            manifest: GraphQLExportManifest {
                schema_version: export.manifest.schema_version.to_string(),
                exported_at: export.manifest.exported_at,
                worlds: export.manifest.counts.worlds as i32,
                world_tokens: export.manifest.counts.world_tokens as i32,
                world_events: export.manifest.counts.world_events as i32,
                policies: export.manifest.counts.policies as i32,
            },
            user: GraphQLUser {
                id: export.user.id,
                username: export.user.username,
                email: export.user.email,
                role: export.user.role,
                is_admin: export.user.is_admin,
                created_at: export.user.created_at,
                updated_at: export.user.updated_at,
            },
            worlds: export.worlds.into_iter().map(GraphQLWorld::from).collect(),
            world_tokens: export
                .world_tokens
                .into_iter()
                .map(GraphQLWorldToken::from)
                .collect(),
            world_events: export
                .world_events
                .into_iter()
                .map(GraphQLWorldEvent::from)
                .collect(),
            // policies are disabled (module not implemented)
            policies: vec![],
            scenes: export
                .scenes
                .into_iter()
                .map(|item| GraphQLPlaceholderDomainObject {
                    schema_version: item.schema_version.to_string(),
                    status: item.status.to_string(),
                })
                .collect(),
            actors: export
                .actors
                .into_iter()
                .map(|item| GraphQLPlaceholderDomainObject {
                    schema_version: item.schema_version.to_string(),
                    status: item.status.to_string(),
                })
                .collect(),
            asset_packs: export
                .asset_packs
                .into_iter()
                .map(|item| GraphQLPlaceholderDomainObject {
                    schema_version: item.schema_version.to_string(),
                    status: item.status.to_string(),
                })
                .collect(),
            game_systems: export
                .game_systems
                .into_iter()
                .map(|item| GraphQLPlaceholderDomainObject {
                    schema_version: item.schema_version.to_string(),
                    status: item.status.to_string(),
                })
                .collect(),
        }
    }
}

// Constants and struct moved to helpers.rs module (Phase 4.9.Z Step 4a)

// Helper functions moved to helpers.rs module (Phase 4.9.Z Step 4a)

#[derive(Default)]
pub struct UserDataMutation;

#[async_graphql::Object]
impl UserDataMutation {
    async fn delete_my_data(&self, ctx: &Context<'_>) -> GraphQLResult<GraphQLDeleteMyDataPayload> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        delete_user_data_owned(state, auth_user.user_id)
            .await
            .map(GraphQLDeleteMyDataPayload::from)
            .map_err(Error::new)
    }
}
