//! Version management for schema migrations

use serde::{Deserialize, Serialize};

/// Schema version information for migrations
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct SchemaVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl SchemaVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }

    pub fn as_i32(&self) -> i32 {
        (self.major as i32) * 1_000_000 + (self.minor as i32) * 1_000 + (self.patch as i32)
    }

    pub fn from_i32(v: i32) -> Self {
        let major = (v / 1_000_000) as u32;
        let minor = ((v % 1_000_000) / 1_000) as u32;
        let patch = (v % 1_000) as u32;
        Self { major, minor, patch }
    }
}

impl std::fmt::Display for SchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Current version of core models and events
pub const CORE_SCHEMA_VERSION: SchemaVersion = SchemaVersion {
    major: 0,
    minor: 1,
    patch: 0,
};

/// Trait for types that support schema migrations
pub trait Migratable {
    /// Migrate from an older schema version to current
    fn migrate(self, from_version: i32) -> Self;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_version_ordering() {
        let v1 = SchemaVersion::new(0, 1, 0);
        let v2 = SchemaVersion::new(0, 1, 1);
        let v3 = SchemaVersion::new(0, 2, 0);
        assert!(v1 < v2 && v2 < v3);
    }

    #[test]
    fn test_schema_version_roundtrip() {
        let v = SchemaVersion::new(1, 5, 3);
        let as_int = v.as_i32();
        let v2 = SchemaVersion::from_i32(as_int);
        assert_eq!(v, v2);
    }
}
