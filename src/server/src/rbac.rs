// Security Audit Phase 2.2: RBAC Policy Engine
// Provides fine-grained permission checking for worlds and resources

use uuid::Uuid;
use diesel::prelude::*;
use crate::state::AppState;
use crate::models::{WorldCollaborator, PermissionGrant};

/// RBAC Roles
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    Owner,
    Editor,
    Viewer,
}

impl Role {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "OWNER" => Some(Role::Owner),
            "EDITOR" => Some(Role::Editor),
            "VIEWER" => Some(Role::Viewer),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Owner => "OWNER",
            Role::Editor => "EDITOR",
            Role::Viewer => "VIEWER",
        }
    }

    /// Check if this role has the given permission
    pub fn has_permission(&self, permission: &str) -> bool {
        match (self, permission) {
            (Role::Owner, _) => true, // OWNER has all permissions
            (Role::Editor, "view") | (Role::Editor, "edit") => true,
            (Role::Viewer, "view") => true,
            _ => false,
        }
    }
}

/// Permissions (more granular than roles)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Permission {
    View,
    Edit,
    Delete,
}

impl Permission {
    pub fn as_str(&self) -> &'static str {
        match self {
            Permission::View => "view",
            Permission::Edit => "edit",
            Permission::Delete => "delete",
        }
    }
}

/// RBAC Engine for permission checking
pub struct RbacEngine;

impl RbacEngine {
    /// Get the user's role in a world
    /// Admin users always see Owner role (superuser bypass)
    pub async fn get_user_role(
        state: &AppState,
        user_id: Uuid,
        world_id: Uuid,
        is_admin: bool,
    ) -> Result<Option<Role>, String> {
        // Admins bypass RBAC (superuser access)
        if is_admin {
            return Ok(Some(Role::Owner));
        }

        let pool = state.db_pool.clone();
        let result = tokio::task::spawn_blocking(move || {
            use crate::schema::world_collaborators;

            let mut conn = pool
                .get()
                .map_err(|e| format!("Pool error: {}", e))?;

            world_collaborators::table
                .filter(world_collaborators::world_id.eq(world_id))
                .filter(world_collaborators::user_id.eq(user_id))
                .select(world_collaborators::role)
                .first::<String>(&mut conn)
                .optional()
                .map_err(|e| format!("Query error: {}", e))
        })
        .await
        .map_err(|e| format!("Task error: {}", e))??;

        Ok(result.and_then(|r| Role::from_str(&r)))
    }

    /// Check if user can view a world
    pub async fn can_view_world(
        state: &AppState,
        user_id: Uuid,
        world_id: Uuid,
        is_admin: bool,
    ) -> Result<bool, String> {
        match Self::get_user_role(state, user_id, world_id, is_admin).await? {
            Some(role) => Ok(role.has_permission("view")),
            None => Ok(false),
        }
    }

    /// Check if user can edit a world
    pub async fn can_edit_world(
        state: &AppState,
        user_id: Uuid,
        world_id: Uuid,
        is_admin: bool,
    ) -> Result<bool, String> {
        match Self::get_user_role(state, user_id, world_id, is_admin).await? {
            Some(role) => Ok(role.has_permission("edit")),
            None => Ok(false),
        }
    }

    /// Check if user can delete a world
    pub async fn can_delete_world(
        state: &AppState,
        user_id: Uuid,
        world_id: Uuid,
        is_admin: bool,
    ) -> Result<bool, String> {
        match Self::get_user_role(state, user_id, world_id, is_admin).await? {
            Some(role) => Ok(role.has_permission("delete")),
            None => Ok(false),
        }
    }

    /// Get all permissions for a user in a world (if enabled)
    pub async fn get_user_permissions(
        state: &AppState,
        user_id: Uuid,
        world_id: Uuid,
        is_admin: bool,
    ) -> Result<Vec<Permission>, String> {
        let role = Self::get_user_role(state, user_id, world_id, is_admin).await?;

        match role {
            Some(Role::Owner) => Ok(vec![Permission::View, Permission::Edit, Permission::Delete]),
            Some(Role::Editor) => Ok(vec![Permission::View, Permission::Edit]),
            Some(Role::Viewer) => Ok(vec![Permission::View]),
            None => Ok(vec![]),
        }
    }

    /// Auto-assign OWNER role to world creator
    /// Called when a world is created
    pub async fn assign_creator_as_owner(
        state: &AppState,
        world_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), String> {
        let pool = state.db_pool.clone();
        let now = chrono::Utc::now().naive_utc();

        tokio::task::spawn_blocking(move || {
            use crate::schema::world_collaborators;

            let mut conn = pool
                .get()
                .map_err(|e| format!("Pool error: {}", e))?;

            let collaborator_id = uuid::Uuid::now_v7();
            diesel::insert_into(world_collaborators::table)
                .values((
                    world_collaborators::id.eq(collaborator_id),
                    world_collaborators::world_id.eq(world_id),
                    world_collaborators::user_id.eq(user_id),
                    world_collaborators::role.eq("OWNER"),
                    world_collaborators::created_by.eq(user_id),
                    world_collaborators::created_at.eq(now),
                    world_collaborators::updated_at.eq(now),
                ))
                .execute(&mut conn)
                .map_err(|e| format!("Insert error: {}", e))?;

            Ok::<(), String>(())
        })
        .await
        .map_err(|e| format!("Task error: {}", e))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_from_str_parses_all_roles() {
        assert_eq!(Role::from_str("OWNER"), Some(Role::Owner));
        assert_eq!(Role::from_str("EDITOR"), Some(Role::Editor));
        assert_eq!(Role::from_str("VIEWER"), Some(Role::Viewer));
        assert_eq!(Role::from_str("INVALID"), None);
    }

    #[test]
    fn role_as_str_converts_to_string() {
        assert_eq!(Role::Owner.as_str(), "OWNER");
        assert_eq!(Role::Editor.as_str(), "EDITOR");
        assert_eq!(Role::Viewer.as_str(), "VIEWER");
    }

    #[test]
    fn owner_has_all_permissions() {
        assert!(Role::Owner.has_permission("view"));
        assert!(Role::Owner.has_permission("edit"));
        assert!(Role::Owner.has_permission("delete"));
    }

    #[test]
    fn editor_has_view_and_edit() {
        assert!(Role::Editor.has_permission("view"));
        assert!(Role::Editor.has_permission("edit"));
        assert!(!Role::Editor.has_permission("delete"));
    }

    #[test]
    fn viewer_has_only_view() {
        assert!(Role::Viewer.has_permission("view"));
        assert!(!Role::Viewer.has_permission("edit"));
        assert!(!Role::Viewer.has_permission("delete"));
    }

    #[test]
    fn permission_as_str() {
        assert_eq!(Permission::View.as_str(), "view");
        assert_eq!(Permission::Edit.as_str(), "edit");
        assert_eq!(Permission::Delete.as_str(), "delete");
    }
}
