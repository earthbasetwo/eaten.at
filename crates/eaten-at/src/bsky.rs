//! The Bluesky comment thread a document names through `bskyPostRef`
//! (plan §5.7, D6).
//!
//! Comments live on Bluesky, not here: the thread is read from the public
//! `AppView` with `app.bsky.feed.getPostThread`, an unauthenticated query, so
//! logged-out readers and documents crossposted by hand both get their
//! replies. Nothing is written. The thread is cached for five minutes, a
//! missing or blocked post for as long, and any failure leaves the page
//! with its plain link to the thread.

use serde::{Deserialize, Serialize};
use url::Url;

use eaten_at_atproto::at_uri::AtUri;

use crate::cache::Namespace;
use crate::state::AppState;

/// The public Bluesky `AppView`.
pub const DEFAULT_APPVIEW: &str = "https://public.api.bsky.app";
/// Levels of replies asked for. The `AppView`'s default is six; deeper
/// threads are read on Bluesky.
pub const REPLY_DEPTH: u8 = 4;
/// Most replies shown under a document. The rest are a link away.
pub const MAX_COMMENTS: usize = 100;
/// Largest thread response read; a hundred replies with their embeds
/// and author cards fit in a fraction of this.
const MAX_JSON_BYTES: usize = 4 * 1024 * 1024;
/// Longest reply text kept. Bluesky's own limit is 300 graphemes, so this
/// is only a guard against an odd record.
const MAX_TEXT_CHARS: usize = 2000;
/// Label values that mean "do not show this": Bluesky's own hide label
/// and a takedown, on either the reply or its author.
const HIDDEN_LABELS: [&str; 2] = ["!hide", "!takedown"];

/// Where the `AppView` is. Tests point it at a mock.
#[derive(Debug, Clone)]
pub struct BskyConfig {
    pub appview: Url,
}

impl Default for BskyConfig {
    fn default() -> Self {
        Self {
            appview: Url::parse(DEFAULT_APPVIEW).expect("constant"),
        }
    }
}

/// A post's thread as we show it: the replies, flattened in reading order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Thread {
    /// The root post's AT-URI.
    pub uri: String,
    /// Replies depth-first, siblings oldest first, each after the reply
    /// it answers. At most [`MAX_COMMENTS`].
    pub comments: Vec<Comment>,
    /// Whether replies past [`MAX_COMMENTS`] were dropped.
    pub truncated: bool,
}

/// One reply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comment {
    pub uri: String,
    pub author: Author,
    /// Plain text, as posted. Facets (links, mentions) are not applied;
    /// the text is shown as written.
    pub text: String,
    /// RFC 3339, from the post record.
    pub created_at: String,
    /// Nesting under the root post: `0` for a direct reply.
    pub depth: u8,
}

/// Who wrote a reply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Author {
    pub did: String,
    pub handle: String,
    pub display_name: Option<String>,
}

impl Author {
    /// The name that stands for this author in a URL: the handle, unless
    /// Bluesky marks it invalid, then the DID.
    fn slug(&self) -> &str {
        if self.handle.is_empty() || self.handle == "handle.invalid" {
            &self.did
        } else {
            &self.handle
        }
    }

    /// The author's profile on Bluesky.
    pub fn url(&self) -> String {
        format!("https://bsky.app/profile/{}", self.slug())
    }
}

impl Comment {
    /// The reply's own page on Bluesky. A reply whose URI does not parse
    /// links to its author instead of nowhere.
    pub fn url(&self) -> String {
        match AtUri::parse(&self.uri) {
            Ok(uri) => format!(
                "https://bsky.app/profile/{}/post/{}",
                self.author.slug(),
                uri.rkey()
            ),
            Err(_) => self.author.url(),
        }
    }
}

/// A post's page on bsky.app.
pub fn post_url(did: &str, rkey: &str) -> String {
    format!("https://bsky.app/profile/{did}/post/{rkey}")
}

/// Why a thread could not be read. Not-found and blocked posts are not
/// errors; they read as no thread.
#[derive(Debug, thiserror::Error)]
pub enum ThreadError {
    #[error(transparent)]
    Http(#[from] eaten_at_atproto::http::HttpError),
    #[error("AppView responded {0}")]
    Status(u16),
    #[error("could not decode thread: {0}")]
    Decode(#[from] serde_json::Error),
}

// ---- wire shapes of app.bsky.feed.getPostThread ----

#[derive(Deserialize)]
struct WireOutput {
    thread: WireNode,
}

/// A node of the thread. Anything that is not a post view, including a
/// type this build does not know, is skipped.
#[derive(Deserialize)]
#[serde(tag = "$type")]
enum WireNode {
    #[serde(rename = "app.bsky.feed.defs#threadViewPost")]
    Post(Box<WireThreadPost>),
    #[serde(rename = "app.bsky.feed.defs#notFoundPost")]
    NotFound,
    #[serde(rename = "app.bsky.feed.defs#blockedPost")]
    Blocked,
    #[serde(other)]
    Unknown,
}

#[derive(Deserialize)]
struct WireThreadPost {
    post: WirePostView,
    #[serde(default)]
    replies: Vec<WireNode>,
}

#[derive(Deserialize)]
struct WirePostView {
    uri: String,
    author: WireAuthor,
    #[serde(default)]
    record: WireRecord,
    #[serde(default)]
    labels: Vec<WireLabel>,
}

#[derive(Deserialize)]
struct WireAuthor {
    did: String,
    handle: String,
    #[serde(rename = "displayName", default)]
    display_name: Option<String>,
    #[serde(default)]
    labels: Vec<WireLabel>,
}

/// The post record, read leniently: it is `unknown` in the lexicon.
#[derive(Deserialize, Default)]
struct WireRecord {
    #[serde(default)]
    text: String,
    #[serde(rename = "createdAt", default)]
    created_at: String,
}

#[derive(Deserialize)]
struct WireLabel {
    val: String,
}

/// The error body the `AppView` sends with a 400.
#[derive(Deserialize)]
struct WireError {
    #[serde(default)]
    error: String,
}

/// Turn a parsed thread into the flat list of replies to show.
fn flatten(root: WireThreadPost) -> Thread {
    let uri = root.post.uri;
    let mut comments = Vec::new();
    collect(root.replies, 0, &mut comments);
    let truncated = comments.len() > MAX_COMMENTS;
    comments.truncate(MAX_COMMENTS);
    Thread {
        uri,
        comments,
        truncated,
    }
}

/// Append `nodes` and their replies to `out`, depth-first, siblings
/// oldest first. Hidden posts take their subtrees with them: a reply to
/// something not shown would read as a reply to nothing.
fn collect(nodes: Vec<WireNode>, depth: u8, out: &mut Vec<Comment>) {
    let mut posts: Vec<WireThreadPost> = nodes
        .into_iter()
        .filter_map(|node| match node {
            WireNode::Post(post) => Some(*post),
            WireNode::NotFound | WireNode::Blocked | WireNode::Unknown => None,
        })
        .filter(|post| !is_hidden(&post.post))
        .collect();
    posts.sort_by(|a, b| a.post.record.created_at.cmp(&b.post.record.created_at));
    for post in posts {
        let view = post.post;
        out.push(Comment {
            uri: view.uri,
            author: Author {
                did: view.author.did,
                handle: view.author.handle,
                display_name: view
                    .author
                    .display_name
                    .map(|n| n.trim().to_owned())
                    .filter(|n| !n.is_empty()),
            },
            text: clean_text(&view.record.text),
            created_at: view.record.created_at,
            depth,
        });
        collect(post.replies, depth.saturating_add(1), out);
    }
}

fn is_hidden(post: &WirePostView) -> bool {
    post.labels
        .iter()
        .chain(post.author.labels.iter())
        .any(|label| HIDDEN_LABELS.contains(&label.val.as_str()))
}

/// Reply text as shown: line endings normalised, control characters
/// other than newline and tab dropped, runs of blank lines collapsed to
/// one, trimmed, and cut at [`MAX_TEXT_CHARS`].
fn clean_text(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len().min(MAX_TEXT_CHARS));
    let mut newlines = 0;
    for c in raw.replace("\r\n", "\n").chars().take(MAX_TEXT_CHARS) {
        if c == '\n' {
            newlines += 1;
            if newlines <= 2 {
                out.push(c);
            }
        } else if c == '\t' || !c.is_control() {
            newlines = 0;
            out.push(c);
        }
    }
    out.trim().to_owned()
}

impl AppState {
    /// The comment thread under a document's Bluesky post, cached for
    /// five minutes. `None` when the post is gone, blocked, or unreadable;
    /// the page then shows only its link to the thread.
    pub async fn bluesky_thread(&self, uri: &AtUri) -> Option<Thread> {
        let result = self
            .cache()
            .get_or_fetch::<Thread, ThreadError, _, _>(
                Namespace::BlueskyThread,
                uri.as_str(),
                || async { self.fetch_thread(uri).await },
            )
            .await;
        match result {
            Ok(found) => found,
            Err(err) => {
                tracing::warn!(uri = uri.as_str(), %err, "Bluesky thread not read; showing the link");
                None
            }
        }
    }

    async fn fetch_thread(&self, post: &AtUri) -> Result<Option<Thread>, ThreadError> {
        let mut url = self.bsky().appview.clone();
        url.set_path("/xrpc/app.bsky.feed.getPostThread");
        url.query_pairs_mut()
            .append_pair("uri", post.as_str())
            .append_pair("depth", &REPLY_DEPTH.to_string())
            .append_pair("parentHeight", "0");
        let response = self.http().get_limited(url, MAX_JSON_BYTES).await?;
        let status = response.status.as_u16();
        // A deleted post is a 400 `NotFound` from the AppView.
        if status == 400 {
            let body: WireError = response.json().unwrap_or(WireError {
                error: String::new(),
            });
            return if body.error == "NotFound" {
                Ok(None)
            } else {
                Err(ThreadError::Status(status))
            };
        }
        if status == 404 {
            return Ok(None);
        }
        if !response.status.is_success() {
            return Err(ThreadError::Status(status));
        }
        let output: WireOutput = response.json()?;
        Ok(match output.thread {
            WireNode::Post(root) => Some(flatten(*root)),
            WireNode::NotFound | WireNode::Blocked | WireNode::Unknown => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const THREAD: &str = include_str!("../tests/fixtures/bsky-thread.json");

    fn parse(json: &str) -> Option<Thread> {
        let output: WireOutput = serde_json::from_str(json).unwrap();
        match output.thread {
            WireNode::Post(root) => Some(flatten(*root)),
            _ => None,
        }
    }

    #[test]
    fn fixture_flattens_in_reading_order_and_skips_what_is_not_a_post() {
        let thread = parse(THREAD).unwrap();
        assert_eq!(
            thread.uri,
            "at://did:plc:re3ebnp5v7ffagz6rb6xfei4/app.bsky.feed.post/3kroot"
        );
        assert!(!thread.truncated);
        let summary: Vec<(u8, &str, &str)> = thread
            .comments
            .iter()
            .map(|c| (c.depth, c.author.handle.as_str(), c.text.as_str()))
            .collect();
        assert_eq!(
            summary,
            vec![
                (
                    0,
                    "bob.test",
                    "Loved *this* <b>record</b>\nand the write-up."
                ),
                (1, "alice.test", "Thanks Bob!"),
                (2, "bob.test", "Any time."),
                (0, "handle.invalid", "Same here."),
            ],
            "siblings oldest first, replies nested, not-found, blocked, hidden dropped"
        );
        let bob = &thread.comments[0];
        assert_eq!(bob.author.display_name.as_deref(), Some("Bob"));
        assert_eq!(bob.author.url(), "https://bsky.app/profile/bob.test");
        assert_eq!(bob.url(), "https://bsky.app/profile/bob.test/post/3kbob1");
        assert_eq!(bob.created_at, "2026-09-08T10:00:00.000Z");
        let carol = &thread.comments[3];
        assert_eq!(carol.author.display_name, None, "blank names are dropped");
        assert_eq!(
            carol.author.url(),
            "https://bsky.app/profile/did:plc:carolcarolcarolcarolcaro",
            "an invalid handle falls back to the DID"
        );
    }

    #[test]
    fn root_that_is_not_a_post_reads_as_no_thread() {
        let not_found = r#"{"thread":{"$type":"app.bsky.feed.defs#notFoundPost","uri":"at://x/app.bsky.feed.post/y","notFound":true}}"#;
        assert_eq!(parse(not_found), None);
        let blocked = r#"{"thread":{"$type":"app.bsky.feed.defs#blockedPost","uri":"at://x/app.bsky.feed.post/y","blocked":true,"author":{"did":"did:plc:x"}}}"#;
        assert_eq!(parse(blocked), None);
        let novel = r#"{"thread":{"$type":"app.bsky.feed.defs#somethingNew","uri":"at://x/app.bsky.feed.post/y"}}"#;
        assert_eq!(parse(novel), None);
    }

    #[test]
    fn post_without_replies_is_an_empty_thread() {
        let json = r#"{"thread":{"$type":"app.bsky.feed.defs#threadViewPost","post":{"uri":"at://did:plc:a/app.bsky.feed.post/r","cid":"bafy","author":{"did":"did:plc:a","handle":"a.test"},"record":{"text":"root"},"indexedAt":"2026-09-08T09:00:00.000Z"}}}"#;
        let thread = parse(json).unwrap();
        assert!(thread.comments.is_empty());
        assert!(!thread.truncated);
    }

    #[test]
    fn long_threads_are_cut_and_say_so() {
        let make_reply = |i: usize| {
            format!(
                r#"{{"$type":"app.bsky.feed.defs#threadViewPost","post":{{"uri":"at://did:plc:b/app.bsky.feed.post/r{i}","cid":"bafy","author":{{"did":"did:plc:b","handle":"b.test"}},"record":{{"text":"reply {i}","createdAt":"2026-09-08T10:{:02}:00.000Z"}},"indexedAt":"2026-09-08T10:00:00.000Z"}}}}"#,
                i % 60
            )
        };
        let replies: Vec<String> = (0..MAX_COMMENTS + 5).map(make_reply).collect();
        let json = format!(
            r#"{{"thread":{{"$type":"app.bsky.feed.defs#threadViewPost","post":{{"uri":"at://did:plc:a/app.bsky.feed.post/r","cid":"bafy","author":{{"did":"did:plc:a","handle":"a.test"}},"record":{{"text":"root"}},"indexedAt":"2026-09-08T09:00:00.000Z"}},"replies":[{}]}}}}"#,
            replies.join(",")
        );
        let thread = parse(&json).unwrap();
        assert_eq!(thread.comments.len(), MAX_COMMENTS);
        assert!(thread.truncated);
    }

    #[test]
    fn text_is_tidied_not_interpreted() {
        assert_eq!(clean_text("  hi\r\nthere  "), "hi\nthere");
        assert_eq!(clean_text("a\n\n\n\n\nb"), "a\n\nb");
        assert_eq!(
            clean_text("tab\tok\u{7}bell\u{200b}zw"),
            "tab\tokbell\u{200b}zw"
        );
        let long = "x".repeat(MAX_TEXT_CHARS + 10);
        assert_eq!(clean_text(&long).chars().count(), MAX_TEXT_CHARS);
        assert_eq!(
            clean_text("<script>alert(1)</script>"),
            "<script>alert(1)</script>"
        );
    }

    #[test]
    fn unknown_fields_and_missing_record_are_tolerated() {
        let json = r#"{"thread":{"$type":"app.bsky.feed.defs#threadViewPost","post":{"uri":"at://did:plc:a/app.bsky.feed.post/r","cid":"bafy","author":{"did":"did:plc:a","handle":"a.test","avatar":"https://cdn/x.jpg","viewer":{}},"embed":{"$type":"app.bsky.embed.external#view"},"indexedAt":"2026-09-08T09:00:00.000Z","replyCount":1},"replies":[{"$type":"app.bsky.feed.defs#threadViewPost","post":{"uri":"not an at uri","cid":"bafy","author":{"did":"did:plc:b","handle":"b.test"},"indexedAt":"2026-09-08T09:00:00.000Z"}}]},"threadgate":{"uri":"at://x"}}"#;
        let thread = parse(json).unwrap();
        assert_eq!(thread.comments.len(), 1);
        assert_eq!(thread.comments[0].text, "");
        assert_eq!(
            thread.comments[0].url(),
            "https://bsky.app/profile/b.test",
            "an unparseable URI links to the author"
        );
    }
    /// Live check against the public `AppView`: a post on the Bluesky
    /// team's account with a thread under it.
    #[tokio::test]
    #[ignore = "reads a live Bluesky thread"]
    async fn live_public_appview_thread_parses() {
        use std::sync::Arc;

        use eaten_at_atproto::http::{GuardedClient, Policy, SystemHosts};
        let client = GuardedClient::new(
            Policy::production(),
            Arc::new(SystemHosts),
            crate::state::USER_AGENT,
        )
        .unwrap();
        let mut url = BskyConfig::default().appview;
        url.set_path("/xrpc/app.bsky.feed.getPostThread");
        url.query_pairs_mut()
            .append_pair(
                "uri",
                "at://did:plc:urx2hrgoruj3gyx5dm73igus/app.bsky.feed.post/3ms2bif6yr725",
            )
            .append_pair("depth", &REPLY_DEPTH.to_string())
            .append_pair("parentHeight", "0");
        let response = client.get_limited(url, MAX_JSON_BYTES).await.unwrap();
        assert!(response.status.is_success(), "{}", response.text());
        let thread = parse(&response.text()).expect("a thread view");
        assert!(!thread.comments.is_empty());
        assert!(thread.comments.iter().all(|c| !c.author.handle.is_empty()));
        assert!(
            thread.comments.iter().any(|c| c.depth > 0),
            "nested replies"
        );
    }
}
