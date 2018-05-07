#!/bin/sh

set -ex

# ucd-util is used by the regex crate, which supports Rust 1.12. Therefore,
# we ensure that ucd-util does the same. We don't test anything else for
# Rust 1.12.
if [ "$TRAVIS_RUST_VERSION" = "1.12.0" ]; then
  # I guess older versions of Cargo can't parse newer lock files? Bummer.
  rm Cargo.lock
  cargo build --verbose --manifest-path ucd-util/Cargo.toml
  cargo test --verbose --manifest-path ucd-util/Cargo.toml
  exit
fi

cargo build --all --verbose
cargo doc --all --verbose
cargo test --all --verbose
if [ "$TRAVIS_RUST_VERSION" = "nightly" ]; then
  cargo bench --all --verbose --no-run
  # Test no_std + alloc mode for ucd-util
  cargo test --lib --manifest-path ucd-util/Cargo.toml --no-default-features --features "alloc"
fi
