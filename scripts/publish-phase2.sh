#!/usr/bin/env bash
# Publish Phase 2–4 crates (run after crates.io rate limit resets)
set -euo pipefail

echo "=== Phase 2 ==="
# Switch path → version for publish
sed -i 's|ravel-core = { path = "../ravel-core" }|ravel-core = "0.1.0"|' \
    crates/ravel-support/Cargo.toml crates/ravel-http/Cargo.toml

cargo publish -p ravel-support --allow-dirty
sleep 10
cargo publish -p ravel-http --allow-dirty
sleep 10

# Restore path deps
sed -i 's|ravel-core = "0.1.0"|ravel-core = { path = "../ravel-core" }|' \
    crates/ravel-support/Cargo.toml crates/ravel-http/Cargo.toml

echo "=== Phase 3 ==="
sed -i 's|ravel-core = { path = "../ravel-core" }|ravel-core = "0.1.0"|'     crates/ravel-db-seaorm/Cargo.toml
sed -i 's|ravel-db-core = { path = "../ravel-db-core" }|ravel-db-core = "0.1.0"|' crates/ravel-db-seaorm/Cargo.toml

cargo publish -p ravel-db-seaorm --allow-dirty
sleep 10

sed -i 's|ravel-core = "0.1.0"|ravel-core = { path = "../ravel-core" }|'     crates/ravel-db-seaorm/Cargo.toml
sed -i 's|ravel-db-core = "0.1.0"|ravel-db-core = { path = "../ravel-db-core" }|' crates/ravel-db-seaorm/Cargo.toml

echo "=== Phase 4 ==="
sed -i 's|ravel-generator = { path = "../ravel-generator" }|ravel-generator = "0.1.0"|' crates/ravel-cli/Cargo.toml
sed -i 's|ravel-db-seaorm = { path = "../ravel-db-seaorm" }|ravel-db-seaorm = "0.1.0"|' crates/ravel-cli/Cargo.toml

cargo publish -p ravel-cli --allow-dirty
sleep 10

sed -i 's|ravel-generator = "0.1.0"|ravel-generator = { path = "../ravel-generator" }|' crates/ravel-cli/Cargo.toml
sed -i 's|ravel-db-seaorm = "0.1.0"|ravel-db-seaorm = { path = "../ravel-db-seaorm" }|' crates/ravel-cli/Cargo.toml

echo "=== All done! ==="
