//! Shared error types

use serde::{Deserialize, Serialize};

/// Standard error response for GraphQL and API calls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub code: String,
    pub message: String,
    pub details: Option<String>,
}

impl ErrorResponse {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }
}

// Common error codes
pub const ERR_UNAUTHORIZED: &str = "UNAUTHORIZED";
pub const ERR_FORBIDDEN: &str = "FORBIDDEN";
pub const ERR_NOT_FOUND: &str = "NOT_FOUND";
pub const ERR_INVALID_MUTATION: &str = "INVALID_MUTATION";
pub const ERR_VALIDATION_FAILED: &str = "VALIDATION_FAILED";
pub const ERR_CONFLICT: &str = "CONFLICT";
pub const ERR_INTERNAL: &str = "INTERNAL_SERVER_ERROR";
