#!/bin/sh

# Sync down -------------------------------
gsutil cp \
    gs://notify-run-build-cache/target.tar.gz \
    ./
tar -xf target.tar.gz

set -e

# Run build -------------------------------
cargo build --release

# Sync up ---------------------------------
tar -zcf target.tar.gz target
! gsutil -o GSUtil:parallel_composite_upload_threshold=150M cp \
    target.tar.gz \
    gs://notify-run-build-cache/target.tar.gz

mv target/release/notify-run ./
rm -rf target
rm -rf target.tar.gz
rm -rf ~/.cargo
