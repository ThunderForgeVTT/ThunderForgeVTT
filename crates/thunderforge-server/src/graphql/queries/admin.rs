//! Admin queries for system management, stats, and configuration.

use async_graphql::Context;

use crate::admin::{
    load_admin_bootstrap_settings, load_admin_stats, load_admin_welcome_summary,
    load_auth_security_settings, load_oauth_providers, read_system_manifest,
};
use crate::graphql::*;

#[derive(Default)]
pub struct AdminQuery;

/// Where this instance's telemetry goes (spec 086, contracts/served-config-and-csp.md).
/// The tier is `full` on the wire, not `operator`, to match Appendix A.3.
#[derive(async_graphql::SimpleObject, Clone, Debug, PartialEq)]
#[graphql(name = "TelemetryStatus")]
pub struct GraphQLTelemetryStatus {
    pub enabled: bool,
    pub server_exporting: bool,
    pub server_tier: String,
    pub server_endpoint: Option<String>,
    pub browser_enabled: bool,
    pub browser_tier: String,
    pub browser_endpoint: Option<String>,
    pub instance_id: String,
}

impl From<&crate::telemetry::TelemetryStatus> for GraphQLTelemetryStatus {
    fn from(s: &crate::telemetry::TelemetryStatus) -> Self {
        Self {
            enabled: s.enabled,
            server_exporting: s.server_exporting,
            server_tier: s.server_tier_wire().to_string(),
            server_endpoint: s.server_endpoint.clone(),
            browser_enabled: s.browser.enabled,
            browser_tier: s.browser_tier_wire().to_string(),
            browser_endpoint: s.browser.enabled.then(|| s.browser.endpoint.clone()),
            instance_id: s.instance_id.clone(),
        }
    }
}

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

    async fn telemetry_status(&self, ctx: &Context<'_>) -> GraphQLResult<GraphQLTelemetryStatus> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        Ok(GraphQLTelemetryStatus::from(state.telemetry.as_ref()))
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

    /// Spec 041 FR-021. Counts, never a list of accounts — see
    /// [`GraphQLTwoFactorCoverage`].
    async fn two_factor_coverage(
        &self,
        ctx: &Context<'_>,
    ) -> GraphQLResult<GraphQLTwoFactorCoverage> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        crate::admin::load_two_factor_coverage(state)
            .await
            .map(GraphQLTwoFactorCoverage::from)
            .map_err(Error::new)
    }

    /// Spec 041 US6/US7: find the one account an operator has been asked to
    /// help, by exact username or email.
    ///
    /// Not a roster and not a prefix search. An operator with a real reason to
    /// act on somebody's second factor already knows which account it is,
    /// because that person has just asked them; a browsable list of who has no
    /// second factor is a target list for whoever takes over this session.
    async fn admin_account(
        &self,
        ctx: &Context<'_>,
        identifier: String,
    ) -> GraphQLResult<Option<GraphQLAdminAccount>> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        crate::admin::find_account_for_admin(state, &identifier)
            .await
            .map(|account| account.map(GraphQLAdminAccount::from))
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

#[cfg(test)]
mod telemetry_status_tests {
    use super::*;
    use crate::telemetry::{BrowserTelemetry, TelemetryStatus, Tier};

    #[test]
    fn off_is_off_on_both_rows_with_no_destination() {
        let s = TelemetryStatus {
            instance_id: "id-1".into(),
            ..TelemetryStatus::off_for_tests()
        };
        let wire = GraphQLTelemetryStatus::from(&s);
        assert_eq!(wire.server_tier, "off");
        assert_eq!(wire.browser_tier, "off");
        assert_eq!(wire.server_endpoint, None);
        assert_eq!(wire.browser_endpoint, None);
        assert_eq!(wire.instance_id, "id-1");
    }

    #[test]
    fn operator_is_full_on_the_wire() {
        let s = TelemetryStatus {
            enabled: true,
            server_exporting: true,
            server_tier: Tier::Operator,
            server_endpoint: Some("https://otel.example.org".into()),
            browser: BrowserTelemetry {
                enabled: true,
                ..BrowserTelemetry::off()
            },
            instance_id: "id-2".into(),
        };
        let wire = GraphQLTelemetryStatus::from(&s);
        assert_eq!(wire.server_tier, "full");
        assert_eq!(wire.browser_tier, "anonymous");
        assert_eq!(
            wire.browser_endpoint.as_deref(),
            Some("https://telemetry.thunderforge.dev")
        );
    }
}
