pub mod auth;
pub mod events;
pub mod models;
mod loops;
pub mod policies;
mod utils;

// Re-export commonly used types for convenience
pub use models::{
    auth::{User, UserSession, OAuthProvider, TwoFactorSecret, AuthSecuritySettings},
    world::{World, WorldToken, WorldEvent, WorldEventCode, MutationResult},
    errors::{ErrorResponse, ERR_UNAUTHORIZED, ERR_FORBIDDEN, ERR_NOT_FOUND},
    version::{SchemaVersion, CORE_SCHEMA_VERSION, Migratable},
};

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
