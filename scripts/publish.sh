#!/usr/bin/env bash
# Publish all Ravel crates to crates.io in dependency order.
#
# Usage:
#   ./scripts/publish.sh          # dry-run (check packaging)
#   ./scripts/publish.sh --live    # actually publish
#
# Prerequisites:
#   cargo login                    # one-time: log in to crates.io
#   git status --clean             # no uncommitted changes

set -euo pipefail

LIVE=false
if [[ "${1:-}" == "--live" ]]; then
    LIVE=true
fi

publish_cmd() {
    if $LIVE; then
        cargo publish -p "$1"
    else
        echo "  [dry-run] cargo publish -p $1"
        cargo package -p "$1"
    fi
}

# ── Phase 1: No internal path dependencies ──────────────────────────
echo ""
echo "=== Phase 1: Independent crates ==="

for crate in ravel-macros ravel-core ravel-db-core ravel-test ravel-generator; do
    echo "Publishing $crate..."
    publish_cmd "$crate"
    echo "  done."
    if $LIVE; then
        sleep 5  # let crates.io index update
    fi
done

# ── Phase 2: Depend on ravel-core ───────────────────────────────────
echo ""
echo "=== Phase 2: Depends on ravel-core ==="

# Temporarily switch path → version for publishing
sed -i 's|ravel-core = { path = "../ravel-core" }|ravel-core = "0.1.0"|' \
    crates/ravel-support/Cargo.toml \
    crates/ravel-http/Cargo.toml

for crate in ravel-support ravel-http; do
    echo "Publishing $crate..."
    publish_cmd "$crate"
    echo "  done."
    if $LIVE; then sleep 5; fi
done

# Restore path deps
sed -i 's|ravel-core = "0.1.0"|ravel-core = { path = "../ravel-core" }|' \
    crates/ravel-support/Cargo.toml \
    crates/ravel-http/Cargo.toml

# ── Phase 3: Depends on ravel-core + ravel-db-core ──────────────────
echo ""
echo "=== Phase 3: Depends on ravel-core + ravel-db-core ==="

sed -i 's|ravel-core = { path = "../ravel-core" }|ravel-core = "0.1.0"|' \
    crates/ravel-db-seaorm/Cargo.toml
sed -i 's|ravel-db-core = { path = "../ravel-db-core" }|ravel-db-core = "0.1.0"|' \
    crates/ravel-db-seaorm/Cargo.toml

echo "Publishing ravel-db-seaorm..."
publish_cmd "ravel-db-seaorm"
echo "  done."

sed -i 's|ravel-core = "0.1.0"|ravel-core = { path = "../ravel-core" }|' \
    crates/ravel-db-seaorm/Cargo.toml
sed -i 's|ravel-db-core = "0.1.0"|ravel-db-core = { path = "../ravel-db-core" }|' \
    crates/ravel-db-seaorm/Cargo.toml

# ── Phase 4: CLI (depends on ravel-generator + ravel-db-seaorm) ─────
echo ""
echo "=== Phase 4: CLI ==="

sed -i 's|ravel-generator = { path = "../ravel-generator" }|ravel-generator = "0.1.0"|' \
    crates/ravel-cli/Cargo.toml
sed -i 's|ravel-db-seaorm = { path = "../ravel-db-seaorm" }|ravel-db-seaorm = "0.1.0"|' \
    crates/ravel-cli/Cargo.toml

echo "Publishing ravel-cli..."
publish_cmd "ravel-cli"
echo "  done."

sed -i 's|ravel-generator = "0.1.0"|ravel-generator = { path = "../ravel-generator" }|' \
    crates/ravel-cli/Cargo.toml
sed -i 's|ravel-db-seaorm = "0.1.0"|ravel-db-seaorm = { path = "../ravel-db-seaorm" }|' \
    crates/ravel-cli/Cargo.toml

echo ""
echo "=== All crates published! ==="
