//! Terms-of-service disputes and privacy requests: intake, and the operator's
//! queue of them.
//!
//! # Why this exists beside the DMCA path rather than inside it
//!
//! `graphql/mutations_moderation.rs` handles takedown notices, and it is
//! shaped by the statute: it demands a described work, a good-faith statement,
//! an accuracy statement and a signature, and on a valid notice it **disables
//! the targeted content**. None of that belongs to somebody who thinks a
//! clause in the terms is unfair, or who wants to know what data an instance
//! holds about them. Reusing that path would mean either a takedown that can
//! be filed without the elements that make it lawful, or eight columns that
//! are always empty. The operator's queue shows all three kinds together,
//! which is the part that wanted unifying.
//!
//! # Why intake does not require an account
//!
//! A person objecting to the terms of service is quite often objecting to the
//! terms they were asked to accept, and a complaints channel reachable only
//! after accepting them is not one. The account is recorded when there is one
//! and the enquiry is accepted when there is not.
//!
//! # Why it is rate limited
//!
//! It is an unauthenticated write that stores text. Without a limiter it is a
//! spam target with a nice form on it. The limiter is the one spec 035 already
//! uses for anonymous share reads, keyed on the caller the public transport
//! derives, so this product has one idea of what rate limiting looks like.

pub mod graphql;

/// What an enquiry is about. Takedowns are deliberately not a variant: they
/// have their own table because they have their own law.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnquiryKind {
    Terms,
    Privacy,
}

impl EnquiryKind {
    pub fn as_db_str(self) -> &'static str {
        match self {
            EnquiryKind::Terms => "terms",
            EnquiryKind::Privacy => "privacy",
        }
    }

    pub fn parse(value: &str) -> Option<EnquiryKind> {
        match value {
            "terms" => Some(EnquiryKind::Terms),
            "privacy" => Some(EnquiryKind::Privacy),
            _ => None,
        }
    }
}

/// Where an enquiry has got to. Three states, because a queue an operator
/// works needs "I have seen this" to be distinct from "this is finished" —
/// a privacy request in particular may be acknowledged long before it is
/// answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnquiryStatus {
    Open,
    Acknowledged,
    Closed,
}

impl EnquiryStatus {
    pub fn as_db_str(self) -> &'static str {
        match self {
            EnquiryStatus::Open => "open",
            EnquiryStatus::Acknowledged => "acknowledged",
            EnquiryStatus::Closed => "closed",
        }
    }

    pub fn parse(value: &str) -> Option<EnquiryStatus> {
        match value {
            "open" => Some(EnquiryStatus::Open),
            "acknowledged" => Some(EnquiryStatus::Acknowledged),
            "closed" => Some(EnquiryStatus::Closed),
            _ => None,
        }
    }
}

/// The longest each field may be.
///
/// Not a validation nicety: this is an unauthenticated write, and a field with
/// no ceiling is a way to fill somebody's disk through a contact form. The
/// body is generous because a privacy request explaining what somebody wants
/// and why is legitimately long.
pub const MAX_NAME: usize = 200;
pub const MAX_CONTACT: usize = 320;
pub const MAX_SUBJECT: usize = 300;
pub const MAX_BODY: usize = 20_000;

/// What is wrong with a submission, in the words the form should show.
///
/// Every message names the field and says what is needed. None of them repeat
/// what was submitted — the same rule the settings validators follow, for the
/// same reason: a refusal that echoes the input is a refusal that can be made
/// to echo anything.
pub fn validate(name: &str, contact: &str, subject: &str, body: &str) -> Vec<String> {
    let mut problems = Vec::new();

    if name.trim().is_empty() {
        problems.push("Give a name we can address a reply to.".to_string());
    } else if name.chars().count() > MAX_NAME {
        problems.push(format!("The name must be {MAX_NAME} characters or fewer."));
    }

    let contact = contact.trim();
    if contact.is_empty() {
        problems.push("Give an email address so this can be answered.".to_string());
    } else if contact.chars().count() > MAX_CONTACT {
        problems.push(format!(
            "The contact address must be {MAX_CONTACT} characters or fewer."
        ));
    } else if !is_an_address(contact) {
        problems.push(
            "Give an email address of the form name@example.org so this can be answered."
                .to_string(),
        );
    }

    if subject.trim().is_empty() {
        problems.push("Say what this is about in one line.".to_string());
    } else if subject.chars().count() > MAX_SUBJECT {
        problems.push(format!(
            "The subject must be {MAX_SUBJECT} characters or fewer."
        ));
    }

    if body.trim().is_empty() {
        problems.push("Describe what you are asking for.".to_string());
    } else if body.chars().count() > MAX_BODY {
        problems.push(format!(
            "The message must be {MAX_BODY} characters or fewer."
        ));
    }

    problems
}

/// The same shape check `sendTestMail` uses, and deliberately no cleverer:
/// research.md § R15's finding was that a stricter address validator rejects
/// somebody's real address, and the cost of accepting one that bounces is that
/// a reply bounces.
fn is_an_address(value: &str) -> bool {
    let mut parts = value.split('@');
    let (Some(local), Some(domain), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_complete_submission_is_accepted() {
        assert!(validate("A Person", "person@example.org", "A subject", "A body").is_empty());
    }

    #[test]
    fn every_missing_field_is_named_rather_than_the_first_one() {
        // The failure this guards: a validator that returns on the first
        // problem makes somebody fix a four-field form one round trip at a
        // time.
        let problems = validate("", "", "", "");
        assert_eq!(problems.len(), 4, "{problems:?}");
    }

    #[test]
    fn an_address_that_could_not_receive_a_reply_is_refused() {
        let problems = validate("A Person", "not-an-address", "A subject", "A body");
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("email address"), "{problems:?}");
    }

    #[test]
    fn a_refusal_never_repeats_what_was_submitted() {
        // An unauthenticated endpoint whose error messages echo the input is a
        // reflection gadget. Asserted over every field at once.
        let planted = "PLANTED-9f2ab7c4";
        let problems = validate(planted, planted, planted, "");
        for problem in &problems {
            assert!(
                !problem.contains(planted),
                "a refusal repeated the submission: {problem}"
            );
        }
    }

    #[test]
    fn an_oversized_field_is_refused_rather_than_stored() {
        let long = "x".repeat(MAX_BODY + 1);
        let problems = validate("A Person", "person@example.org", "A subject", &long);
        assert_eq!(problems.len(), 1, "{problems:?}");
    }

    #[test]
    fn the_kinds_round_trip_and_a_takedown_is_not_one_of_them() {
        for kind in [EnquiryKind::Terms, EnquiryKind::Privacy] {
            assert_eq!(EnquiryKind::parse(kind.as_db_str()), Some(kind));
        }
        // Takedowns have their own table and their own law; accepting the word
        // here would be a way to file one without its statutory elements.
        assert_eq!(EnquiryKind::parse("takedown"), None);
    }

    #[test]
    fn the_statuses_round_trip() {
        for status in [
            EnquiryStatus::Open,
            EnquiryStatus::Acknowledged,
            EnquiryStatus::Closed,
        ] {
            assert_eq!(EnquiryStatus::parse(status.as_db_str()), Some(status));
        }
    }
}
