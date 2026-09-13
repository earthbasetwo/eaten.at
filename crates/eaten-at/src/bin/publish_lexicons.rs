//! Publish the project's lexicon schemas as `com.atproto.lexicon.schema`
//! records, or check that what is published matches the files.
//!
//! ```text
//! publish-lexicons --dry-run                  # diff files against the repo; exit 1 on drift
//! publish-lexicons                            # write whatever differs
//! publish-lexicons --verify                   # resolve each NSID via DNS and compare
//! ```
//!
//! Credentials come from `EATEN_AT_LEXICON_IDENTIFIER` (handle or DID)
//! and `EATEN_AT_LEXICON_APP_PASSWORD`. A dry run needs only the
//! identifier. The development variables from `eaten_at::settings`
//! apply, so this works against a local PDS too.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{bail, Context};
use eaten_at::settings::Settings;
use eaten_at_atproto::identity::{Did, Handle, IdentityResolver};
use eaten_at_atproto::lexicon::publish::{apply, diff, load_dir, plan, Action, Entry};
use eaten_at_atproto::lexicon::resolve::resolve_lexicon;
use eaten_at_atproto::repo::write::AppPasswordSession;
use eaten_at_atproto::repo::RepoClient;
use url::Url;

const IDENTIFIER_ENV: &str = "EATEN_AT_LEXICON_IDENTIFIER";
const PASSWORD_ENV: &str = "EATEN_AT_LEXICON_APP_PASSWORD";

#[derive(Debug)]
struct Args {
    dry_run: bool,
    verify: bool,
    lexicons: PathBuf,
    pds: Url,
}

fn parse_args() -> anyhow::Result<Args> {
    let mut args = Args {
        dry_run: false,
        verify: false,
        lexicons: PathBuf::from("lexicons"),
        pds: Url::parse("https://bsky.social").expect("constant"),
    };
    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--dry-run" => args.dry_run = true,
            "--verify" => args.verify = true,
            "--lexicons" => {
                args.lexicons = PathBuf::from(iter.next().context("--lexicons needs a path")?);
            }
            "--pds" => {
                let raw = iter.next().context("--pds needs a URL")?;
                args.pds =
                    Url::parse(&raw).with_context(|| format!("--pds {raw:?} is not a URL"))?;
            }
            "-h" | "--help" => {
                println!(
                    "usage: publish-lexicons [--dry-run] [--verify] [--lexicons DIR] [--pds URL]"
                );
                std::process::exit(0);
            }
            other => bail!("unknown argument {other:?}"),
        }
    }
    Ok(args)
}

#[tokio::main]
async fn main() -> anyhow::Result<ExitCode> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .init();
    let args = parse_args()?;
    let settings = Settings::from_env().context("invalid configuration")?;
    let http = settings.http()?;
    let dns = settings.dns()?;
    let identity = IdentityResolver::new(http.clone(), dns.clone(), settings.identity_config());

    let schemas = load_dir(&args.lexicons)?;
    println!(
        "{} schema file(s) in {}",
        schemas.len(),
        args.lexicons.display()
    );

    let mut failed = false;

    // Plan and (unless dry-running) publish; verification, when asked for,
    // follows so it checks what was just written.
    {
        let identifier = std::env::var(IDENTIFIER_ENV)
            .ok()
            .filter(|v| !v.is_empty())
            .with_context(|| format!("{IDENTIFIER_ENV} is not set"))?;
        let did = resolve_identifier(&identity, &identifier).await?;
        println!("publishing account: {identifier} ({did})");

        let reader = RepoClient::new(http.clone(), args.pds.clone());
        let entries = plan(&reader, &did, &schemas)
            .await
            .context("could not read published schemas")?;
        report(&entries);
        let drift = entries.iter().any(Entry::needs_write);

        if args.dry_run {
            if drift {
                println!("drift detected; run without --dry-run to publish");
                failed = true;
            } else {
                println!("published schemas are up to date");
            }
        } else if drift {
            let password = std::env::var(PASSWORD_ENV)
                .ok()
                .filter(|v| !v.is_empty())
                .with_context(|| format!("{PASSWORD_ENV} is not set"))?;
            let session =
                AppPasswordSession::login(http.clone(), args.pds.clone(), &identifier, &password)
                    .await
                    .context("login failed")?;
            if session.did != did {
                bail!("logged in as {} but planned against {did}", session.did);
            }
            let written = apply(&session, &entries).await.context("publish failed")?;
            for (nsid, receipt) in written {
                println!("wrote {nsid} ({})", receipt.cid);
            }
        } else {
            println!("nothing to publish");
        }
    }

    if args.verify {
        println!("verifying via _lexicon DNS resolution");
        for schema in &schemas {
            match resolve_lexicon(&identity, dns.as_ref(), &schema.id).await {
                Ok(value) if value == schema.value() => {
                    println!("  {}: resolves and matches", schema.id);
                }
                Ok(value) => {
                    println!("  {}: resolves but DIFFERS from the file", schema.id);
                    print!("{}", diff(&value, &schema.value()));
                    failed = true;
                }
                Err(err) => {
                    println!("  {}: FAILED to resolve: {err}", schema.id);
                    failed = true;
                }
            }
        }
    }

    Ok(if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

async fn resolve_identifier(identity: &IdentityResolver, identifier: &str) -> anyhow::Result<Did> {
    if let Ok(did) = Did::parse(identifier) {
        return Ok(did);
    }
    let handle = Handle::parse(identifier)
        .with_context(|| format!("{identifier:?} is neither a DID nor a handle"))?;
    Ok(identity
        .resolve_handle(&handle)
        .await
        .with_context(|| format!("could not resolve {handle}"))?
        .did)
}

fn report(entries: &[Entry]) {
    for entry in entries {
        match &entry.action {
            Action::Unchanged => println!("  {}: unchanged", entry.schema.id),
            Action::Create => println!("  {}: not yet published (would create)", entry.schema.id),
            Action::Update { current } => {
                println!("  {}: differs (would update)", entry.schema.id);
                print!("{}", diff(current, &entry.schema.value()));
            }
        }
    }
}
