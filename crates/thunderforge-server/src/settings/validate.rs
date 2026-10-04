//! FR-004's refusal rules, and the reason they are a list rather than a
//! heuristic.
//!
//! The requirement is that setup refuses "a blank or an obvious placeholder".
//! A clever placeholder detector rejects somebody's real name; a reserved-TLD
//! rule and a list of the strings this product itself ships are both exactly
//! right and both explainable in the refusal message (research.md § R15).
//!
//! # A refusal never echoes the value
//!
//! Every message here says what is wrong and what would be right, and none of
//! them contains the submitted value. A rejected password echoed back into a
//! browser lands in that browser's form history, and the one value most likely
//! to be rejected is the one most worth not storing there.

use super::registry::{Kind, SettingDeclaration, Validator};

/// TLDs reserved by RFC 2606 and RFC 6761. Mail to one of these can never
/// arrive, which makes an address there indistinguishable from no address.
const RESERVED_TLDS: [&str; 4] = ["local", "example", "invalid", "test"];

/// The placeholders this product ships, refused by name.
///
/// `config/realm-defaults.json` ships the first, and the DMCA page shipped the
/// second. The single most likely way to publish a page naming nobody is to
/// accept a default unchanged, so accepting one is the case worth naming
/// explicitly rather than leaving to the reserved-TLD rule.
const SHIPPED_PLACEHOLDERS: [&str; 4] = [
    "stewards@thunderforge.local",
    "dmca@thunderforge.example",
    "copyright agent, thunderforge",
    "[configure via instance legal/compliance settings before launch]",
];

/// Checks one submitted value against a declaration.
///
/// `Ok(String)` is the value as it will be stored — trimmed, because a
/// trailing newline pasted from a terminal is not a different address.
pub fn validate(declaration: &SettingDeclaration, value: &str) -> Result<String, String> {
    let trimmed = trimmed_for(declaration, value);

    for rule in declaration.validators {
        check(*rule, declaration, &trimmed)?;
    }

    Ok(trimmed)
}

/// Prose keeps its interior shape; everything else is trimmed whole.
fn trimmed_for(declaration: &SettingDeclaration, value: &str) -> String {
    match declaration.kind {
        Kind::Prose => value.trim_end().to_string(),
        _ => value.trim().to_string(),
    }
}

fn check(rule: Validator, declaration: &SettingDeclaration, value: &str) -> Result<(), String> {
    match rule {
        Validator::NonEmptyAfterTrim => {
            if value.trim().is_empty() {
                return Err(format!(
                    "`{}` cannot be blank. {}",
                    declaration.key, declaration.what_to_set
                ));
            }
        }
        Validator::EmailSyntax => {
            if !looks_like_an_address(value) {
                return Err(format!(
                    "`{}` must be an email address of the form name@example.org.",
                    declaration.key
                ));
            }
        }
        Validator::NoReservedTld => {
            if let Some(tld) = reserved_tld_of(value) {
                return Err(format!(
                    "`{}` cannot use the reserved `.{tld}` domain suffix: mail sent there \
                     never arrives, so the address would name nobody reachable.",
                    declaration.key
                ));
            }
        }
        Validator::NotAShippedPlaceholder => {
            if is_a_shipped_placeholder(value) {
                return Err(format!(
                    "`{}` is still the placeholder this instance shipped with. {}",
                    declaration.key, declaration.what_to_set
                ));
            }
        }
        Validator::PortRange => {
            let port = value
                .parse::<u32>()
                .map_err(|_| format!("`{}` must be a whole number.", declaration.key))?;
            if !(1..=65535).contains(&port) {
                return Err(format!(
                    "`{}` must be a port between 1 and 65535.",
                    declaration.key
                ));
            }
        }
        Validator::BoolLike => {
            if parse_bool(value).is_none() {
                return Err(format!("`{}` must be true or false.", declaration.key));
            }
        }
        Validator::OneOf(allowed) => {
            if !allowed.contains(&value.to_ascii_lowercase().as_str()) {
                return Err(format!(
                    "`{}` must be one of: {}.",
                    declaration.key,
                    allowed.join(", ")
                ));
            }
        }
    }

    Ok(())
}

/// True when a value that is *set* would nonetheless be refused if it were
/// submitted now.
///
/// This is what makes research.md § D6 observable. `support_email` has shipped
/// as `stewards@thunderforge.local` since before this feature existed, and
/// FR-004 only runs at setup — which does not run again on an instance that
/// already exists. So an upgraded instance keeps a support address at a
/// reserved TLD, silently, unless something asks the question afterwards.
/// Readiness asks it.
pub fn placeholder_problem(declaration: &SettingDeclaration, value: &str) -> Option<String> {
    validate(declaration, value).err()
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// Deliberately not a full RFC 5322 parser.
///
/// The rules an address must pass to be *useful here* are: one `@`, something
/// either side of it, a dot in the domain, and no whitespace. A stricter
/// grammar rejects valid exotic addresses; a looser one accepts `hello`. This
/// is the line where the refusal message stays truthful.
fn looks_like_an_address(value: &str) -> bool {
    if value.chars().any(char::is_whitespace) {
        return false;
    }
    let mut parts = value.split('@');
    let (Some(local), Some(domain), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && domain.len() > 2
}

fn reserved_tld_of(value: &str) -> Option<&'static str> {
    let domain = value.rsplit('@').next()?.to_ascii_lowercase();
    RESERVED_TLDS
        .into_iter()
        .find(|tld| domain.ends_with(&format!(".{tld}")))
}

fn is_a_shipped_placeholder(value: &str) -> bool {
    let folded = value.trim().to_ascii_lowercase();
    SHIPPED_PLACEHOLDERS.contains(&folded.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::registry::declaration;

    fn notice_email() -> &'static SettingDeclaration {
        declaration("notice.contact_email").expect("declared")
    }

    #[test]
    fn a_real_address_is_accepted_and_trimmed() {
        assert_eq!(
            validate(notice_email(), "  notices@example.org\n").expect("accepted"),
            "notices@example.org"
        );
    }

    #[test]
    fn a_blank_value_is_refused() {
        assert!(validate(notice_email(), "   ").is_err());
    }

    /// RFC 2606/6761. An address here cannot receive a copyright notice, which
    /// is the entire purpose of this particular setting.
    #[test]
    fn a_reserved_tld_is_refused_and_the_message_says_which() {
        for address in [
            "a@thunderforge.local",
            "a@host.example",
            "a@host.invalid",
            "a@host.test",
        ] {
            let message = validate(notice_email(), address).expect_err("refused");
            assert!(
                message.contains("reserved"),
                "{address} refused without saying why: {message}"
            );
        }
    }

    /// The shipped defaults are named in the refusal list rather than left to
    /// the TLD rule, because accepting a default unchanged is the likeliest
    /// route to a page naming nobody.
    #[test]
    fn the_shipped_placeholders_are_refused_by_name() {
        let support = declaration("support_email").expect("declared");
        assert!(validate(support, "stewards@thunderforge.local").is_err());
        let name = declaration("notice.contact_name").expect("declared");
        assert!(validate(name, "Copyright Agent, ThunderForge").is_err());
    }

    /// FR-023 in the one place it is easiest to forget: a refusal is prose an
    /// operator reads back, and a browser remembers.
    #[test]
    fn a_refusal_never_echoes_the_submitted_value() {
        let secret = declaration("mail.password").expect("declared");
        let submitted = "hunter2-correct-horse";
        let message = validate(secret, "   ").expect_err("refused");
        assert!(!message.contains(submitted));

        for (key, bad) in [
            ("notice.contact_email", "not-an-address"),
            ("mail.port", "99999"),
            ("mail.security", "wide-open"),
            ("mail.enabled", "perhaps"),
            ("operator.name", "  "),
        ] {
            let d = declaration(key).expect("declared");
            let message = validate(d, bad).expect_err("refused");
            assert!(
                !message.contains(bad.trim()) || bad.trim().is_empty(),
                "`{key}` echoed the submitted value: {message}"
            );
        }
    }

    #[test]
    fn a_port_must_be_a_port() {
        let port = declaration("mail.port").expect("declared");
        assert_eq!(validate(port, "587").expect("accepted"), "587");
        assert!(validate(port, "0").is_err());
        assert!(validate(port, "70000").is_err());
        assert!(validate(port, "smtp").is_err());
    }

    #[test]
    fn an_enum_accepts_only_what_it_declares() {
        let security = declaration("mail.security").expect("declared");
        assert!(validate(security, "starttls").is_ok());
        assert!(validate(security, "implicit").is_ok());
        assert!(validate(security, "tls-ish").is_err());
    }

    /// Prose keeps its paragraphs. Trimming a multi-line clause whole would
    /// silently reflow an operator's legal text.
    #[test]
    fn prose_keeps_its_interior_shape() {
        let prose = declaration("legal.community_addendum").expect("declared");
        let written = "One rule.\n\nAnd another.\n";
        assert_eq!(
            validate(prose, written).expect("accepted"),
            "One rule.\n\nAnd another."
        );
    }

    /// § D6: a value that is set can still be a placeholder, and nothing asks
    /// FR-004's question again after setup unless readiness does.
    #[test]
    fn a_stored_placeholder_is_still_reported_as_a_problem() {
        let support = declaration("support_email").expect("declared");
        assert!(placeholder_problem(support, "stewards@thunderforge.local").is_some());
        assert!(placeholder_problem(support, "help@mail.example").is_some());
        assert!(placeholder_problem(support, "help@thunderforge.example").is_some());
        // `example.org` is an ordinary domain: the rule is about the reserved
        // *suffix*, not about the word appearing anywhere in the name.
        assert!(placeholder_problem(support, "help@example.org").is_none());
        assert!(placeholder_problem(support, "help@realdomain.org").is_none());
    }
}
