use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};

#[derive(DbEnum, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[ExistingTypePath = "crate::schema::sql_types::PolicyEffect"]
pub enum PolicyEffectEnum {
    Allow,
    Deny,
}
