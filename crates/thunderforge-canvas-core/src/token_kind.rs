//! What a token *is*, and how it reads at a glance.
//!
//! # Why a type at all
//!
//! Every token on a scene without a portrait rendered in the same blue. A
//! player character, a hostile ogre, the cart they are all trying to escort
//! and a barrel somebody might set on fire were visually identical, and the
//! only way to tell them apart was to click one. On a crowded battle map that
//! is the difference between reading the board and interrogating it.
//!
//! # Why the appearance lives beside the type
//!
//! The engine's `Token` component has carried a `token_type: String` field —
//! documented as "character, npc, object, etc." — while the database had no
//! such column and the renderer ignored it. A declared type nothing stored
//! and nothing drew.
//!
//! Putting the kind and its appearance together means a new kind cannot be
//! added without deciding how it looks: [`TokenKind::fill`] matches on every
//! variant with no catch-all, so the compiler asks. That is the only
//! mechanism here that actually prevents a kind quietly inheriting somebody
//! else's colour.
//!
//! # Why the palette is tested rather than eyeballed
//!
//! Distinguishable is a measurable property, not a matter of taste, and it is
//! easy to pick four colours that look distinct to the person who chose them
//! and collapse into two for the roughly one in twelve men with a red-green
//! deficiency. [`tests::every_pair_of_kinds_is_distinguishable`] asserts a
//! minimum separation in perceived lightness as well as hue, so a future
//! palette change cannot quietly make two kinds look alike.

/// What a token represents on the board.
///
/// Deliberately small. These are the distinctions a player needs to make
/// *without clicking* — friend, threat, thing that moves, thing that does
/// not. Finer classification belongs to the actor behind the token, not to
/// its silhouette on a map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TokenKind {
    /// A player's character.
    #[default]
    Character,
    /// Anyone the Game Master runs.
    Npc,
    /// A mount, cart, ship — something that carries others and moves.
    Vehicle,
    /// Scenery a player can interact with: a barrel, a lever, a corpse.
    Object,
}

/// A colour, as the renderer wants it: linear sRGB, 0.0–1.0 per channel.
pub type Rgb = (f32, f32, f32);

impl TokenKind {
    /// Every kind, so callers and tests can enumerate exhaustively.
    pub const ALL: [TokenKind; 4] = [
        TokenKind::Character,
        TokenKind::Npc,
        TokenKind::Vehicle,
        TokenKind::Object,
    ];

    /// Parse the string stored in `tokens.token_type`.
    ///
    /// Unknown values answer `None`. The caller decides what to do about it —
    /// the renderer falls back to [`TokenKind::Character`] rather than
    /// refusing to draw, because a token you cannot see is worse than a token
    /// wearing the wrong colour.
    pub fn from_stored(stored: &str) -> Option<TokenKind> {
        match stored {
            "character" => Some(TokenKind::Character),
            "npc" => Some(TokenKind::Npc),
            "vehicle" => Some(TokenKind::Vehicle),
            "object" => Some(TokenKind::Object),
            _ => None,
        }
    }

    /// The string this kind is stored as. Inverse of [`Self::from_stored`].
    pub fn as_stored(self) -> &'static str {
        match self {
            TokenKind::Character => "character",
            TokenKind::Npc => "npc",
            TokenKind::Vehicle => "vehicle",
            TokenKind::Object => "object",
        }
    }

    /// What a human calls this kind.
    pub fn label(self) -> &'static str {
        match self {
            TokenKind::Character => "Character",
            TokenKind::Npc => "NPC",
            TokenKind::Vehicle => "Vehicle",
            TokenKind::Object => "Object",
        }
    }

    /// The colour a token of this kind is drawn in when it has no portrait.
    ///
    /// Chosen for separation in both hue *and* lightness, so the four stay
    /// distinguishable to someone who cannot rely on hue alone — see the
    /// module note. Every variant is spelled out with no catch-all so adding
    /// a kind fails to compile until it has been given an appearance.
    pub fn fill(self) -> Rgb {
        match self {
            // Blue, and the lightest: the character is what a player looks
            // for first, so it wins the most legible slot.
            TokenKind::Character => (0.282, 0.565, 0.996),
            // Deep red. Darker than the character, so the pair separates by
            // lightness as well as by hue.
            TokenKind::Npc => (0.784, 0.208, 0.216),
            // Amber. Warm like the NPC but far lighter, and far from both
            // blues.
            TokenKind::Vehicle => (0.918, 0.678, 0.196),
            // Slate. Deliberately desaturated — scenery should recede rather
            // than compete with anything that acts.
            TokenKind::Object => (0.435, 0.478, 0.529),
        }
    }

    /// Perceived lightness of this kind's fill, 0.0–1.0.
    ///
    /// Rec. 709 luma, which weights green far above blue because human vision
    /// does. A plain channel average would call the amber and the slate
    /// equally light and they are nothing alike.
    pub fn luma(self) -> f32 {
        let (r, g, b) = self.fill();
        0.2126 * r + 0.7152 * g + 0.0722 * b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_round_trips_through_the_stored_string() {
        for kind in TokenKind::ALL {
            assert_eq!(TokenKind::from_stored(kind.as_stored()), Some(kind));
        }
    }

    /// The column default has to be a value this code understands.
    #[test]
    fn the_default_kind_is_the_one_the_column_defaults_to() {
        assert_eq!(TokenKind::default(), TokenKind::Character);
        assert_eq!(TokenKind::default().as_stored(), "character");
    }

    #[test]
    fn an_unrecognised_kind_is_none_rather_than_a_guess() {
        for wrong in ["NPC", "Character", "monster", "", "vehicle "] {
            assert_eq!(
                TokenKind::from_stored(wrong),
                None,
                "{wrong:?} must not resolve to a kind"
            );
        }
    }

    /// The property the palette exists for.
    ///
    /// Two kinds that look alike are worse than one kind, because they
    /// promise a distinction they do not deliver. This asserts separation in
    /// hue *and* in lightness, so the set survives a viewer who cannot use
    /// hue: roughly one man in twelve has a red-green deficiency, and a
    /// palette that collapses for them collapses in the middle of a fight.
    #[test]
    fn every_pair_of_kinds_is_distinguishable() {
        /// Squared euclidean distance in RGB — a coarse hue check, but enough
        /// to catch two variants pointed at nearly the same colour.
        fn separation(a: Rgb, b: Rgb) -> f32 {
            let (dr, dg, db) = (a.0 - b.0, a.1 - b.1, a.2 - b.2);
            dr * dr + dg * dg + db * db
        }

        for (i, a) in TokenKind::ALL.iter().enumerate() {
            for b in &TokenKind::ALL[i + 1..] {
                let rgb_gap = separation(a.fill(), b.fill());
                assert!(
                    rgb_gap > 0.05,
                    "{:?} and {:?} are too close in colour ({rgb_gap:.3})",
                    a,
                    b
                );

                let luma_gap = (a.luma() - b.luma()).abs();
                assert!(
                    luma_gap > 0.05,
                    "{:?} and {:?} differ by only {luma_gap:.3} in lightness — \
                     they would collapse for a viewer who cannot use hue",
                    a,
                    b
                );
            }
        }
    }

    /// Every kind is drawable. A colour of all zeroes usually means somebody
    /// added a variant and left the arm to be filled in later.
    #[test]
    fn no_kind_is_left_without_an_appearance() {
        for kind in TokenKind::ALL {
            let (r, g, b) = kind.fill();
            assert!(
                r + g + b > 0.1,
                "{kind:?} has no fill colour worth the name"
            );
            assert!(!kind.label().is_empty());
        }
    }

    #[test]
    fn scenery_is_the_quietest_thing_on_the_board() {
        // Not a style preference: an object that competes with a creature for
        // attention is a bug report waiting to happen.
        let (r, g, b) = TokenKind::Object.fill();
        let spread = r.max(g).max(b) - r.min(g).min(b);
        for acting in [TokenKind::Character, TokenKind::Npc, TokenKind::Vehicle] {
            let (ar, ag, ab) = acting.fill();
            let acting_spread = ar.max(ag).max(ab) - ar.min(ag).min(ab);
            assert!(
                spread < acting_spread,
                "{acting:?} is less saturated than scenery, which inverts the \
                 visual hierarchy"
            );
        }
    }
}
