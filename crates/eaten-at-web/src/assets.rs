//! Static assets embedded in the binary, addressed by content hash so
//! they can be cached forever.
//!
//! The stylesheet refers to the font files by their plain names; when the
//! asset table is built those references are rewritten to the hashed
//! names, so a changed font changes the stylesheet's hash as well and no
//! cached stylesheet can point at a file that no longer exists.

use std::sync::LazyLock;

/// The stylesheet as written, before font paths are rewritten.
const CSS_SOURCE: &str = include_str!("../static/app.css");

/// The editor island: autosave, a restore banner, growing textareas.
/// A convenience on top of a form that works without it (plan §6.3).
pub const EDITOR_SCRIPT: &str = include_str!("../static/editor.js");

/// Place suggestions in the editor's choosing state (plan 12): the
/// search box as a combobox over this site's suggest endpoint. Needs
/// [`COMBOBOX_SCRIPT`] before it.
pub const PLACE_SUGGEST_SCRIPT: &str = include_str!("../static/place-suggest.js");

/// The combobox (plan 10): a listbox under a text field, fed by a source
/// the page names. Shared by the handle and place suggestions.
pub const COMBOBOX_SCRIPT: &str = include_str!("../static/combobox.js");

/// Handle suggestions from the Bluesky `AppView`, on the sign-in and
/// landing pages (plan 10, D42). Needs [`COMBOBOX_SCRIPT`] before it.
pub const HANDLE_TYPEAHEAD_SCRIPT: &str = include_str!("../static/handle-typeahead.js");

/// The live find on the author's home (plan 11): the find form's
/// results swapped in after a pause in typing, without a reload.
pub const FIND_SCRIPT: &str = include_str!("../static/find.js");

/// Connect on the signed-out landing page (plan 09): the sign-in link
/// swapped for the handle field in place, so signing in starts there.
/// The field suggests handles too, so it needs [`COMBOBOX_SCRIPT`] and
/// [`HANDLE_TYPEAHEAD_SCRIPT`] before it.
pub const CONNECT_SCRIPT: &str = include_str!("../static/connect.js");

/// Filed under, in the editor's writing state: the tags field's comma
/// list as chips, one slot for the next tag. The list still submits
/// as the text the server reads.
pub const TAGS_SCRIPT: &str = include_str!("../static/tags.js");

/// Every inline script the site ships, all counted against the tripwire.
pub const INLINE_SCRIPTS: &[&str] = &[
    EDITOR_SCRIPT,
    TAGS_SCRIPT,
    COMBOBOX_SCRIPT,
    HANDLE_TYPEAHEAD_SCRIPT,
    PLACE_SUGGEST_SCRIPT,
    FIND_SCRIPT,
    CONNECT_SCRIPT,
];

/// A tripwire on all inline JavaScript combined, in bytes (D43). Not a
/// target: the rule is to be judicious, and every page works without
/// any of it, which each page's no-JS test keeps proving. Crossing this
/// is the moment to look at how the site feels, not a reason to trim
/// by itself.
pub const JS_BUDGET_BYTES: usize = 24 * 1024;

/// Self-hosted web fonts. Newsreader and the mono face are OFL
/// (`static/fonts/OFL.txt`), as latin and latin-ext subsets that the
/// stylesheet's `unicode-range` descriptors choose between; Evantic
/// Regular is the logotype face bundled with the design handoff
/// (`static/fonts/EVANTIC.txt`), one file.
const FONT_FILES: &[(&str, &[u8])] = &[
    (
        "newsreader-latin.woff2",
        include_bytes!("../static/fonts/newsreader-latin.woff2"),
    ),
    (
        "newsreader-latin-ext.woff2",
        include_bytes!("../static/fonts/newsreader-latin-ext.woff2"),
    ),
    (
        "newsreader-italic-latin.woff2",
        include_bytes!("../static/fonts/newsreader-italic-latin.woff2"),
    ),
    (
        "newsreader-italic-latin-ext.woff2",
        include_bytes!("../static/fonts/newsreader-italic-latin-ext.woff2"),
    ),
    (
        "jetbrains-mono-latin.woff2",
        include_bytes!("../static/fonts/jetbrains-mono-latin.woff2"),
    ),
    (
        "jetbrains-mono-latin-ext.woff2",
        include_bytes!("../static/fonts/jetbrains-mono-latin-ext.woff2"),
    ),
    (
        "evantic-regular.woff2",
        include_bytes!("../static/fonts/evantic-regular.woff2"),
    ),
];

/// One servable file.
#[derive(Debug, PartialEq, Eq)]
pub struct Asset {
    /// The plain name, e.g. `app.css`.
    pub name: &'static str,
    /// The name with the content hash before the extension, e.g.
    /// `app.1a2b3c4d.css`.
    pub hashed_name: String,
    pub content_type: &'static str,
    pub body: &'static [u8],
}

impl Asset {
    fn new(name: &'static str, content_type: &'static str, body: &'static [u8]) -> Self {
        let hash = hex8(fnv1a(body));
        let hashed_name = match name.rsplit_once('.') {
            Some((stem, ext)) => format!("{stem}.{hash}.{ext}"),
            None => format!("{name}.{hash}"),
        };
        Self {
            name,
            hashed_name,
            content_type,
            body,
        }
    }

    /// URL path of the hashed form.
    pub fn path(&self) -> String {
        format!("/static/{}", self.hashed_name)
    }
}

/// How a matched request may be cached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Served {
    pub asset: &'static Asset,
    /// True when the request used the hashed name, which never changes
    /// meaning; the plain name may be cached only briefly.
    pub immutable: bool,
}

static ASSETS: LazyLock<Vec<Asset>> = LazyLock::new(build);

fn build() -> Vec<Asset> {
    let fonts: Vec<Asset> = FONT_FILES
        .iter()
        .map(|(name, bytes)| Asset::new(name, "font/woff2", bytes))
        .collect();
    let mut css = CSS_SOURCE.to_owned();
    for font in &fonts {
        css = css.replace(
            &format!("url(\"/static/{}\")", font.name),
            &format!("url(\"{}\")", font.path()),
        );
    }
    // One leak per process, at first use: every asset body is then a
    // plain static slice, which is what a response body wants.
    let css: &'static str = Box::leak(css.into_boxed_str());
    let mut assets = vec![Asset::new(
        "app.css",
        "text/css; charset=utf-8",
        css.as_bytes(),
    )];
    assets.extend(fonts);
    assets
}

/// The stylesheet as served, with hashed font paths.
pub fn css() -> &'static str {
    // Built from a `&str`, so the bytes are valid UTF-8.
    std::str::from_utf8(ASSETS[0].body).unwrap_or_default()
}

/// URL path of the stylesheet, including its hash: `/static/app.<hash>.css`.
pub fn css_path() -> String {
    ASSETS[0].path()
}

/// Resolve a `/static/{name}` request. Both the hashed and the plain name
/// are accepted; only the hashed one may be cached immutably.
pub fn lookup(name: &str) -> Option<Served> {
    ASSETS.iter().find_map(|asset| {
        if name == asset.hashed_name {
            Some(Served {
                asset,
                immutable: true,
            })
        } else if name == asset.name {
            Some(Served {
                asset,
                immutable: false,
            })
        } else {
            None
        }
    })
}

/// 64-bit FNV-1a.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, &b| {
        (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// The low 32 bits of `h` as eight lowercase hex digits.
fn hex8(h: u64) -> String {
    format!("{:08x}", h & 0xffff_ffff)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashed_names_are_stable_hex() {
        for asset in ASSETS.iter() {
            let (stem, ext) = asset.name.rsplit_once('.').unwrap();
            let hash = asset
                .hashed_name
                .strip_prefix(&format!("{stem}."))
                .and_then(|rest| rest.strip_suffix(&format!(".{ext}")))
                .unwrap_or_else(|| panic!("{}", asset.hashed_name));
            assert_eq!(hash.len(), 8, "{}", asset.hashed_name);
            assert!(hash.bytes().all(|b| b.is_ascii_hexdigit()));
        }
        assert_eq!(css_path(), format!("/static/{}", ASSETS[0].hashed_name));
    }

    #[test]
    fn recognises_hashed_and_plain_names() {
        let css = lookup("app.css").unwrap();
        assert!(!css.immutable);
        assert_eq!(css.asset.content_type, "text/css; charset=utf-8");
        let hashed = lookup(&ASSETS[0].hashed_name).unwrap();
        assert!(hashed.immutable);
        assert!(std::ptr::eq(hashed.asset, css.asset));
        let font = lookup("newsreader-latin.woff2").unwrap();
        assert_eq!(font.asset.content_type, "font/woff2");
        assert!(!font.asset.body.is_empty());
        assert_eq!(lookup("app.deadbeef.css"), None);
        assert_eq!(lookup("../etc/passwd"), None);
        assert_eq!(lookup("OFL.txt"), None);
        assert_eq!(lookup("EVANTIC.txt"), None);
    }

    #[test]
    fn stylesheet_references_only_hashed_fonts_that_exist() {
        let css = css();
        assert!(!css.contains("url(\"/static/newsreader-latin.woff2\")"));
        for font in FONT_FILES {
            let (stem, _) = font.0.rsplit_once('.').unwrap();
            let asset = lookup(font.0).unwrap().asset;
            assert!(
                css.contains(&format!("url(\"{}\")", asset.path())),
                "stylesheet does not reference {}",
                font.0
            );
            assert!(
                !css.contains(&format!("url(\"/static/{stem}.woff2\")")),
                "plain font path left in stylesheet: {stem}"
            );
        }
        // Every url() in the stylesheet resolves to an embedded asset.
        for (i, _) in css.match_indices("url(\"/static/") {
            let rest = &css[i + "url(\"".len()..];
            let end = rest.find('"').unwrap();
            let name = rest[..end].trim_start_matches("/static/");
            assert!(lookup(name).is_some(), "dangling asset url: {name}");
        }
        // Woff2 files start with the 'wOF2' signature.
        for font in FONT_FILES {
            assert_eq!(&font.1[..4], b"wOF2", "{}", font.0);
        }
    }

    #[test]
    fn javascript_stays_under_the_tripwire() {
        let total: usize = INLINE_SCRIPTS.iter().map(|s| s.len()).sum();
        assert!(
            total <= JS_BUDGET_BYTES,
            "{total} bytes of inline JS crosses the {JS_BUDGET_BYTES} byte tripwire; \
             be judicious, and if it is all earning its keep, raise the tripwire (D43)"
        );
        for script in INLINE_SCRIPTS {
            assert!(
                !script.contains("</script"),
                "inline script must not close itself"
            );
        }
    }

    #[test]
    fn the_hidden_attribute_beats_every_display_rule() {
        // An island hides the connect row, a flex container, by setting
        // `hidden`; without this the row's display rule would win.
        assert!(
            CSS_SOURCE.contains("[hidden] { display: none !important; }"),
            "the reset must make [hidden] win"
        );
    }

    #[test]
    fn stylesheet_declares_the_layer_order_first() {
        let first = CSS_SOURCE
            .lines()
            .find(|l| l.trim_start().starts_with("@layer"))
            .unwrap();
        assert_eq!(
            first.trim(),
            "@layer reset, tokens, base, layout, components, utilities;"
        );
    }
}
