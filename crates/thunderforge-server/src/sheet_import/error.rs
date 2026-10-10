//! Why bringing a sheet in was refused, by the code the client shows
//! (contracts/graphql-sheet-import.md, "Common errors").

use async_graphql::{Error, ErrorExtensions};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SheetImportError {
    /// `feature.sheet_import` is off.
    Disabled,
    /// The caller may not edit this actor.
    Forbidden(String),
    /// Another rule refused it (a paused world), with that rule's own code.
    Refused {
        code: String,
        message: String,
    },
    /// A reader or a bound refused the document: `SHEET_ENCRYPTED`,
    /// `SHEET_TOO_LARGE`, `SHEET_TOO_MANY_PAGES`, `SHEET_UNREADABLE`,
    /// `SHEET_NOT_RECOGNISED`.
    Sheet {
        code: &'static str,
        message: String,
    },
    /// The actor's system declares no `sheetImport`.
    NoMapping,
    /// The plan the server makes is not the one the person reviewed.
    PlanChanged,
    /// A write the validators refused, or the database failed.
    Invalid(String),
    Storage(String),
    Database(String),
}

impl SheetImportError {
    pub fn code(&self) -> String {
        match self {
            Self::Disabled => "FEATURE_DISABLED",
            Self::Forbidden(_) => "FORBIDDEN",
            Self::Refused { code, .. } => code.as_str(),
            Self::Sheet { code, .. } => code,
            Self::NoMapping => "SYSTEM_HAS_NO_MAPPING",
            Self::PlanChanged => "PLAN_CHANGED",
            Self::Invalid(_) => "VALIDATION_FAILED",
            Self::Storage(_) => "STORAGE_FAILED",
            Self::Database(_) => "INTERNAL",
        }
        .to_string()
    }

    pub fn message(&self) -> String {
        match self {
            Self::Disabled => "Bringing a character in is switched off on this server.".into(),
            Self::Forbidden(message) | Self::Refused { message, .. } => message.clone(),
            Self::Sheet { message, .. } => message.clone(),
            Self::NoMapping => "This game system does not take character sheets.".into(),
            Self::PlanChanged => "The sheet reads differently now. Review it again.".into(),
            Self::Invalid(message) => format!("The sheet does not fit this actor: {message}"),
            Self::Storage(message) => format!("The sheet could not be stored: {message}"),
            Self::Database(message) => format!("The import failed: {message}"),
        }
    }

    pub fn sheet(code: &'static str, message: impl Into<String>) -> Self {
        Self::Sheet {
            code,
            message: message.into(),
        }
    }

    /// A refusal from a permission check, keeping its code when it has one.
    pub fn from_permission(error: Error) -> Self {
        let message = error.message.clone();
        let code = error
            .extensions
            .as_ref()
            .and_then(|ext| ext.get("code"))
            .map(|code| code.to_string().trim_matches('"').to_string());
        match code {
            Some(code) if code != "FORBIDDEN" => Self::Refused { code, message },
            _ => Self::Forbidden(message),
        }
    }
}

impl std::fmt::Display for SheetImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code(), self.message())
    }
}

impl SheetImportError {
    /// The GraphQL error, with the code under `extensions.code`. Not a
    /// `From`: async-graphql already converts anything `Display`, and that
    /// would drop the code.
    pub fn into_graphql(self) -> Error {
        let code = self.code();
        Error::new(self.message()).extend_with(move |_, ext| ext.set("code", code.clone()))
    }
}

impl From<diesel::result::Error> for SheetImportError {
    fn from(error: diesel::result::Error) -> Self {
        Self::Database(error.to_string())
    }
}
