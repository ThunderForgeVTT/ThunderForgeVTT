//! Turning a provider's error redirect into something a human can act on.
//!
//! RFC 6749 §4.1.2.1 says the provider redirects back with `error` and
//! optionally `error_description` instead of a `code`. Both are attacker-
//! influenceable in the sense that anyone can craft that redirect, so
//! neither is trusted for anything but display.

/// The `error`/`error_description` pair from a failed callback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderError {
    pub code: String,
    pub description: Option<String>,
}

impl ProviderError {
    /// The message shown to whoever was trying to log in.
    ///
    /// The provider's own code is included verbatim because it is the only
    /// thing an operator can search their identity provider's logs for; a
    /// friendly rewrite here would erase the one useful fact.
    pub fn message(&self) -> String {
        format!(
            "Provider returned error '{}': {}",
            self.code,
            self.description.as_deref().unwrap_or("unknown")
        )
    }
}

/// Was this callback an error redirect?
///
/// A callback with an `error` present is a failure **even if it also carries
/// a `code`**: a provider that sends both is malfunctioning, and redeeming
/// the code anyway would mean completing a login the provider just refused.
pub fn provider_error_from_callback(
    error: Option<String>,
    error_description: Option<String>,
) -> Option<ProviderError> {
    error.map(|code| ProviderError {
        code,
        description: error_description,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn a_missing_description_reads_as_unknown() {
        let err = provider_error_from_callback(Some("access_denied".to_string()), None)
            .expect("an error code makes this a failure");
        assert_eq!(err.message(), "Provider returned error 'access_denied': unknown");
    }

    #[test]
    fn a_callback_with_no_error_is_not_an_error() {
        assert_eq!(provider_error_from_callback(None, Some("ignored".into())), None);
    }

    proptest! {
        /// Any `error` at all means failure, whatever else is present.
        #[test]
        fn any_error_code_produces_a_failure(
            code in ".{0,32}",
            description in proptest::option::of(".{0,32}"),
        ) {
            let err = provider_error_from_callback(Some(code.clone()), description)
                .expect("presence of `error` is the whole test");
            prop_assert!(err.message().contains(&code));
        }

        /// Message formatting never panics on provider-supplied text.
        #[test]
        fn message_formatting_is_total(
            code: String,
            description in proptest::option::of(".{0,64}"),
        ) {
            let _ = ProviderError { code, description }.message();
        }
    }
}
