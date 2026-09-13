//! RSS 2.0 per publication: the v1 follow story (plan §5.3).

use std::io::Cursor;

use quick_xml::events::{BytesDecl, BytesText, Event};
use quick_xml::Writer;

/// A feed's channel-level fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Channel {
    pub title: String,
    /// The publication's canonical origin.
    pub link: String,
    pub description: String,
    /// Absolute URL of this feed, for `atom:link rel="self"`.
    pub self_url: String,
    pub items: Vec<Item>,
}

/// One write-up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub title: String,
    /// Canonical document URL; also the GUID.
    pub link: String,
    pub description: String,
    /// RFC 3339; converted to RFC 2822 for `pubDate`.
    pub published: jiff::Timestamp,
    /// Absolute URL of the cover JPEG.
    pub image: String,
}

/// Format an instant the way RSS wants it: RFC 2822 in UTC.
pub fn rfc2822(ts: jiff::Timestamp) -> String {
    ts.strftime("%a, %d %b %Y %H:%M:%S +0000").to_string()
}

/// Serialize the channel. Every text value is escaped by the writer, so
/// titles with `&` or `<` are safe.
pub fn render(channel: &Channel) -> String {
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);
    // Writing to an in-memory buffer cannot fail, so the results are ignored.
    let _ = writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)));
    let _ = writer
        .create_element("rss")
        .with_attributes([
            ("version", "2.0"),
            ("xmlns:atom", "http://www.w3.org/2005/Atom"),
        ])
        .write_inner_content(|w| {
            w.create_element("channel").write_inner_content(|w| {
                text(w, "title", &channel.title)?;
                text(w, "link", &channel.link)?;
                text(w, "description", &channel.description)?;
                w.create_element("atom:link")
                    .with_attributes([
                        ("href", channel.self_url.as_str()),
                        ("rel", "self"),
                        ("type", "application/rss+xml"),
                    ])
                    .write_empty()?;
                if let Some(latest) = channel.items.iter().map(|i| i.published).max() {
                    text(w, "lastBuildDate", &rfc2822(latest))?;
                }
                for item in &channel.items {
                    w.create_element("item").write_inner_content(|w| {
                        text(w, "title", &item.title)?;
                        text(w, "link", &item.link)?;
                        w.create_element("guid")
                            .with_attribute(("isPermaLink", "true"))
                            .write_text_content(BytesText::new(&item.link))?;
                        text(w, "description", &item.description)?;
                        text(w, "pubDate", &rfc2822(item.published))?;
                        w.create_element("enclosure")
                            .with_attributes([
                                ("url", item.image.as_str()),
                                ("type", "image/jpeg"),
                                ("length", "0"),
                            ])
                            .write_empty()?;
                        Ok(())
                    })?;
                }
                Ok(())
            })?;
            Ok(())
        });
    let bytes = writer.into_inner().into_inner();
    String::from_utf8(bytes).unwrap_or_default()
}

fn text(w: &mut Writer<Cursor<Vec<u8>>>, tag: &str, value: &str) -> std::io::Result<()> {
    w.create_element(tag)
        .write_text_content(BytesText::new(value))
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn channel(items: Vec<Item>) -> Channel {
        Channel {
            title: "Ross & Records <live>".into(),
            link: "https://ross.eaten.at".into(),
            description: "Write-ups".into(),
            self_url: "https://eaten.at/at/did:plc:x/p/feed.xml".into(),
            items,
        }
    }

    #[test]
    fn escapes_text_and_is_well_formed() {
        let out = render(&channel(vec![Item {
            title: "Sample Subject — Curió & <Curió>".into(),
            link: "https://ross.eaten.at/2026/09/curi?x=1&y=2".into(),
            description: "First \"paragraph\".".into(),
            published: "2026-09-07T12:00:00Z".parse().unwrap(),
            image: "https://eaten.at/img/did:plc:x/d?size=og".into(),
        }]));
        assert!(
            out.starts_with("<?xml version=\"1.0\" encoding=\"utf-8\"?>"),
            "{out}"
        );
        assert!(
            out.contains("<title>Ross &amp; Records &lt;live&gt;</title>"),
            "{out}"
        );
        assert!(out.contains("Curió &amp; &lt;Curió&gt;"), "{out}");
        assert!(
            out.contains("<link>https://ross.eaten.at/2026/09/curi?x=1&amp;y=2</link>"),
            "{out}"
        );
        assert!(
            out.contains("<pubDate>Mon, 07 Sep 2026 12:00:00 +0000</pubDate>"),
            "{out}"
        );
        assert!(
            out.contains("<lastBuildDate>Mon, 07 Sep 2026 12:00:00 +0000</lastBuildDate>"),
            "{out}"
        );
        // Parse it back: well-formed, one item.
        let mut reader = quick_xml::Reader::from_str(&out);
        let mut items = 0;
        loop {
            match reader.read_event().unwrap() {
                Event::Start(e) if e.name().as_ref() == "item" => items += 1,
                Event::Eof => break,
                _ => {}
            }
        }
        assert_eq!(items, 1);
    }

    #[test]
    fn empty_channel_is_valid_and_has_no_build_date() {
        let out = render(&channel(Vec::new()));
        assert!(
            out.contains("<channel>") && !out.contains("<item>"),
            "{out}"
        );
        assert!(!out.contains("lastBuildDate"), "{out}");
        assert!(out.contains("rel=\"self\""), "{out}");
    }
}
