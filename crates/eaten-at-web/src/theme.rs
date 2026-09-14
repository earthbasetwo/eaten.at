//! Publication themes: four author-chosen colors, clamped to readable
//! contrast and rendered into CSS custom properties as bare integers.
//!
//! The stylesheet needs one more ground than an author gives: the bright
//! surface for fields, sheets, and panels. Which way it moves depends on
//! whether the ground is light or dark, which CSS cannot ask, so it is
//! derived here and emitted alongside the four author colors. Everything
//! else (the ink shades, stone, the hairline) the stylesheet mixes from
//! the foreground and background itself.

use std::fmt::Write as _;

/// WCAG AA minimum contrast for body text.
pub const MIN_CONTRAST: f64 = 4.5;

/// An sRGB color. `u8` channels make out-of-range values unrepresentable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0 };
    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
    };

    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// WCAG relative luminance, 0 (black) to 1 (white).
    pub fn luminance(self) -> f64 {
        fn channel(c: u8) -> f64 {
            let c = f64::from(c) / 255.0;
            if c <= 0.039_28 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * channel(self.r) + 0.7152 * channel(self.g) + 0.0722 * channel(self.b)
    }

    /// `rrggbb`, lowercase, no `#`.
    pub fn to_hex(self) -> String {
        format!("{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    /// Space-separated channels for `rgb(var(--x))`.
    fn channels(self) -> String {
        format!("{} {} {}", self.r, self.g, self.b)
    }

    /// Linear blend toward `target` by `t` in [0, 1].
    fn mix(self, target: Self, t: f64) -> Self {
        // The value is rounded and clamped to 0..=255 before the cast, so
        // neither truncation nor sign loss can occur.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let lerp = |a: u8, b: u8| -> u8 {
            let v = f64::from(a) + (f64::from(b) - f64::from(a)) * t;
            v.round().clamp(0.0, 255.0) as u8
        };
        Self::new(
            lerp(self.r, target.r),
            lerp(self.g, target.g),
            lerp(self.b, target.b),
        )
    }
}

/// WCAG contrast ratio between two colors, 1 to 21.
pub fn contrast_ratio(a: Rgb, b: Rgb) -> f64 {
    let (la, lb) = (a.luminance(), b.luminance());
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// A publication's palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Theme {
    pub background: Rgb,
    pub foreground: Rgb,
    pub accent: Rgb,
    pub accent_foreground: Rgb,
}

/// How far the raised surface moves toward white from a light ground.
/// Masthead's paper-bright sits about seven tenths of the way to white
/// from its paper.
const RAISE_LIGHT: f64 = 0.7;
/// How far the raised surface moves toward white from a dark ground: a
/// small step, so light text on it keeps its contrast.
const RAISE_DARK: f64 = 0.06;

impl Theme {
    /// Adjust text colors until every text/ground pair meets
    /// [`MIN_CONTRAST`]. Backgrounds are kept as the author chose them;
    /// the foreground is pushed toward black or white, whichever the
    /// background favours. Colors that already pass are returned untouched.
    ///
    /// Text and the accent (as link text) are checked against both
    /// grounds they are set on: the page and the raised surface. Where a
    /// ground sits so close to mid-gray that no color reaches the minimum
    /// on both, the result is the extreme (black or white), which is the
    /// best available.
    ///
    /// Only an author's palette passes through here. The site's own
    /// palette is the handoff's, exact: its vermilion reads 3.99 on
    /// paper, which this clamp would darken (see `docs/design.md`).
    #[must_use]
    pub fn clamped(self) -> Self {
        let raised = self.raised();
        // The accent may move; its text must be checked against where it
        // ends up, not where the author put it.
        let accent = ensure_contrast(self.accent, &[self.background, raised]);
        Self {
            background: self.background,
            foreground: ensure_contrast(self.foreground, &[self.background, raised]),
            accent,
            accent_foreground: ensure_contrast(self.accent_foreground, &[accent]),
        }
    }

    /// The surface for fields, sheets, and panels: lighter than the page.
    pub fn raised(&self) -> Rgb {
        let t = if is_light(self.background) {
            RAISE_LIGHT
        } else {
            RAISE_DARK
        };
        self.background.mix(Rgb::WHITE, t)
    }

    /// The CSS custom property declarations for `:root`, containing only
    /// digits, spaces, and the fixed property names.
    pub fn css_declarations(&self) -> String {
        let mut out = String::new();
        for (name, color) in [
            ("--theme-bg", self.background),
            ("--theme-fg", self.foreground),
            ("--theme-accent", self.accent),
            ("--theme-accent-fg", self.accent_foreground),
            ("--theme-raised", self.raised()),
        ] {
            let _ = write!(out, "{name}: {};", color.channels());
        }
        out
    }

    /// A complete `<style>` body applying the theme to `:root` and
    /// pinning the color scheme, since an author palette is one palette.
    pub fn css_rule(&self) -> String {
        format!(
            ":root[data-theme]{{{} color-scheme: light;}}",
            self.css_declarations()
        )
    }
}

/// Blend steps tried when pushing a color toward its contrast target.
const CLAMP_STEPS: u32 = 64;

/// Whether dark text reads better than light text on `bg`.
fn is_light(bg: Rgb) -> bool {
    contrast_ratio(Rgb::BLACK, bg) > contrast_ratio(Rgb::WHITE, bg)
}

/// Whether `fg` meets [`MIN_CONTRAST`] on every one of `grounds`.
fn contrasts_with_all(fg: Rgb, grounds: &[Rgb]) -> bool {
    grounds
        .iter()
        .all(|&bg| contrast_ratio(fg, bg) >= MIN_CONTRAST)
}

/// Move `fg` toward black or white until it contrasts with every one of
/// `grounds`. The direction is the one the first ground favours; the
/// rest are expected to be close to it in lightness.
fn ensure_contrast(fg: Rgb, grounds: &[Rgb]) -> Rgb {
    let Some(&first) = grounds.first() else {
        return fg;
    };
    if contrasts_with_all(fg, grounds) {
        return fg;
    }
    let target = if is_light(first) {
        Rgb::BLACK
    } else {
        Rgb::WHITE
    };
    for i in 1..=CLAMP_STEPS {
        let candidate = fg.mix(target, f64::from(i) / f64::from(CLAMP_STEPS));
        if contrasts_with_all(candidate, grounds) {
            return candidate;
        }
    }
    target
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn luminance_and_contrast_match_reference_values() {
        assert!((Rgb::WHITE.luminance() - 1.0).abs() < 1e-9);
        assert!(Rgb::BLACK.luminance().abs() < 1e-9);
        assert!((contrast_ratio(Rgb::BLACK, Rgb::WHITE) - 21.0).abs() < 1e-9);
        // #777 on white is the classic "just fails AA" gray: 4.48.
        let gray = Rgb::new(0x77, 0x77, 0x77);
        let ratio = contrast_ratio(gray, Rgb::WHITE);
        assert!((ratio - 4.48).abs() < 0.01, "{ratio}");
    }

    #[test]
    fn passing_theme_is_unchanged() {
        let theme = Theme {
            background: Rgb::WHITE,
            foreground: Rgb::new(30, 30, 30),
            accent: Rgb::new(0, 100, 160),
            accent_foreground: Rgb::WHITE,
        };
        assert_eq!(theme.clamped(), theme);
    }

    #[test]
    fn failing_pairs_are_pushed_to_contrast_without_touching_backgrounds() {
        let theme = Theme {
            background: Rgb::new(250, 250, 240),
            foreground: Rgb::new(200, 200, 190), // light on light
            accent: Rgb::new(255, 200, 0),       // yellow link on cream
            accent_foreground: Rgb::new(255, 230, 120), // pale on yellow
        };
        let fixed = theme.clamped();
        assert_eq!(fixed.background, theme.background);
        assert!(contrast_ratio(fixed.foreground, fixed.background) >= MIN_CONTRAST);
        assert!(contrast_ratio(fixed.accent, fixed.background) >= MIN_CONTRAST);
        assert!(contrast_ratio(fixed.accent_foreground, fixed.accent) >= MIN_CONTRAST);
        // The dark direction was chosen for light backgrounds.
        assert!(fixed.foreground.luminance() < theme.foreground.luminance());
    }

    #[test]
    fn dark_backgrounds_push_text_lighter() {
        let theme = Theme {
            background: Rgb::new(20, 20, 30),
            foreground: Rgb::new(60, 60, 70),
            accent: Rgb::new(40, 40, 90),
            accent_foreground: Rgb::new(50, 50, 100),
        };
        let fixed = theme.clamped();
        assert!(fixed.foreground.luminance() > theme.foreground.luminance());
        assert!(contrast_ratio(fixed.foreground, fixed.background) >= MIN_CONTRAST);
        assert!(contrast_ratio(fixed.accent, fixed.background) >= MIN_CONTRAST);
    }

    #[test]
    fn any_background_can_be_satisfied() {
        // Mid-gray is the hardest background; both black and white must
        // still reach 4.5 from it, and the clamp must terminate there.
        for v in (0..=255u8).step_by(5) {
            let bg = Rgb::new(v, v, v);
            let fixed = ensure_contrast(bg, &[bg]);
            assert!(contrast_ratio(fixed, bg) >= MIN_CONTRAST, "bg {v}");
        }
    }

    #[test]
    fn surfaces_lift_in_the_direction_the_ground_favours() {
        // The site's own palette (Masthead): paper, ink, vermilion.
        let light = Theme {
            background: Rgb::new(0xf6, 0xf1, 0xe5),
            foreground: Rgb::new(0x1c, 0x19, 0x14),
            accent: Rgb::new(0xd8, 0x40, 0x1f),
            accent_foreground: Rgb::new(0xfd, 0xfb, 0xf4),
        };
        assert!(light.raised().luminance() > light.background.luminance());
        // Paper lifts to within a few steps of paper-bright (#fdfbf4).
        let raised = light.raised();
        assert!((i16::from(raised.r) - 0xfd).abs() <= 3, "{raised:?}");
        assert!((i16::from(raised.g) - 0xfb).abs() <= 3, "{raised:?}");
        assert!((i16::from(raised.b) - 0xf4).abs() <= 3, "{raised:?}");
        // The handoff's vermilion is under AA on paper, so the clamp would
        // darken it: the reason the site palette does not pass through
        // the clamp, and a fact `docs/design.md` records.
        let ratio = contrast_ratio(light.accent, light.background);
        assert!((ratio - 3.99).abs() < 0.01, "{ratio}");
        let clamped = light.clamped();
        assert_eq!(clamped.foreground, light.foreground);
        assert!(clamped.accent.luminance() < light.accent.luminance());

        let dark = Theme {
            background: Rgb::new(24, 20, 30),
            foreground: Rgb::new(235, 230, 240),
            accent: Rgb::new(255, 140, 120),
            accent_foreground: Rgb::new(24, 20, 30),
        };
        assert!(dark.raised().luminance() > dark.background.luminance());
        // The dark lift is a small step, not most of the way to white.
        assert!(contrast_ratio(dark.foreground, dark.raised()) >= MIN_CONTRAST);
    }

    #[test]
    fn clamped_text_reads_on_every_surface() {
        // Author grounds across the range, with text chosen to be as bad
        // as possible (the ground itself). Mid-grays are left out: there
        // no single color can reach the minimum on both surfaces, and the
        // clamp settles on black or white instead.
        for v in (0..=255u8).step_by(5).filter(|v| !(90..=150).contains(v)) {
            let bg = Rgb::new(v, v.saturating_sub(10), v.saturating_add(5));
            let theme = Theme {
                background: bg,
                foreground: bg,
                accent: bg,
                accent_foreground: bg,
            }
            .clamped();
            for ground in [theme.background, theme.raised()] {
                assert!(
                    contrast_ratio(theme.foreground, ground) >= MIN_CONTRAST,
                    "fg on {ground:?}, bg {v}"
                );
                assert!(
                    contrast_ratio(theme.accent, ground) >= MIN_CONTRAST,
                    "accent on {ground:?}, bg {v}"
                );
            }
            assert!(contrast_ratio(theme.accent_foreground, theme.accent) >= MIN_CONTRAST);
        }
    }

    #[test]
    fn css_output_contains_only_integers() {
        let theme = Theme {
            background: Rgb::new(1, 2, 3),
            foreground: Rgb::new(250, 251, 252),
            accent: Rgb::new(120, 190, 245),
            accent_foreground: Rgb::BLACK,
        };
        let decl = theme.css_declarations();
        assert_eq!(
            decl,
            "--theme-bg: 1 2 3;--theme-fg: 250 251 252;--theme-accent: 120 190 245;--theme-accent-fg: 0 0 0;--theme-raised: 16 17 18;"
        );
        assert!(decl.bytes().all(|b| b.is_ascii_digit()
            || b == b' '
            || b == b';'
            || b == b':'
            || b == b'-'
            || b.is_ascii_lowercase()));
        assert!(theme.css_rule().starts_with(":root[data-theme]{--theme-bg"));
        assert_eq!(theme.accent.to_hex(), "78bef5");
    }
}
