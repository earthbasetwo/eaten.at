//! Start a browser session for an account without signing in, so the
//! signed-in pages can be checked automatically against a local network.
//!
//! ```text
//! dev-session did:plc:…        # prints ea_session=<token>
//! ```
//!
//! This writes the same row a completed sign-in writes (`WebSessions`),
//! into the database named by `EATEN_AT_DB`, and prints the cookie pair to
//! send. No OAuth tokens are created: pages that only read work, and
//! anything that writes to the account's repository fails as an expired
//! sign-in would.
//!
//! It refuses to run unless the development policy is on
//! (`EATEN_AT_DEV_INSECURE=1`) and the public URL is a loopback address,
//! so it cannot mint a session for a deployed site.

use std::process::ExitCode;
use std::sync::Arc;

use anyhow::{bail, Context};
use eaten_at::auth::{WebSessions, COOKIE_NAME};
use eaten_at::cache::SystemClock;
use eaten_at::db::Database;
use eaten_at::settings::{env, Settings};
use eaten_at_atproto::identity::Did;
use url::{Host, Url};

const USAGE: &str = "usage: dev-session <did>";

#[tokio::main]
async fn main() -> anyhow::Result<ExitCode> {
    let mut args = std::env::args().skip(1);
    let did = match (args.next(), args.next()) {
        (Some(arg), None) if arg == "-h" || arg == "--help" => {
            println!("{USAGE}");
            return Ok(ExitCode::SUCCESS);
        }
        (Some(raw), None) => {
            Did::parse(&raw).map_err(|err| anyhow::anyhow!("{raw:?} is not a DID: {err}"))?
        }
        _ => {
            eprintln!("{USAGE}");
            return Ok(ExitCode::FAILURE);
        }
    };

    let settings = Settings::from_env().context("invalid configuration")?;
    ensure_local(&settings)?;

    let db = Database::open(&settings.db)
        .with_context(|| format!("could not open database {}", settings.db.display()))?;
    let token = WebSessions::new(db, Arc::new(SystemClock))
        .create(&did)
        .await
        .context("could not create the session")?;
    println!("{COOKIE_NAME}={token}");
    Ok(ExitCode::SUCCESS)
}

/// Refuse anything but a development run served from this machine.
fn ensure_local(settings: &Settings) -> anyhow::Result<()> {
    if !settings.dev.as_ref().is_some_and(|dev| dev.insecure) {
        bail!(
            "{} is not set; dev-session only runs against a local development network",
            env::DEV_INSECURE
        );
    }
    if !is_loopback(&settings.public_url) {
        bail!(
            "the public URL {} is not a loopback address; dev-session only runs locally",
            settings.public_url
        );
    }
    Ok(())
}

fn is_loopback(url: &Url) -> bool {
    match url.host() {
        Some(Host::Domain(name)) => name == "localhost",
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eaten_at::settings::Dev;
    use eaten_at_atproto::http::StaticHosts;
    use eaten_at_atproto::identity::StaticDns;

    fn settings(insecure: Option<bool>, public_url: &str) -> Settings {
        Settings {
            listen: "127.0.0.1:3000".parse().unwrap(),
            db: "dev.db".into(),
            public_url: Url::parse(public_url).unwrap(),
            plc_directory: None,
            oauth_key_file: None,
            bsky_appview: None,
            places: eaten_at::places::PlacesConfig::default(),
            geoip_db: None,
            dev: insecure.map(|insecure| Dev {
                insecure,
                hosts: StaticHosts::default(),
                dns: StaticDns::default(),
                location: None,
            }),
        }
    }

    #[test]
    fn runs_only_in_insecure_dev_mode_on_a_loopback_origin() {
        assert!(ensure_local(&settings(Some(true), "http://127.0.0.1:3000/")).is_ok());
        assert!(ensure_local(&settings(Some(true), "http://localhost:3100/")).is_ok());
        assert!(ensure_local(&settings(Some(true), "http://[::1]:3000/")).is_ok());
        // Production, or dev overrides without the insecure policy.
        assert!(ensure_local(&settings(None, "http://127.0.0.1:3000/")).is_err());
        assert!(ensure_local(&settings(Some(false), "http://127.0.0.1:3000/")).is_err());
        // Insecure but pointed at a real origin.
        assert!(ensure_local(&settings(Some(true), "https://eaten.at/")).is_err());
        assert!(ensure_local(&settings(Some(true), "http://localhost.eaten.at/")).is_err());
    }
}
