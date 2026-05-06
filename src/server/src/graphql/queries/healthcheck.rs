//! Healthcheck query for system status monitoring.



#[derive(Default)]
pub struct HealthcheckQuery;

#[async_graphql::Object]
impl HealthcheckQuery {
    async fn healthcheck(&self) -> bool {
        true
    }
}
