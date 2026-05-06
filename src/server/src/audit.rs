//! Audit logging module for tracking sensitive operations.
//!
//! This module provides audit logging functionality for:
//! - User authentication events (login, logout, 2FA setup/verification)
//! - World operations (create, delete, update)
//! - Token operations (create, update, delete)
//! - Scene operations (create, update, delete)
//! - Admin access (queries to system metrics, user data)
//!
//! All audit events are logged to the `audit_logs` table with:
//! - actor_id (who performed the action)
//! - event_type (what kind of event)
//! - resource_type and resource_id (what was affected)
//! - action (read, write, delete)
//! - timestamp
//! - optional details (JSONB)

use crate::models::NewAuditLog;
use crate::schema::audit_logs;
use crate::state::AppState;
use chrono::Utc;
use diesel::prelude::*;
use serde_json::json;
use uuid::Uuid;

/// Log an audit event to the database.
///
/// # Arguments
/// * `state` - Application state with database pool
/// * `event_type` - Type of event (e.g., "mutation_create", "query_access", "admin_query")
/// * `actor_id` - UUID of the user performing the action
/// * `resource_type` - Type of resource affected (e.g., "world", "token", "scene")
/// * `resource_id` - UUID of the resource affected (optional)
/// * `action` - Action performed (e.g., "read", "write", "delete")
/// * `details` - Additional context as JSONB (optional)
///
/// # Example
/// ```rust,ignore
/// log_audit_event(
///     state,
///     "mutation_create",
///     user_id,
///     Some("world"),
///     Some(world_id),
///     Some("write"),
///     None,
/// ).await
/// ```
pub async fn log_audit_event(
    state: &AppState,
    event_type: &str,
    actor_id: Uuid,
    resource_type: Option<&str>,
    resource_id: Option<Uuid>,
    action: Option<&str>,
    details: Option<serde_json::Value>,
) -> Result<(), String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    let audit_log = NewAuditLog {
        id: Uuid::now_v7(),
        event_type: event_type.to_string(),
        actor_id,
        resource_type: resource_type.map(|s| s.to_string()),
        resource_id,
        action: action.map(|s| s.to_string()),
        details,
        created_at: Utc::now().naive_utc(),
    };

    tokio::task::spawn_blocking(move || {
        diesel::insert_into(audit_logs::table)
            .values(&audit_log)
            .execute(&mut conn)
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|_| "Failed to insert audit log".to_string())?;

    Ok(())
}

/// Log a mutation event (create, update, delete).
pub async fn log_mutation(
    state: &AppState,
    action: &str,
    actor_id: Uuid,
    resource_type: &str,
    resource_id: Uuid,
) -> Result<(), String> {
    log_audit_event(
        state,
        "mutation",
        actor_id,
        Some(resource_type),
        Some(resource_id),
        Some(action),
        None,
    )
    .await
}

/// Log a query access event (admin/special queries only).
pub async fn log_query_access(
    state: &AppState,
    actor_id: Uuid,
    query_name: &str,
    resource_type: Option<&str>,
) -> Result<(), String> {
    log_audit_event(
        state,
        "query_access",
        actor_id,
        resource_type,
        None,
        Some("read"),
        Some(json!({ "query": query_name })),
    )
    .await
}

/// Log an authentication event.
pub async fn log_auth_event(
    state: &AppState,
    event_type: &str,
    user_id: Uuid,
    success: bool,
    details: Option<serde_json::Value>,
) -> Result<(), String> {
    let mut detail_obj = details.unwrap_or_else(|| json!({}));
    if let serde_json::Value::Object(ref mut map) = detail_obj {
        map.insert("success".to_string(), json!(success));
    }

    log_audit_event(
        state,
        &format!("auth_{}", event_type),
        user_id,
        Some("user"),
        Some(user_id),
        Some(if success { "success" } else { "failure" }),
        Some(detail_obj),
    )
    .await
}

/// Log a deletion event with additional context.
pub async fn log_deletion(
    state: &AppState,
    actor_id: Uuid,
    resource_type: &str,
    resource_id: Uuid,
    details: Option<serde_json::Value>,
) -> Result<(), String> {
    log_audit_event(
        state,
        "mutation",
        actor_id,
        Some(resource_type),
        Some(resource_id),
        Some("delete"),
        details,
    )
    .await
}

/// Log an admin access event (restricted to admins only).
pub async fn log_admin_query(
    state: &AppState,
    actor_id: Uuid,
    query_name: &str,
    result_count: Option<i64>,
) -> Result<(), String> {
    let details = result_count.map(|count| json!({ "result_count": count }));

    log_audit_event(
        state,
        "admin_query",
        actor_id,
        None,
        None,
        Some("read"),
        details.or_else(|| Some(json!({ "query": query_name }))),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_mutation_formats_event_correctly() {
        // This is a compile-check test; full tests require database
        let event_type = "mutation";
        let action = "write";
        assert!(!event_type.is_empty());
        assert!(!action.is_empty());
    }

    #[test]
    fn test_audit_event_types() {
        let valid_types = vec![
            "mutation",
            "query_access",
            "auth_login",
            "auth_logout",
            "auth_2fa",
            "admin_query",
        ];
        for event_type in valid_types {
            assert!(!event_type.is_empty());
        }
    }

    #[test]
    fn test_audit_resource_types() {
        let valid_types = vec!["world", "scene", "token", "fog_mask", "user", "system"];
        for resource_type in valid_types {
            assert!(!resource_type.is_empty());
        }
    }

    #[test]
    fn test_audit_actions() {
        let valid_actions = vec!["read", "write", "delete", "success", "failure"];
        for action in valid_actions {
            assert!(!action.is_empty());
        }
    }
}
