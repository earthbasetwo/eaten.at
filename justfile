set dotenv-load := true

default: check

# Format, lint, and test the whole workspace
check: fmt-check clippy test

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all --check

clippy:
    cargo clippy --workspace --all-targets -- -D warnings

test:
    cargo test --workspace

build:
    cargo build --workspace

# Run the web app (EATEN_AT_LISTEN overrides the bind address)
run *ARGS:
    cargo run -p eaten-at -- {{ARGS}}

# Diff the lexicon files against what the eaten.at account publishes
lexicons-check:
    cargo run -q -p eaten-at --bin publish-lexicons -- --dry-run

# Publish lexicon files that differ, then verify via DNS resolution
lexicons-publish:
    cargo run -q -p eaten-at --bin publish-lexicons -- --verify

# Fetch this month's DB-IP IP-to-City Lite database (CC BY 4.0) into the
# path EATEN_AT_GEOIP_DB names, atomically. Run monthly; the app reads the
# file at startup, so restart it afterwards.
geoip-refresh:
    #!/usr/bin/env bash
    set -euo pipefail
    : "${EATEN_AT_GEOIP_DB:?set EATEN_AT_GEOIP_DB to the path the database should live at}"
    month="$(date -u +%Y-%m)"
    url="https://download.db-ip.com/free/dbip-city-lite-${month}.mmdb.gz"
    echo "fetching ${url}"
    curl -fsSL "${url}" | gunzip > "${EATEN_AT_GEOIP_DB}.tmp"
    mv "${EATEN_AT_GEOIP_DB}.tmp" "${EATEN_AT_GEOIP_DB}"
    ls -la "${EATEN_AT_GEOIP_DB}"

# ---- local atproto network (see docs/local-dev.md) ----

# Start the in-memory PLC + PDS from the sibling atproto checkout and seed it.
# The webstorage flag keeps Node 25 compatible with the atproto build.
dev-env:
    ATPROTO_DIR="${ATPROTO_DIR:-../atproto}" NODE_OPTIONS=--no-experimental-webstorage node scripts/dev-env.mjs

# Run the app against the local network described by .env.dev
run-dev:
    #!/usr/bin/env bash
    set -euo pipefail
    set -a; . ./.env.dev; set +a
    cargo run -p eaten-at

# Dry-run the lexicon publish against the local network
lexicons-check-dev:
    #!/usr/bin/env bash
    set -euo pipefail
    set -a; . ./.env.dev; set +a
    cargo run -q -p eaten-at --bin publish-lexicons -- --pds "$EATEN_AT_DEV_PDS" --dry-run

# Render every page, signed out and signed in, at desktop and phone widths in
# headless Chrome against the local network; fail on errors, CSP violations,
# missing fonts, or horizontal scrolling. Screenshots go to target/visual-check/.
visual-check:
    #!/usr/bin/env bash
    set -euo pipefail
    set -a; . ./.env.dev; set +a
    node scripts/visual-check.mjs

# Export one page as a single self-contained HTML file (stylesheet and
# fonts inlined) into target/export/. Defaults to the editor for the first
# seeded write-up; pass a path, and optionally a file name, to pick another.
export-page *ARGS:
    #!/usr/bin/env bash
    set -euo pipefail
    set -a; . ./.env.dev; set +a
    node scripts/export-page.mjs {{ARGS}}

# Publish to the local network and verify via the overridden _lexicon TXT
lexicons-publish-dev:
    #!/usr/bin/env bash
    set -euo pipefail
    set -a; . ./.env.dev; set +a
    cargo run -q -p eaten-at --bin publish-lexicons -- --pds "$EATEN_AT_DEV_PDS" --verify
