//! Reading a credential out of an exchange response, and deciding when to
//! replace it.
//!
//! Two jobs, both pure, and the second is the one that matters. FR-036d says
//! installation credentials are short-lived and must be refreshed rather than
//! stored beyond their lifetime — which sounds like a storage rule and is
//! actually an arithmetic one. The question every push asks is *is this token
//! going to still be valid when the request lands*, and the honest answer
//! needs a margin, because the token was fetched some time ago, the request
//! takes some time to arrive, and the two clocks involved are not the same
//! clock.
//!
//! [`needs_refresh`] is that question, written so it can be generated against.
//!
//! # The timestamp format, and why it is parsed here
//!
//! The exchange response reports expiry as an RFC 3339 instant
//! (`2016-07-11T22:14:10Z`), and this crate carries no date-time library —
//! see the crate documentation for why. So the conversion to
//! [`UnixSeconds`](crate::UnixSeconds) is done here, in about forty lines, and
//! it is a genuine trade: a hand-written date parser is code that could be
//! wrong, whereas `chrono` is code that is known to be right.
//!
//! It is taken because the input is not arbitrary. It is one host's
//! machine-generated UTC timestamp in a fixed shape, the failure mode is a
//! refused connection rather than a wrong one, and every branch of it is
//! property-tested against round-tripped values in `tests/`. Adding a
//! date-time crate and its time-zone database to every consumer of this crate,
//! to convert one field, is the larger cost.

use serde::Deserialize;

use crate::{RepoHostError, RepositoryCredential, UnixSeconds};

/// How long before expiry a credential is treated as needing replacement.
///
/// Five minutes. An installation credential lives an hour, so this spends
/// about eight percent of its life to buy the margin, and the cost of
/// refreshing slightly early is one extra HTTPS call while the cost of
/// refreshing slightly late is a synchronisation failure a Game Master has to
/// interpret. The asymmetry is the whole argument.
pub const DEFAULT_REFRESH_MARGIN_SECS: u64 = 300;

/// The exchange response, as the host writes it.
///
/// Unknown fields are ignored rather than rejected: hosts add their own
/// (`permissions`, `repository_selection`, `repositories`), and a strict
/// struct here would break every connection on the day one of them shipped a
/// new field.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct ExchangeResponse {
    pub token: String,
    /// RFC 3339, UTC. See the module documentation.
    pub expires_at: String,
}

/// Parse an exchange response into the host-neutral credential type.
///
/// Total: every byte sequence produces `Ok` or `Err`, never a panic. The body
/// arrives from someone else's server over TLS, so it is not attacker-chosen
/// in this flow, but it is still someone else's JSON — and a host returning an
/// HTML error page with a 200 status must produce an error, not a crash.
///
/// Note what the error messages do *not* contain: the token. A malformed
/// response is logged; a logged token is FR-035 violated. `serde`'s own
/// message names the offending field, not its value, and the empty-token case
/// has nothing to quote.
pub fn parse_exchange_response(body: &str) -> Result<RepositoryCredential, RepoHostError> {
    let parsed: ExchangeResponse =
        serde_json::from_str(body).map_err(|e| RepoHostError::MalformedResponse(e.to_string()))?;

    let expires_at = parse_rfc3339_utc(&parsed.expires_at)?;
    RepositoryCredential::new(parsed.token, expires_at)
}

/// Should this credential be replaced before it is used at `now`?
///
/// True when the credential expires within `margin_secs` of `now`, and — the
/// case that is easy to write wrongly — true whenever it has already expired,
/// however long ago.
///
/// Written as a single addition and comparison, with `saturating_add` rather
/// than `+`. The saturation is not defensive noise: a caller passing
/// `u64::MAX` as a margin ("never trust the cache") would otherwise overflow
/// and, in release mode, wrap to a small number that declares an expiring
/// credential fresh. Saturating turns the pathological input into the
/// pathological *intent* — always refresh — which is the safe direction.
pub fn needs_refresh(
    credential: &RepositoryCredential,
    now: UnixSeconds,
    margin_secs: u64,
) -> bool {
    credential.expires_at() <= now.saturating_add(margin_secs)
}

/// How long a credential remains usable at `now`, allowing for the margin.
///
/// Zero exactly when [`needs_refresh`] is true, which is the invariant worth
/// stating: a caller that schedules the next refresh from this value and a
/// caller that asks the predicate directly must never disagree.
pub fn remaining_useful_secs(
    credential: &RepositoryCredential,
    now: UnixSeconds,
    margin_secs: u64,
) -> u64 {
    credential
        .expires_at()
        .saturating_sub(now.saturating_add(margin_secs))
}

/// Parse an RFC 3339 timestamp into seconds since the Unix epoch.
///
/// Accepts `YYYY-MM-DDTHH:MM:SS`, an optional fractional part (discarded —
/// this crate's resolution is whole seconds), and either `Z` or a numeric
/// `±HH:MM` offset. Case-insensitive on the `T` and `Z` separators, as
/// RFC 3339 §5.6 permits.
///
/// Instants before the epoch are rejected rather than represented. `u64` is
/// the crate's chosen shape (see the crate documentation) and a credential
/// that expired in 1969 is a malformed response, not a value worth carrying.
pub fn parse_rfc3339_utc(value: &str) -> Result<UnixSeconds, RepoHostError> {
    let bad = |reason: &'static str| RepoHostError::UnreadableExpiry {
        value: value.to_string(),
        reason,
    };

    let bytes = value.as_bytes();
    if bytes.len() < 19 {
        return Err(bad("too short to be an RFC 3339 instant"));
    }

    let digits = |from: usize, to: usize| -> Option<u64> {
        let slice = value.get(from..to)?;
        if slice.bytes().all(|b| b.is_ascii_digit()) {
            slice.parse().ok()
        } else {
            None
        }
    };

    if bytes[4] != b'-' || bytes[7] != b'-' || !bytes[10].eq_ignore_ascii_case(&b'T') {
        return Err(bad("date and time are not separated as RFC 3339 requires"));
    }
    if bytes[13] != b':' || bytes[16] != b':' {
        return Err(bad("time fields are not separated by colons"));
    }

    let year = digits(0, 4).ok_or_else(|| bad("year is not four digits"))?;
    let month = digits(5, 7).ok_or_else(|| bad("month is not two digits"))?;
    let day = digits(8, 10).ok_or_else(|| bad("day is not two digits"))?;
    let hour = digits(11, 13).ok_or_else(|| bad("hour is not two digits"))?;
    let minute = digits(14, 16).ok_or_else(|| bad("minute is not two digits"))?;
    let second = digits(17, 19).ok_or_else(|| bad("second is not two digits"))?;

    if year < 1970 {
        return Err(bad("instant is before the Unix epoch"));
    }
    if !(1..=12).contains(&month) {
        return Err(bad("month is out of range"));
    }
    if !(1..=days_in_month(year, month)).contains(&day) {
        return Err(bad("day is out of range for that month"));
    }
    // 60 is admitted because RFC 3339 §5.7 allows a leap second, and refusing
    // one would mean a connection that fails on exactly the days a leap second
    // is inserted — the least debuggable failure imaginable.
    if hour > 23 || minute > 59 || second > 60 {
        return Err(bad("time of day is out of range"));
    }

    let mut rest = &value[19..];

    // An optional fractional second, discarded. Sub-second precision on a
    // credential expiry an hour away is noise, and rounding it either way
    // would be a decision with no defensible answer.
    if let Some(stripped) = rest.strip_prefix('.') {
        let fraction_len = stripped.bytes().take_while(u8::is_ascii_digit).count();
        if fraction_len == 0 {
            return Err(bad("fractional second marker with no digits"));
        }
        rest = &stripped[fraction_len..];
    }

    // The offset. Applied by subtraction: an instant written in +02:00 is two
    // hours *earlier* in UTC than its digits read.
    let offset_secs: i64 = if rest.eq_ignore_ascii_case("Z") {
        0
    } else if rest.len() == 6 && (rest.starts_with('+') || rest.starts_with('-')) {
        let sign: i64 = if rest.starts_with('-') { -1 } else { 1 };
        let ob = rest.as_bytes();
        if ob[3] != b':' {
            return Err(bad("offset is not written as ±HH:MM"));
        }
        let oh: i64 = rest[1..3]
            .parse()
            .map_err(|_| bad("offset hours are not two digits"))?;
        let om: i64 = rest[4..6]
            .parse()
            .map_err(|_| bad("offset minutes are not two digits"))?;
        if oh > 23 || om > 59 {
            return Err(bad("offset is out of range"));
        }
        sign * (oh * 3600 + om * 60)
    } else {
        return Err(bad("no UTC designator or numeric offset"));
    };

    let days = days_from_civil(year, month, day);
    let local = days * 86_400 + hour * 3600 + minute * 60 + second;

    let utc = i128::from(local) - i128::from(offset_secs);
    if utc < 0 {
        return Err(bad(
            "instant is before the Unix epoch once the offset is applied",
        ));
    }
    u64::try_from(utc).map_err(|_| bad("instant is too far in the future to represent"))
}

/// Days from 1970-01-01 to `year-month-day`, by Howard Hinnant's civil-date
/// algorithm.
///
/// Chosen over a loop-over-the-years because it is branch-free, exact for
/// every proleptic Gregorian date, and short enough to read in one sitting —
/// which is the bar a hand-written date routine has to clear to be worth
/// having instead of a dependency. Restricted to `year >= 1970`, which the
/// caller has already checked.
fn days_from_civil(year: u64, month: u64, day: u64) -> u64 {
    // Shift the year so that March is month 1: the leap day then lands at the
    // end of the year, and the day-of-year becomes a single linear formula.
    let y = if month <= 2 { year - 1 } else { year };
    let era = (y - 1200) / 400; // 1200 keeps every intermediate non-negative.
    let yoe = y - 1200 - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    // Days from 1200-03-01 to 1970-01-01, the offset that re-bases the result
    // on the Unix epoch.
    const EPOCH_SHIFT: u64 = 281_177;
    era * 146_097 + doe - EPOCH_SHIFT
}

fn days_in_month(year: u64, month: u64) -> u64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn is_leap(year: u64) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}
