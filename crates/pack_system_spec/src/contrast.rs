//! Colour parsing and the WCAG legibility floor an interface pack must clear.
//!
//! Spec 032 (FR-012a, research §6). An interface pack sets the whole table's
//! colours and FR-009 leaves a reader no per-person override to escape to, so
//! an unreadable pack is unreadable for everyone at that table until the Game
//! Master changes packs. That is why the floor is checked here, once, at
//! validation, and why failing it is a rejection rather than a warning.
//!
//! Two jobs live in one module because they cannot be separated: you cannot
//! measure the contrast of a colour you cannot parse. [`parse_color`] turns
//! the CSS colour strings a pack actually writes into sRGB;
//! [`relative_luminance`] and [`contrast_ratio`] measure them.
//!
//! # This is NOT `thunderforge_canvas_core::resource_display::luma`
//!
//! Read that function beside this one and they look like the same thing. They
//! are not, and confusing them is the failure this section exists to prevent.
//!
//! - `resource_display::luma` is **Rec. 709 luma**: `0.2126·R + 0.7152·G +
//!   0.0722·B` applied to *gamma-encoded* channel values. It picks token bar
//!   colours that separate by perceived lightness. Cheap, approximate, and
//!   entirely adequate for "are these two bars distinguishable".
//! - [`relative_luminance`] here is **WCAG 2.x relative luminance**: the same
//!   three coefficients applied to channels *linearised first* through the
//!   `0.03928 / 12.92 / ((c+0.055)/1.055)^2.4` piecewise transfer function.
//!
//! Identical coefficients, one extra step, and the two disagree by enough to
//! move a colour across a threshold — mid-grey `#767676` on white is 4.54:1 by
//! the WCAG definition and passes AA; drop the linearisation and the same pair
//! measures differently and can fail. Neither function is a drop-in for the
//! other, and neither should be "unified" into the other by a later reader who
//! notices they share a line of arithmetic. `luma` must not be used to decide
//! whether a pack is legible, and this must not be used to space a palette.

/// WCAG 2.x AA minimum for normal-size body text.
///
/// Applied to foreground/background pairs a person reads words out of.
pub const AA_NORMAL_TEXT: f64 = 4.5;

/// WCAG 2.x AA minimum for large text and for non-text/UI components.
///
/// Kept as a separate constant from [`AA_NORMAL_TEXT`] deliberately. A border,
/// an input outline and a focus ring are shapes, not sentences, and WCAG asks
/// 3:1 of them. Applying 4.5 to a border colour would reject packs for no
/// reader's benefit — the pack would be rejected over a value no reader ever
/// had to read — while applying 3.0 to body text would let through packs a
/// reader genuinely cannot use. Collapsing the two into one number is wrong in
/// one direction or the other whichever number you pick.
pub const AA_LARGE_TEXT_AND_UI: f64 = 3.0;

/// A colour the validator could not parse.
///
/// Carries the offending value, because an interface pack declares dozens of
/// colours and "unparseable colour" without the string sends an author reading
/// their whole file (quickstart.md's point about error messages).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorParseError {
    /// The value exactly as it appeared in the manifest.
    pub value: String,
    /// What was wrong with it.
    pub reason: String,
}

impl std::fmt::Display for ColorParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "`{}` is not a colour this validator can measure: {}. Use oklch(), #rgb/#rrggbb/#rrggbbaa, or rgb()/rgba()",
            self.value, self.reason
        )
    }
}

impl std::error::Error for ColorParseError {}

/// A colour in gamma-encoded sRGB, every component `0.0..=1.0`.
///
/// Gamma-encoded rather than linear because that is what every input form
/// gives us directly (a hex pair *is* an encoded channel) and what alpha
/// compositing in a browser operates on. Linearisation happens once, inside
/// [`relative_luminance`], which is the only place WCAG asks for it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Srgb {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    /// `1.0` unless the value carried an explicit alpha.
    pub a: f64,
}

impl Srgb {
    /// Composite this colour over an opaque backdrop.
    ///
    /// Needed because the product's own stylesheet declares translucent UI
    /// colours — `--border: oklch(1 0 0 / 10%)` in `.dark` in
    /// `apps/web/src/styles/globals.css` — and the contrast of a 10%-white
    /// border is a question about what is behind it, not about white.
    #[must_use]
    pub fn over(self, backdrop: Srgb) -> Srgb {
        let a = self.a;
        Srgb {
            r: self.r * a + backdrop.r * (1.0 - a),
            g: self.g * a + backdrop.g * (1.0 - a),
            b: self.b * a + backdrop.b * (1.0 - a),
            a: 1.0,
        }
    }
}

/// WCAG 2.x relative luminance of a colour, `0.0..=1.0`.
///
/// Linearise each channel through the piecewise sRGB transfer function, then
/// weight `0.2126·R + 0.7152·G + 0.0722·B`. See the module doc for why this is
/// not `resource_display::luma` despite sharing those three coefficients.
///
/// Alpha is ignored: a translucent colour has no luminance of its own. Composite
/// it with [`Srgb::over`] first.
#[must_use]
pub fn relative_luminance(color: Srgb) -> f64 {
    fn linearise(channel: f64) -> f64 {
        if channel <= 0.03928 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * linearise(color.r) + 0.7152 * linearise(color.g) + 0.0722 * linearise(color.b)
}

/// WCAG contrast ratio of a foreground against a background, `1.0..=21.0`.
///
/// `(L1 + 0.05) / (L2 + 0.05)` with the lighter colour on top, so the result is
/// symmetric and never below 1.
///
/// A translucent foreground is composited over the background first, which is
/// the only interpretation that means anything: `oklch(1 0 0 / 10%)` measured
/// as if it were opaque white would report a contrast the reader never sees. A
/// translucent *background* is taken at face value — what sits behind it is
/// outside the manifest, so the honest choice is to measure the colour as
/// declared rather than invent a backdrop for it.
#[must_use]
pub fn contrast_ratio(foreground: Srgb, background: Srgb) -> f64 {
    let foreground = if foreground.a < 1.0 {
        foreground.over(background)
    } else {
        foreground
    };
    let (l1, l2) = (
        relative_luminance(foreground),
        relative_luminance(background),
    );
    let (lighter, darker) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
    (lighter + 0.05) / (darker + 0.05)
}

/// Parse both sides and measure them, so a caller checking a declared pair does
/// not have to thread two `Result`s together.
pub fn contrast_ratio_of(foreground: &str, background: &str) -> Result<f64, ColorParseError> {
    Ok(contrast_ratio(
        parse_color(foreground)?,
        parse_color(background)?,
    ))
}

/// Parse a CSS colour string into sRGB.
///
/// The accepted forms are the ones the product's own stylesheet uses — read the
/// `:root` and `.dark` blocks of `apps/web/src/styles/globals.css`, which are
/// the corpus the bundled Forge pack is transcribed from (T026):
///
/// - `oklch(L C H)` and `oklch(L C H / A)` — overwhelmingly the dominant form
///   there, and the only one those two blocks use.
/// - `#rgb`, `#rrggbb`, `#rrggbbaa`
/// - `rgb()` / `rgba()`, comma- or space-separated, channels as integers or
///   percentages.
///
/// Anything else is an error naming the value. There is deliberately no
/// fallback to black, white, or transparent: a fallback would let an
/// unparseable colour silently pass a contrast check that was performed against
/// a colour nobody wrote, and the pack would ship looking nothing like the
/// thing that was measured.
pub fn parse_color(value: &str) -> Result<Srgb, ColorParseError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(err(value, "the value is empty"));
    }
    if let Some(hex) = trimmed.strip_prefix('#') {
        return parse_hex(value, hex);
    }
    let lowered = trimmed.to_ascii_lowercase();
    if let Some(inner) = function_body(&lowered, "oklch") {
        return parse_oklch(value, inner);
    }
    if let Some(inner) = function_body(&lowered, "rgba") {
        return parse_rgb(value, inner);
    }
    if let Some(inner) = function_body(&lowered, "rgb") {
        return parse_rgb(value, inner);
    }
    Err(err(
        value,
        "unrecognised colour syntax (named colours, hsl(), lab() and colour keywords are not accepted)",
    ))
}

fn err(value: &str, reason: &str) -> ColorParseError {
    ColorParseError {
        value: value.to_string(),
        reason: reason.to_string(),
    }
}

/// `oklch( … )` → ` … `, or `None` if this is not that function.
fn function_body<'a>(lowered: &'a str, name: &str) -> Option<&'a str> {
    let rest = lowered.strip_prefix(name)?;
    let rest = rest.trim_start().strip_prefix('(')?;
    rest.strip_suffix(')')
}

/// Split a functional colour's body into components and an optional alpha.
///
/// Commas are normalised to spaces so `rgb(1, 2, 3)` and `rgb(1 2 3)` take one
/// path; a `/` separates the modern alpha, and a fourth bare component is the
/// legacy `rgba(r, g, b, a)` alpha.
fn components(body: &str) -> (Vec<String>, Option<String>) {
    let normalised = body.replace(',', " ");
    let mut halves = normalised.splitn(2, '/');
    let head = halves.next().unwrap_or("");
    let mut parts: Vec<String> = head.split_whitespace().map(str::to_string).collect();
    match halves.next() {
        Some(alpha) => (parts, Some(alpha.trim().to_string())),
        None if parts.len() == 4 => {
            let alpha = parts.pop();
            (parts, alpha)
        }
        None => (parts, None),
    }
}

/// A number, or a percentage of `full`.
fn number(value: &str, raw: &str, full: f64) -> Result<f64, ColorParseError> {
    let raw = raw.trim();
    match raw.strip_suffix('%') {
        Some(percent) => percent
            .trim()
            .parse::<f64>()
            .map(|p| p / 100.0 * full)
            .map_err(|_| err(value, &format!("`{raw}` is not a number"))),
        None => raw
            .parse::<f64>()
            .map_err(|_| err(value, &format!("`{raw}` is not a number"))),
    }
}

fn parse_alpha(value: &str, raw: Option<String>) -> Result<f64, ColorParseError> {
    match raw {
        None => Ok(1.0),
        Some(raw) => Ok(number(value, &raw, 1.0)?.clamp(0.0, 1.0)),
    }
}

fn parse_hex(value: &str, hex: &str) -> Result<Srgb, ColorParseError> {
    let digits: Vec<char> = hex.trim().chars().collect();
    if !digits.iter().all(char::is_ascii_hexdigit) {
        return Err(err(value, "contains a character that is not a hex digit"));
    }
    let expand = |c: char| -> String { format!("{c}{c}") };
    let pairs: Vec<String> = match digits.len() {
        3 | 4 => digits.iter().map(|c| expand(*c)).collect(),
        6 | 8 => digits.chunks(2).map(|c| c.iter().collect()).collect(),
        other => {
            return Err(err(
                value,
                &format!("a hex colour has 3, 4, 6 or 8 digits, not {other}"),
            ));
        }
    };
    let channel = |pair: &String| -> f64 {
        // Unreachable failure: every character was checked as a hex digit above.
        u8::from_str_radix(pair, 16).unwrap_or(0) as f64 / 255.0
    };
    Ok(Srgb {
        r: channel(&pairs[0]),
        g: channel(&pairs[1]),
        b: channel(&pairs[2]),
        a: pairs.get(3).map_or(1.0, channel),
    })
}

fn parse_rgb(value: &str, body: &str) -> Result<Srgb, ColorParseError> {
    let (parts, alpha) = components(body);
    if parts.len() != 3 {
        return Err(err(
            value,
            &format!("rgb() takes three channels, found {}", parts.len()),
        ));
    }
    // A percentage channel is a percentage of 255; a bare number already is a
    // 0–255 channel. Both land on the same 0.0–1.0 scale.
    let channel = |raw: &str| -> Result<f64, ColorParseError> {
        Ok((number(value, raw, 255.0)? / 255.0).clamp(0.0, 1.0))
    };
    Ok(Srgb {
        r: channel(&parts[0])?,
        g: channel(&parts[1])?,
        b: channel(&parts[2])?,
        a: parse_alpha(value, alpha)?,
    })
}

/// `oklch(L C H)` → sRGB.
///
/// OKLCH is polar OKLab, so the conversion is: LCH → Lab (`a = C·cos(H°)`,
/// `b = C·sin(H°)`), Lab → LMS through the inverse matrix, cube each LMS
/// component, LMS → linear sRGB through the second matrix, then gamma-encode.
///
/// Coefficients are Björn Ottosson's published values from "A perceptual color
/// space for image processing" (<https://bottosson.github.io/posts/oklab/>),
/// the definition CSS Color 4 adopts for `oklch()`. Transcribed rather than
/// pulled from a crate: this crate has four dependencies and none of them is a
/// colour library, and the whole conversion is two matrices and a cube root.
fn parse_oklch(value: &str, body: &str) -> Result<Srgb, ColorParseError> {
    let (parts, alpha) = components(body);
    if parts.len() != 3 {
        return Err(err(
            value,
            &format!(
                "oklch() takes lightness, chroma and hue, found {} component(s)",
                parts.len()
            ),
        ));
    }
    // Lightness is 0–1, or a percentage of it. Chroma is an absolute number,
    // where CSS defines 100% as 0.4. Hue is degrees; `deg` is the only unit
    // CSS lets `oklch()` take without a conversion nobody here needs.
    let lightness = number(value, &parts[0], 1.0)?;
    let chroma = number(value, &parts[1], 0.4)?;
    let hue_raw = parts[2].trim_end_matches("deg");
    let hue = number(value, hue_raw, 360.0)?;

    let hue_radians = hue.to_radians();
    let lab_a = chroma * hue_radians.cos();
    let lab_b = chroma * hue_radians.sin();

    let l_ = lightness + 0.396_337_777_4 * lab_a + 0.215_803_757_3 * lab_b;
    let m_ = lightness - 0.105_561_345_8 * lab_a - 0.063_854_172_8 * lab_b;
    let s_ = lightness - 0.089_484_177_5 * lab_a - 1.291_485_548_0 * lab_b;

    let (l, m, s) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);

    let linear_r = 4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s;
    let linear_g = -1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s;
    let linear_b = -0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701_0 * s;

    Ok(Srgb {
        r: encode(linear_r),
        g: encode(linear_g),
        b: encode(linear_b),
        a: parse_alpha(value, alpha)?,
    })
}

/// Linear sRGB → gamma-encoded sRGB, clamped to the gamut.
///
/// The clamp is deliberate, and it is the only place this module silently
/// changes a value. OKLCH can address colours sRGB cannot show — the product's
/// own `--destructive: oklch(0.577 0.245 27.325)` is one, its green channel
/// coming out negative — and a browser still paints those, clipped to whatever
/// the display can manage. Refusing to measure an out-of-gamut colour would
/// reject a pack over a value the author sees rendering perfectly well, for a
/// reason no reader of the error could act on. So we measure the colour as it
/// will actually appear, which is the clipped one.
fn encode(linear: f64) -> f64 {
    let c = linear.clamp(0.0, 1.0);
    if c <= 0.003_130_8 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ratios are compared with a tolerance because the reference figures are
    /// published to two decimal places; the parse itself is exact.
    fn assert_close(actual: f64, expected: f64, what: &str) {
        assert!(
            (actual - expected).abs() < 0.01,
            "{what}: expected ~{expected}, got {actual}"
        );
    }

    fn ratio(fg: &str, bg: &str) -> f64 {
        contrast_ratio_of(fg, bg).expect("both colours parse")
    }

    /// The published anchor of the whole scale: WCAG's maximum.
    #[test]
    fn black_on_white_is_twenty_one_to_one() {
        assert_close(ratio("#000000", "#ffffff"), 21.0, "black on white");
        assert_close(ratio("#000", "#fff"), 21.0, "short-form black on white");
        assert_close(
            ratio("rgb(0, 0, 0)", "rgb(255, 255, 255)"),
            21.0,
            "rgb() black on white",
        );
        assert_close(
            ratio("oklch(0 0 0)", "oklch(1 0 0)"),
            21.0,
            "oklch black on white",
        );
    }

    /// The other end: a colour has no contrast with itself.
    #[test]
    fn white_on_white_is_one_to_one() {
        assert_close(ratio("#ffffff", "#ffffff"), 1.0, "white on white");
        assert_close(ratio("oklch(1 0 0)", "oklch(1 0 0)"), 1.0, "oklch white");
    }

    /// `#767676` on white is the canonical "just passes AA" grey (4.54:1) and
    /// `#777777` the next step up is the one that just fails (4.48:1). One
    /// shade apart, opposite sides of the line — which is exactly why the
    /// threshold has to be computed rather than eyeballed.
    #[test]
    fn the_normal_text_threshold_is_exercised_in_both_directions() {
        let passes = ratio("#767676", "#ffffff");
        assert_close(passes, 4.54, "#767676 on white");
        assert!(passes >= AA_NORMAL_TEXT, "{passes} should clear 4.5");

        let fails = ratio("#777777", "#ffffff");
        assert_close(fails, 4.48, "#777777 on white");
        assert!(fails < AA_NORMAL_TEXT, "{fails} should not clear 4.5");
    }

    /// The non-text floor, either side. `#8f8f8f` on white is 3.23:1 and clears
    /// 3:1 as a border would need to; `#959595` is 3.00:1 and does not — and
    /// note that *both* fail 4.5:1. That is the pair that would be rejected if
    /// the two thresholds were ever collapsed into one number.
    #[test]
    fn the_ui_threshold_is_exercised_in_both_directions() {
        let passes = ratio("#8f8f8f", "#ffffff");
        assert_close(passes, 3.23, "#8f8f8f on white");
        assert!(passes >= AA_LARGE_TEXT_AND_UI, "{passes} should clear 3.0");
        assert!(
            passes < AA_NORMAL_TEXT,
            "a border colour need not clear 4.5"
        );

        let fails = ratio("#959595", "#ffffff");
        assert_close(fails, 3.00, "#959595 on white");
        assert!(fails < AA_LARGE_TEXT_AND_UI, "{fails} should not clear 3.0");
    }

    /// The real corpus: `--foreground` on `--background` from the `:root` block
    /// of `apps/web/src/styles/globals.css`, verbatim. Near-black text on white
    /// should be near the top of the scale, and it is.
    #[test]
    fn the_products_own_light_palette_is_legible() {
        let foreground = "oklch(0.145 0 0)";
        let background = "oklch(1 0 0)";
        let measured = ratio(foreground, background);
        assert_close(measured, 19.79, "--foreground on --background, light");
        assert!(measured >= AA_NORMAL_TEXT);

        // `--muted-foreground` is the closest call in that block — deliberately
        // dimmed text, and it clears 4.5 with very little to spare. Worth
        // pinning: a pack author copying Forge and darkening the background
        // slightly is who this check catches.
        let muted = ratio("oklch(0.556 0 0)", background);
        assert_close(muted, 4.73, "--muted-foreground on --background");
        assert!(muted >= AA_NORMAL_TEXT);
    }

    /// The `.dark` block of the same file, which inverts the pair.
    #[test]
    fn the_products_own_dark_palette_is_legible() {
        let measured = ratio("oklch(0.985 0 0)", "oklch(0.145 0 0)");
        assert_close(measured, 18.96, "--foreground on --background, dark");
        assert!(measured >= AA_NORMAL_TEXT);
    }

    /// `--border` in `.dark` is `oklch(1 0 0 / 10%)`. Measured as opaque white
    /// it would report a contrast the reader never sees; composited over the
    /// dark background it reports what is actually on screen — and, as it
    /// happens, does not clear 3:1, which is a real finding about the current
    /// stylesheet rather than a defect in this module.
    #[test]
    fn a_translucent_border_is_measured_against_what_is_behind_it() {
        let background = "oklch(0.145 0 0)";
        let border = parse_color("oklch(1 0 0 / 10%)").expect("parses");
        assert_close(border.a, 0.1, "alpha as a percentage");

        let composited = contrast_ratio(border, parse_color(background).expect("parses"));
        let as_if_opaque = ratio("oklch(1 0 0)", background);
        assert!(
            composited < as_if_opaque,
            "compositing must lower the ratio, got {composited} vs {as_if_opaque}"
        );
        assert!(composited < AA_LARGE_TEXT_AND_UI);
    }

    /// `--destructive` is out of the sRGB gamut: its green channel comes out
    /// negative. It still has to be measurable, because a browser still paints
    /// it. See `encode`.
    #[test]
    fn an_out_of_gamut_oklch_value_is_clamped_rather_than_refused() {
        let destructive = parse_color("oklch(0.577 0.245 27.325)").expect("parses");
        assert_close(destructive.r, 0.906, "red channel");
        assert_close(destructive.g, 0.0, "green clamps to zero, not below it");
        for channel in [destructive.r, destructive.g, destructive.b] {
            assert!((0.0..=1.0).contains(&channel), "{channel} out of range");
        }
    }

    /// Every accepted syntax, including the ones `globals.css` does not use but
    /// the contract promises (contracts/interface-pack-manifest.md).
    #[test]
    fn the_accepted_syntaxes_all_parse() {
        let white = parse_color("#ffffff").expect("hex");
        for form in [
            "#fff",
            "#FFFFFF",
            "#ffffffff",
            "rgb(255 255 255)",
            "rgb(255, 255, 255)",
            "rgba(255, 255, 255, 1)",
            "rgb(100% 100% 100%)",
            "rgb(255 255 255 / 100%)",
            "oklch(1 0 0)",
            "oklch(100% 0 0)",
            "  oklch(1 0 0)  ",
        ] {
            let parsed = parse_color(form).unwrap_or_else(|e| panic!("{form}: {e}"));
            assert_close(parsed.r, white.r, form);
            assert_close(parsed.g, white.g, form);
            assert_close(parsed.b, white.b, form);
            assert_close(parsed.a, 1.0, form);
        }

        let half = parse_color("rgba(0, 0, 0, 0.5)").expect("legacy rgba alpha");
        assert_close(half.a, 0.5, "legacy comma alpha");
        let hex_alpha = parse_color("#00000080").expect("8-digit hex");
        assert_close(hex_alpha.a, 0.502, "8-digit hex alpha");
    }

    /// The whole point of having no fallback: an unmeasurable colour must stop
    /// the pack, and the message must say which value stopped it.
    #[test]
    fn an_unparseable_colour_is_an_error_naming_the_value() {
        for bad in [
            "rebeccapurple",
            "hsl(200 50% 50%)",
            "#12345",
            "#gggggg",
            "oklch(0.5 0)",
            "rgb(1 2)",
            "",
        ] {
            let error = parse_color(bad).expect_err("must not parse");
            assert_eq!(error.value, bad, "the error must carry the raw value");
            assert!(
                error.to_string().contains(bad),
                "the message must name `{bad}`, got: {error}"
            );
            assert!(!error.reason.is_empty(), "`{bad}` needs a stated reason");
        }
    }

    /// Guards the module doc's claim. If someone ever "unifies" this with
    /// `resource_display::luma` by deleting the linearisation, this fails.
    #[test]
    fn relative_luminance_is_not_rec_709_luma_on_encoded_values() {
        let grey = parse_color("#767676").expect("parses");
        let wcag = relative_luminance(grey);
        let rec_709_on_encoded = 0.2126 * grey.r + 0.7152 * grey.g + 0.0722 * grey.b;
        assert!(
            (wcag - rec_709_on_encoded).abs() > 0.2,
            "the two definitions must not be interchangeable: {wcag} vs {rec_709_on_encoded}"
        );
    }
}
