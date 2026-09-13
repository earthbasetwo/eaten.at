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

# Publish to the local network and verify via the overridden _lexicon TXT
lexicons-publish-dev:
    #!/usr/bin/env bash
    set -euo pipefail
    set -a; . ./.env.dev; set +a
    cargo run -q -p eaten-at --bin publish-lexicons -- --pds "$EATEN_AT_DEV_PDS" --verify
