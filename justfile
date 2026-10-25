set dotenv-load

check: fmt clippy wasm test features doc

fmt:
    cargo fmt --all --check

clippy:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

wasm:
    cargo clippy -p df-elysium-core --no-default-features --features sign,web --target wasm32-unknown-unknown -- -D warnings

# Unit, conformance and mocked suites; the live and send suites are skipped here.
test:
    cargo test --workspace
    cargo test -p df-elysium-core --features offline --lib

# Against Elysium testnet and HyperEVM testnet, read only: every read, precompile and bridge read, each
# held against its recording. Run by hand.
live:
    cargo test -p df-elysium-core --test live -- --ignored --test-threads 1

# Real transactions on Elysium testnet from ELYSIUM_TEST_KEY in .env, a few millionths of a HYPE each.
# Run by hand.
send:
    cargo test -p df-elysium-core --test send -- --ignored --test-threads 1

# Line coverage of the unit, conformance and mocked suites.
cover:
    cargo llvm-cov -p df-elysium-core --summary-only

# Without `sign`, nothing pulls in the curve.
features:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo clippy -p df-elysium-core --no-default-features -- -D warnings
    cargo clippy -p df-elysium-core --no-default-features --target wasm32-unknown-unknown -- -D warnings
    if cargo tree -p df-elysium-core --no-default-features -e no-dev --target all | grep -q ' k256 v'; then
        echo "df-elysium-core pulls in k256 without sign" >&2
        exit 1
    fi

doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features
