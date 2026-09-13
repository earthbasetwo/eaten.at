//! Publication themes: four author-chosen colors, clamped to readable
//! contrast and rendered into CSS custom properties as bare integers.

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

impl Theme {
    /// Adjust text colors until every text/background pair meets
    /// [`MIN_CONTRAST`]. Backgrounds are kept as the author chose them;
    /// the foreground is pushed toward black or white, whichever gets
    /// there. Colors that already pass are returned untouched.
    ///
    /// The accent is also checked as *link text on the page background*
    /// and nudged the same way, since that is how it is mostly used.
    #[must_use]
    pub fn clamped(self) -> Self {
        // The accent may move; its text must be checked against where it
        // ends up, not where the author put it.
        let accent = ensure_contrast(self.accent, self.background);
        Self {
            background: self.background,
            foreground: ensure_contrast(self.foreground, self.background),
            accent,
            accent_foreground: ensure_contrast(self.accent_foreground, accent),
        }
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

/// Move `fg` toward black or white until it contrasts with `bg`.
fn ensure_contrast(fg: Rgb, bg: Rgb) -> Rgb {
    if contrast_ratio(fg, bg) >= MIN_CONTRAST {
        return fg;
    }
    let target = if contrast_ratio(Rgb::WHITE, bg) >= contrast_ratio(Rgb::BLACK, bg) {
        Rgb::WHITE
    } else {
        Rgb::BLACK
    };
    for i in 1..=CLAMP_STEPS {
        let candidate = fg.mix(target, f64::from(i) / f64::from(CLAMP_STEPS));
        if contrast_ratio(candidate, bg) >= MIN_CONTRAST {
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
            let fixed = ensure_contrast(bg, bg);
            assert!(contrast_ratio(fixed, bg) >= MIN_CONTRAST, "bg {v}");
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
            "--theme-bg: 1 2 3;--theme-fg: 250 251 252;--theme-accent: 120 190 245;--theme-accent-fg: 0 0 0;"
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
