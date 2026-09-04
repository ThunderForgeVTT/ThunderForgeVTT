//! The refresh decision, and the expiry timestamps it reads.
//!
//! FR-036d — "credentials … MUST be refreshed rather than stored beyond their
//! lifetime" — reads like a storage rule and is enforced by arithmetic. These
//! tests exercise that arithmetic at the boundaries where it is easy to get
//! wrong: the instant of expiry itself, an already-lapsed credential, a margin
//! wider than the credential's whole life, and values near the top of the
//! range where an unguarded addition would wrap.
//!
//! Every test here runs with no network and no application configured, which
//! is the property the crate exists to have.

use proptest::prelude::*;
use thunderforge_repo_host::token::{
    DEFAULT_REFRESH_MARGIN_SECS, needs_refresh, parse_exchange_response, parse_rfc3339_utc,
    remaining_useful_secs,
};
use thunderforge_repo_host::{RepoHostError, RepositoryCredential};

fn credential(expires_at: u64) -> RepositoryCredential {
    RepositoryCredential::new("ghs_example", expires_at).expect("a non-empty token is valid")
}

#[test]
fn a_fresh_credential_is_not_refreshed() {
    let cred = credential(10_000);
    assert!(!needs_refresh(&cred, 1_000, DEFAULT_REFRESH_MARGIN_SECS));
}

#[test]
fn the_boundary_is_inclusive() {
    // Expiry lands exactly on the margin. Refreshing here is the safe
    // direction: a credential that expires the instant the request arrives is
    // a credential that fails, and "exactly on the boundary" is precisely the
    // case an off-by-one would get wrong in the unsafe direction.
    let cred = credential(1_000 + DEFAULT_REFRESH_MARGIN_SECS);
    assert!(needs_refresh(&cred, 1_000, DEFAULT_REFRESH_MARGIN_SECS));
}

#[test]
fn one_second_outside_the_margin_is_not_refreshed() {
    let cred = credential(1_001 + DEFAULT_REFRESH_MARGIN_SECS);
    assert!(!needs_refresh(&cred, 1_000, DEFAULT_REFRESH_MARGIN_SECS));
}

#[test]
fn an_expired_credential_is_always_refreshed_even_with_no_margin() {
    let cred = credential(500);
    assert!(needs_refresh(&cred, 501, 0));
    assert!(cred.is_expired_at(501));
}

#[test]
fn a_margin_at_the_top_of_the_range_saturates_rather_than_wrapping() {
    // The failure this guards: `now + margin` overflowing and wrapping to a
    // small number, which in a release build would silently declare an
    // expiring credential fresh. Saturating turns the pathological input into
    // the pathological intent — always refresh.
    let cred = credential(u64::MAX);
    assert!(needs_refresh(&cred, 1_000, u64::MAX));
    assert!(needs_refresh(&cred, u64::MAX, DEFAULT_REFRESH_MARGIN_SECS));
}

#[test]
fn a_debug_rendering_never_contains_the_token() {
    // FR-035: a credential must not appear in a log, and `Debug` is how one
    // gets there. Asserted rather than trusted, because a future `derive`
    // would undo it silently.
    let rendered = format!("{:?}", credential(42));
    assert!(!rendered.contains("ghs_example"));
    assert!(rendered.contains("<redacted>"));
}

#[test]
fn an_empty_token_is_refused_at_construction() {
    assert_eq!(
        RepositoryCredential::new("", 1_000).unwrap_err(),
        RepoHostError::EmptyCredential
    );
}

#[test]
fn a_typical_exchange_response_parses() {
    let cred = parse_exchange_response(
        r#"{"token":"ghs_16C7e42F292c6912E7710c838347Ae178B4a",
            "expires_at":"2016-07-11T22:14:10Z",
            "permissions":{"contents":"write","issues":"write"},
            "repository_selection":"selected"}"#,
    )
    .expect("a well-formed response parses");

    assert_eq!(cred.token(), "ghs_16C7e42F292c6912E7710c838347Ae178B4a");
    assert_eq!(cred.expires_at(), 1_468_275_250);
}

#[test]
fn an_html_error_page_is_an_error_and_not_a_panic() {
    // A host returning HTML with a 200 must not take the process down.
    assert!(matches!(
        parse_exchange_response("<html><body>502 Bad Gateway</body></html>"),
        Err(RepoHostError::MalformedResponse(_))
    ));
}

#[test]
fn an_exchange_response_with_an_empty_token_is_refused() {
    assert_eq!(
        parse_exchange_response(r#"{"token":"","expires_at":"2030-01-01T00:00:00Z"}"#).unwrap_err(),
        RepoHostError::EmptyCredential
    );
}

#[test]
fn known_instants_convert_correctly() {
    // Anchors checked against `date -u -d '<value>' +%s`, not against this
    // implementation — a hand-written date routine tested only against itself
    // proves nothing.
    assert_eq!(parse_rfc3339_utc("1970-01-01T00:00:00Z").unwrap(), 0);
    assert_eq!(
        parse_rfc3339_utc("2016-07-11T22:14:10Z").unwrap(),
        1_468_275_250
    );
    assert_eq!(
        parse_rfc3339_utc("2000-02-29T12:00:00Z").unwrap(),
        951_825_600
    );
    assert_eq!(
        parse_rfc3339_utc("2100-03-01T00:00:00Z").unwrap(),
        4_107_542_400
    );
}

#[test]
fn offsets_and_fractions_and_case_are_handled() {
    let utc = parse_rfc3339_utc("2024-05-01T12:00:00Z").unwrap();
    assert_eq!(parse_rfc3339_utc("2024-05-01T14:00:00+02:00").unwrap(), utc);
    assert_eq!(parse_rfc3339_utc("2024-05-01T09:30:00-02:30").unwrap(), utc);
    assert_eq!(
        parse_rfc3339_utc("2024-05-01T12:00:00.123456Z").unwrap(),
        utc
    );
    assert_eq!(parse_rfc3339_utc("2024-05-01t12:00:00z").unwrap(), utc);
}

#[test]
fn impossible_dates_are_refused() {
    for value in [
        "2023-02-29T00:00:00Z",     // not a leap year
        "2023-13-01T00:00:00Z",     // month 13
        "2023-04-31T00:00:00Z",     // April has 30 days
        "2023-01-01T24:00:00Z",     // hour 24
        "2023-01-01T00:60:00Z",     // minute 60
        "1969-12-31T23:59:59Z",     // before the epoch
        "2023-01-01T00:00:00",      // no designator
        "2023-01-01T00:00:00+0200", // offset not ±HH:MM
        "2023-01-01T00:00:00.Z",    // fraction marker with no digits
        "not a timestamp",
        "",
    ] {
        assert!(
            matches!(
                parse_rfc3339_utc(value),
                Err(RepoHostError::UnreadableExpiry { .. })
            ),
            "expected {value:?} to be refused"
        );
    }
}

#[test]
fn a_leap_second_is_admitted() {
    // Refusing :60 would mean connections that fail on exactly the days a
    // leap second is inserted — the least debuggable failure imaginable.
    assert!(parse_rfc3339_utc("2016-12-31T23:59:60Z").is_ok());
}

proptest! {
    /// The predicate and the countdown can never disagree: remaining time is
    /// zero exactly when a refresh is due. A caller that schedules from one
    /// and a caller that asks the other must reach the same conclusion.
    #[test]
    fn remaining_time_is_zero_exactly_when_a_refresh_is_due(
        expires_at: u64,
        now: u64,
        margin in 0u64..u64::MAX,
    ) {
        let cred = credential(expires_at.max(1));
        prop_assert_eq!(
            needs_refresh(&cred, now, margin),
            remaining_useful_secs(&cred, now, margin) == 0
        );
    }

    /// A wider margin never makes a credential look fresher. Monotonicity is
    /// the property a "safety margin" has to have to deserve the name.
    #[test]
    fn a_wider_margin_never_reduces_the_need_to_refresh(
        expires_at: u64,
        now: u64,
        smaller in 0u64..1_000_000,
        extra in 0u64..1_000_000,
    ) {
        let cred = credential(expires_at.max(1));
        if needs_refresh(&cred, now, smaller) {
            prop_assert!(needs_refresh(&cred, now, smaller.saturating_add(extra)));
        }
    }

    /// Time only moves forward, and so does the need to refresh. A later
    /// clock can never turn a stale credential fresh again.
    #[test]
    fn a_later_clock_never_reduces_the_need_to_refresh(
        expires_at: u64,
        now in 0u64..u64::MAX / 2,
        elapsed in 0u64..u64::MAX / 2,
        margin in 0u64..3_600,
    ) {
        let cred = credential(expires_at.max(1));
        if needs_refresh(&cred, now, margin) {
            prop_assert!(needs_refresh(&cred, now + elapsed, margin));
        }
    }

    /// An already-expired credential is due for refresh at every margin,
    /// including none.
    #[test]
    fn an_expired_credential_always_needs_refreshing(
        expires_at in 1u64..u64::MAX,
        overshoot in 0u64..1_000_000,
        margin in 0u64..1_000_000,
    ) {
        let cred = credential(expires_at);
        let now = expires_at.saturating_add(overshoot);
        prop_assert!(needs_refresh(&cred, now, margin));
    }

    /// The predicate is total: no combination of inputs panics, which is the
    /// property `u64` arithmetic in a release build does not give for free.
    #[test]
    fn the_refresh_decision_never_panics(expires_at: u64, now: u64, margin: u64) {
        let cred = credential(expires_at.max(1));
        let _ = needs_refresh(&cred, now, margin);
        let _ = remaining_useful_secs(&cred, now, margin);
    }

    /// Timestamp parsing is total over arbitrary text. The value comes from
    /// someone else's server; every byte sequence must produce `Ok` or `Err`.
    #[test]
    fn timestamp_parsing_never_panics(value in ".{0,64}") {
        let _ = parse_rfc3339_utc(&value);
    }

    /// Round-trip: every second of a wide range, rendered as RFC 3339 by
    /// independent arithmetic and read back, returns the value it started as.
    /// This is what makes a hand-written date routine defensible instead of
    /// merely short — it is checked against a second implementation rather
    /// than against itself.
    #[test]
    fn every_instant_round_trips(seconds in 0u64..4_102_444_800u64) {
        let rendered = render_rfc3339(seconds);
        prop_assert_eq!(
            parse_rfc3339_utc(&rendered).map_err(|e| e.to_string()),
            Ok(seconds),
            "rendered as {}", rendered
        );
    }

    /// Any well-formed instant plus an offset reads as the same UTC instant
    /// the offset says it is.
    #[test]
    fn an_offset_shifts_the_instant_it_claims_to(
        seconds in 86_400u64..4_102_444_800u64,
        offset_hours in 0i64..24,
        offset_minutes in 0i64..60,
    ) {
        let shift = offset_hours * 3600 + offset_minutes * 60;
        let local = u64::try_from(i64::try_from(seconds).unwrap() + shift).unwrap();
        let rendered = render_rfc3339(local);
        let with_offset = format!(
            "{}+{offset_hours:02}:{offset_minutes:02}",
            rendered.trim_end_matches('Z')
        );
        prop_assert_eq!(parse_rfc3339_utc(&with_offset), Ok(seconds));
    }
}

/// Render seconds-since-epoch as `YYYY-MM-DDTHH:MM:SSZ`.
///
/// Written from the opposite direction to the parser — day counting forward
/// from 1970 rather than Hinnant's closed form — deliberately. A round-trip
/// test against a mirror image of the code under test would pass on a shared
/// mistake; this one cannot.
fn render_rfc3339(seconds: u64) -> String {
    let (mut days, rem) = (seconds / 86_400, seconds % 86_400);
    let (hour, minute, second) = (rem / 3600, (rem % 3600) / 60, rem % 60);

    let leap = |y: u64| y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400));
    let mut year = 1970;
    loop {
        let len = if leap(year) { 366 } else { 365 };
        if days < len {
            break;
        }
        days -= len;
        year += 1;
    }

    let lengths = [
        31,
        if leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1;
    for len in lengths {
        if days < len {
            break;
        }
        days -= len;
        month += 1;
    }

    format!(
        "{year:04}-{month:02}-{:02}T{hour:02}:{minute:02}:{second:02}Z",
        days + 1
    )
}
