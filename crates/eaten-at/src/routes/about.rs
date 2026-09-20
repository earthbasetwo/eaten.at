//! `GET /about` — what the site stands on, and the licences it has to
//! carry. A static page: the same words for everyone, every time.
//!
//! The site has no footer and no chrome above a page (2026-09-15), and
//! the editor's screens are the input and nothing else (Write Pages),
//! so the credit the choosing page used to carry lives here instead.
//! CC BY 4.0 allows that: 3(a)(2) lets the attribution sit on one page
//! the work's pages point to, and both landing pages point here.
//!
//! Overture's places are shared under licences whose one condition is
//! that their own text travel with the data (CDLA-Permissive-2.0 §2.1,
//! Apache-2.0 §4(a)), which is what the links are for; neither asks to
//! be credited. CC BY 4.0 does, so the IP database is named in DB-IP's
//! own words, linked, and said to be unmodified. The fonts are not
//! mentioned: every one carries its copyright notice in its own `name`
//! table, and the licence text they ask for travels as `OFL.txt`,
//! served beside them. Nothing else the site stands on asks for
//! anything either, so nothing is said about it (D47).

use eaten_at_web::layout::{self, Page};
use maud::{html, Markup};

pub async fn about() -> Markup {
    layout::render(&Page {
        title: &["About"],
        main: html! {
            div.page-head {
                p.kicker { "About" }
                h1 { "About eaten.at" }
                p.lede {
                    "This site renders write-ups about places people have eaten at. "
                    "It is a reader over other people's repositories, not a warehouse: "
                    "there is no global feed here, and nothing to scroll."
                }
            }
            div.prose {
                // Placeholder copy: a stand-in blurb until the real one
                // is written.
                h2 { "On the AT Protocol" }
                p {
                    "Every write-up here is a record in its author's own repository on the "
                    a href="https://atproto.com/" rel="noopener" { "AT Protocol" }
                    ", the network Bluesky is built on. A handle is an identity on it, a "
                    "repository holds what its owner has written, and any app that speaks "
                    "the protocol can read it."
                }
                h2 { "Credits" }
                p {
                    "Places come from Overture Maps, whose contributors licence them "
                    "variously: most under "
                    a href="https://cdla.dev/permissive-2-0/" rel="noopener" { "CDLA-Permissive-2.0" }
                    ", some under "
                    a href="https://www.apache.org/licenses/LICENSE-2.0" rel="noopener" { "Apache-2.0" }
                    "."
                }
                p {
                    "Where a request comes from is turned into a rough position by a "
                    "database held on this server: "
                    // DB-IP asks for this exact wording, linking this
                    // exact address (CC BY 4.0, IP-to-City Lite).
                    a href="https://db-ip.com" rel="noopener" { "IP Geolocation by DB-IP" }
                    ", their IP-to-City Lite, used unmodified under the "
                    a href="https://creativecommons.org/licenses/by/4.0/" rel="noopener" {
                        "Creative Commons Attribution 4.0 International Licence"
                    }
                    "."
                }
            }
        },
        ..Page::default()
    })
}
