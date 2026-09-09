//! A coarse, human-recognisable name for the client that opened a session.
//!
//! Spec 036 FR-005 asks that a session be recognisable by its owner — enough
//! for "end the one I don't recognise" to be a decision somebody can actually
//! make. Spec 035's rule is the other half of it: record the act, never the
//! person. So what is stored is a *family* and a *platform* — "Firefox on
//! Linux" — and nothing else. No version, no address, no device name, no
//! locale, and never the raw `User-Agent`.
//!
//! Two properties are deliberate and are pinned by the tests below:
//!
//! - The output is drawn from a **fixed vocabulary**. It is assembled from the
//!   constants in this file, never from a slice of the input, so a header
//!   crafted to smuggle text into somebody's session list has nothing to
//!   smuggle it through.
//! - A header that matches nothing yields `None`, not a guess. The session
//!   list already renders an unrecognised client honestly; inventing a name
//!   for one would make "I don't recognise this" harder to act on, which is
//!   the whole requirement.

/// Browser families, in match order. Order is the entire correctness of this
/// table: every Chromium browser also says "Chrome", and Chrome itself says
/// "Safari", so the more specific token has to be tried first.
const FAMILIES: &[(&str, &str)] = &[
    ("edg/", "Edge"),
    ("opr/", "Opera"),
    ("vivaldi", "Vivaldi"),
    ("brave", "Brave"),
    ("firefox", "Firefox"),
    ("fxios", "Firefox"),
    ("crios", "Chrome"),
    ("chrome", "Chrome"),
    ("chromium", "Chrome"),
    ("safari", "Safari"),
];

/// Platforms, in match order. `android` precedes `linux` because every
/// Android `User-Agent` also says "Linux", and the phone is the useful answer.
const PLATFORMS: &[(&str, &str)] = &[
    ("android", "Android"),
    ("iphone", "iOS"),
    ("ipad", "iPadOS"),
    ("windows", "Windows"),
    ("mac os x", "macOS"),
    ("macintosh", "macOS"),
    ("cros", "ChromeOS"),
    ("linux", "Linux"),
];

/// Describe a client from its `User-Agent`, or `None` if it is not recognised.
///
/// A family with no platform still describes something useful ("Firefox"), so
/// it is returned. A platform with no family is not — "Linux" tells its owner
/// nothing they could act on, and most such headers are scripts rather than
/// browsers.
pub fn describe_client(user_agent: &str) -> Option<String> {
    let haystack = user_agent.to_ascii_lowercase();

    let family = FAMILIES
        .iter()
        .find(|(token, _)| haystack.contains(token))
        .map(|(_, name)| *name)?;

    let platform = PLATFORMS
        .iter()
        .find(|(token, _)| haystack.contains(token))
        .map(|(_, name)| *name);

    Some(match platform {
        Some(platform) => format!("{family} on {platform}"),
        None => family.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_the_common_browsers_and_platforms() {
        let cases = [
            (
                "Mozilla/5.0 (X11; Linux x86_64; rv:129.0) Gecko/20100101 Firefox/129.0",
                "Firefox on Linux",
            ),
            (
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/127.0.0.0 Safari/537.36",
                "Chrome on Windows",
            ),
            (
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Safari/605.1.15",
                "Safari on macOS",
            ),
            (
                "Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1",
                "Safari on iOS",
            ),
            (
                "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/127.0.0.0 Mobile Safari/537.36",
                "Chrome on Android",
            ),
            (
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/127.0.0.0 Safari/537.36 Edg/127.0.0.0",
                "Edge on Windows",
            ),
        ];

        for (agent, expected) in cases {
            assert_eq!(describe_client(agent).as_deref(), Some(expected));
        }
    }

    /// Chromium browsers all claim to be Chrome, and Chrome claims to be
    /// Safari. If the match order in `FAMILIES` is ever sorted or reordered
    /// alphabetically this is the test that notices.
    #[test]
    fn the_more_specific_family_wins() {
        let edge =
            "Mozilla/5.0 (Windows NT 10.0) AppleWebKit/537.36 Chrome/127.0 Safari/537.36 Edg/127.0";
        assert_eq!(describe_client(edge).as_deref(), Some("Edge on Windows"));

        let chrome = "Mozilla/5.0 (Windows NT 10.0) AppleWebKit/537.36 Chrome/127.0 Safari/537.36";
        assert_eq!(
            describe_client(chrome).as_deref(),
            Some("Chrome on Windows")
        );
    }

    /// Spec 035: record the act, never the person. The description is built
    /// from this file's constants, so nothing a client sends can reach a
    /// session list — not a version, not an address, not a device name.
    #[test]
    fn describes_the_client_and_never_the_person() {
        let agent = "Mozilla/5.0 (X11; Linux x86_64; rv:129.0) Gecko/20100101 Firefox/129.0 \
             10.1.2.3 michael@example.org Michaels-Laptop en-GB";
        let described = describe_client(agent).expect("a Firefox header is recognised");

        for leak in ["10.1.2.3", "@", "Michaels-Laptop", "en-GB", "129", "5.0"] {
            assert!(
                !described.contains(leak),
                "{described:?} carried {leak:?} out of the User-Agent",
            );
        }
        assert_eq!(described, "Firefox on Linux");
    }

    #[test]
    fn an_unrecognised_client_is_not_guessed_at() {
        for agent in ["", "curl/8.8.0", "Mozilla/5.0 (X11; Linux x86_64)", "   "] {
            assert_eq!(
                describe_client(agent),
                None,
                "{agent:?} should stay unrecognised rather than be named",
            );
        }
    }

    /// Every value this function can produce, produced. A description that is
    /// not in this set means the vocabulary leaked.
    #[test]
    fn the_vocabulary_is_closed() {
        let permitted: Vec<String> = FAMILIES
            .iter()
            .flat_map(|(_, family)| {
                std::iter::once((*family).to_string()).chain(
                    PLATFORMS
                        .iter()
                        .map(move |(_, platform)| format!("{family} on {platform}")),
                )
            })
            .collect();

        for agent in [
            "Mozilla/5.0 (X11; Linux x86_64) Firefox/129.0",
            "Mozilla/5.0 (Linux; Android 14) Chrome/127.0 Mobile Safari/537.36",
            "Mozilla/5.0 (iPad; CPU OS 17_5 like Mac OS X) Version/17.5 Safari/604.1",
            "Vivaldi/6.8 anything at all",
        ] {
            let described = describe_client(agent).expect("recognised");
            assert!(
                permitted.contains(&described),
                "{described:?} is outside the fixed vocabulary",
            );
        }
    }
}
