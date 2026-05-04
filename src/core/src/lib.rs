pub mod auth;
pub mod events;
mod loops;
pub mod models;
pub mod policies;
mod utils;

// Re-export commonly used types for convenience
pub use models::{
    auth::{AuthSecuritySettings, OAuthProvider, TwoFactorSecret, User, UserSession},
    errors::{ERR_FORBIDDEN, ERR_NOT_FOUND, ERR_UNAUTHORIZED, ErrorResponse},
    version::{CORE_SCHEMA_VERSION, Migratable, SchemaVersion},
    world::{MutationResult, World, WorldEvent, WorldEventCode, WorldToken},
};

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
