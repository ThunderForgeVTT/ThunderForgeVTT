//! Admin queries for system management, stats, and configuration.

use async_graphql::Context;

use crate::admin::{
    load_admin_bootstrap_settings, load_admin_stats, load_admin_welcome_summary,
    load_auth_security_settings, load_oauth_providers, read_system_manifest,
};
use crate::graphql::*;

#[derive(Default)]
pub struct AdminQuery;

#[async_graphql::Object]
impl AdminQuery {
    async fn all_worlds(&self, ctx: &Context<'_>) -> GraphQLResult<Vec<GraphQLWorld>> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        load_all_worlds(state)
            .await
            .map(|items| items.into_iter().map(GraphQLWorld::from).collect())
    }

    async fn admin_welcome_summary(
        &self,
        ctx: &Context<'_>,
    ) -> GraphQLResult<GraphQLAdminWelcomeSummary> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        load_admin_welcome_summary(state)
            .await
            .map(GraphQLAdminWelcomeSummary::from)
            .map_err(Error::new)
    }

    async fn admin_stats(&self, ctx: &Context<'_>) -> GraphQLResult<GraphQLAdminStats> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        load_admin_stats(state)
            .await
            .map(GraphQLAdminStats::from)
            .map_err(Error::new)
    }

    async fn system_manifest(&self, ctx: &Context<'_>) -> GraphQLResult<GraphQLSystemManifest> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        read_system_manifest(state)
            .map(|manifest| {
                GraphQLSystemManifest::from_document(
                    state.directories.manifest_file.clone(),
                    manifest,
                )
            })
            .map_err(Error::new)
    }

    async fn oauth_providers(&self, ctx: &Context<'_>) -> GraphQLResult<Vec<GraphQLOAuthProvider>> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        load_oauth_providers(state)
            .await
            .map(|providers| {
                providers
                    .into_iter()
                    .map(GraphQLOAuthProvider::from)
                    .collect()
            })
            .map_err(Error::new)
    }

    async fn auth_security_settings(
        &self,
        ctx: &Context<'_>,
    ) -> GraphQLResult<GraphQLAuthSecuritySettings> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        load_auth_security_settings(state)
            .await
            .map(GraphQLAuthSecuritySettings::from)
            .map_err(Error::new)
    }

    async fn admin_bootstrap_settings(
        &self,
        ctx: &Context<'_>,
    ) -> GraphQLResult<Option<GraphQLAdminBootstrapSettings>> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        load_admin_bootstrap_settings(state)
            .await
            .map(|item| item.map(GraphQLAdminBootstrapSettings::from))
            .map_err(Error::new)
    }
}
