use diesel_derive_enum::DbEnum;

#[derive(DbEnum, Debug, PartialEq, Eq)]
#[ExistingTypePath = "crate::schema::sql_types::PolicyEffect"]
pub enum PolicyEffectEnum {
    Allow,
    Deny,
}
